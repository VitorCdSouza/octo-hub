//! An in-memory VIA keyboard for `--demo` and tests: it answers the protocol
//! the way QMK's via.c does, so the whole app can run without hardware.

use crate::definition::Definition;
use crate::hid::REPORT_LEN;
use crate::proto::{cmd, value};
use std::collections::HashMap;

pub struct MockKeyboard {
    layers: usize,
    rows: usize,
    cols: usize,
    keymap: Vec<u8>,
    encoders: HashMap<(u8, u8, u8), u16>,
    macro_count: u8,
    macros: Vec<u8>,
    custom: HashMap<Vec<u8>, Vec<u8>>,
    layout_options: u32,
    started: std::time::Instant,
}

impl MockKeyboard {
    pub fn new(def: &Definition) -> Self {
        let (rows, cols, layers) = (def.rows as usize, def.cols as usize, 4);
        let mut keymap = vec![0u8; layers * rows * cols * 2];
        // fill layer 0 row by row with a 65% ANSI layout so the demo looks like a keyboard
        let rows_codes: [&[&str]; 5] = [
            &["KC_MUTE", "KC_ESC", "KC_1", "KC_2", "KC_3", "KC_4", "KC_5", "KC_6", "KC_7", "KC_8", "KC_9", "KC_0", "KC_MINS", "KC_EQL", "KC_GRV", "KC_BSPC"],
            &["MACRO(0)", "KC_TAB", "KC_Q", "KC_W", "KC_E", "KC_R", "KC_T", "KC_Y", "KC_U", "KC_I", "KC_O", "KC_P", "KC_LBRC", "KC_RBRC", "KC_BSLS"],
            &["KC_PGUP", "KC_CAPS", "KC_A", "KC_S", "KC_D", "KC_F", "KC_G", "KC_H", "KC_J", "KC_K", "KC_L", "KC_SCLN", "KC_QUOT", "KC_ENT"],
            &["KC_PGDN", "KC_LSFT", "KC_Z", "KC_X", "KC_C", "KC_V", "KC_B", "KC_N", "KC_M", "KC_COMM", "KC_DOT", "KC_SLSH", "KC_RSFT", "KC_UP", "KC_DEL"],
            &["MO(1)", "KC_LCTL", "KC_LGUI", "KC_LALT", "KC_SPC", "LT(1,KC_SPC)", "KC_RALT", "MO(1)", "KC_LEFT", "KC_DOWN", "KC_RGHT"],
        ];
        let mut keys: Vec<_> = def.keys.iter().collect();
        keys.sort_by(|a, b| (a.y.round() as i32, a.x).partial_cmp(&(b.y.round() as i32, b.x)).unwrap());
        let mut row_idx = 0;
        let mut col_idx = 0;
        let mut last_y = keys.first().map(|k| k.y.round() as i32).unwrap_or(0);
        for k in keys {
            let y = k.y.round() as i32;
            if y != last_y {
                row_idx += 1;
                col_idx = 0;
                last_y = y;
            }
            let code = rows_codes
                .get(row_idx)
                .and_then(|r| r.get(col_idx))
                .and_then(|c| crate::keycode::parse(c))
                .unwrap_or(0x01);
            col_idx += 1;
            let off = (k.row as usize * cols + k.col as usize) * 2;
            keymap[off..off + 2].copy_from_slice(&code.to_be_bytes());
            for l in 1..layers {
                let o = l * rows * cols * 2 + off;
                keymap[o..o + 2].copy_from_slice(&1u16.to_be_bytes());
            }
        }
        // like the GMK70: a Mac copy of the base layer, reached with TO(2) and left with TO(0)
        if layers >= 4 && rows > 2 && cols > 3 {
            let per = rows * cols * 2;
            let base = keymap[..per].to_vec();
            keymap[2 * per..3 * per].copy_from_slice(&base);
            let put = |km: &mut Vec<u8>, layer: usize, row: usize, col: usize, code: u16| {
                let o = layer * per + (row * cols + col) * 2;
                km[o..o + 2].copy_from_slice(&code.to_be_bytes());
            };
            put(&mut keymap, 1, 2, 3, crate::keycode::QK_TO + 2);
            put(&mut keymap, 3, 2, 2, crate::keycode::QK_TO);
        }
        let mut encoders = HashMap::new();
        for l in 0..layers as u8 {
            encoders.insert((l, 0, 0), 0x00aa);
            encoders.insert((l, 0, 1), 0x00a9);
        }
        let mut macros = vec![0u8; 3322];
        let demo = crate::macros::to_bytes(&[crate::macros::from_expression("{KC_LCTL,KC_C}").unwrap()]);
        macros[..demo.len()].copy_from_slice(&demo);
        let mut custom = HashMap::new();
        custom.insert(vec![3, 1], vec![180]);
        custom.insert(vec![3, 2], vec![12]);
        custom.insert(vec![3, 3], vec![90]);
        custom.insert(vec![3, 4], vec![20, 255]);
        MockKeyboard {
            layers,
            rows,
            cols,
            keymap,
            encoders,
            macro_count: 16,
            macros,
            custom,
            layout_options: 0,
            started: std::time::Instant::now(),
        }
    }

    pub fn handle(&mut self, req: &[u8]) -> [u8; REPORT_LEN] {
        let mut r = [0u8; REPORT_LEN];
        r[..req.len().min(REPORT_LEN)].copy_from_slice(&req[..req.len().min(REPORT_LEN)]);
        let arg = |i: usize| req.get(i).copied().unwrap_or(0);
        let off16 = |i: usize| u16::from_be_bytes([arg(i), arg(i + 1)]) as usize;
        match arg(0) {
            cmd::GET_PROTOCOL_VERSION => {
                r[1] = 0;
                r[2] = 12;
            }
            cmd::GET_KEYBOARD_VALUE => match arg(1) {
                value::UPTIME => r[2..6].copy_from_slice(&(self.started.elapsed().as_millis() as u32).to_be_bytes()),
                value::LAYOUT_OPTIONS => r[2..6].copy_from_slice(&self.layout_options.to_be_bytes()),
                value::SWITCH_MATRIX_STATE => {
                    for b in &mut r[3..] {
                        *b = 0;
                    }
                }
                _ => {}
            },
            cmd::SET_KEYBOARD_VALUE => {
                if arg(1) == value::LAYOUT_OPTIONS {
                    self.layout_options = u32::from_be_bytes([arg(2), arg(3), arg(4), arg(5)]);
                }
            }
            cmd::DYNAMIC_KEYMAP_GET_KEYCODE => {
                let o = ((arg(1) as usize * self.rows + arg(2) as usize) * self.cols + arg(3) as usize) * 2;
                r[4] = self.keymap[o];
                r[5] = self.keymap[o + 1];
            }
            cmd::DYNAMIC_KEYMAP_SET_KEYCODE => {
                let o = ((arg(1) as usize * self.rows + arg(2) as usize) * self.cols + arg(3) as usize) * 2;
                self.keymap[o] = arg(4);
                self.keymap[o + 1] = arg(5);
            }
            cmd::DYNAMIC_KEYMAP_GET_LAYER_COUNT => r[1] = self.layers as u8,
            cmd::DYNAMIC_KEYMAP_GET_BUFFER => {
                let (o, n) = (off16(1), arg(3) as usize);
                for i in 0..n {
                    r[4 + i] = self.keymap.get(o + i).copied().unwrap_or(0);
                }
            }
            cmd::DYNAMIC_KEYMAP_SET_BUFFER => {
                let (o, n) = (off16(1), arg(3) as usize);
                for i in 0..n {
                    if let Some(b) = self.keymap.get_mut(o + i) {
                        *b = arg(4 + i);
                    }
                }
            }
            cmd::DYNAMIC_KEYMAP_GET_ENCODER => {
                let v = self.encoders.get(&(arg(1), arg(2), arg(3))).copied().unwrap_or(0);
                r[4..6].copy_from_slice(&v.to_be_bytes());
            }
            cmd::DYNAMIC_KEYMAP_SET_ENCODER => {
                self.encoders
                    .insert((arg(1), arg(2), arg(3)), u16::from_be_bytes([arg(4), arg(5)]));
            }
            cmd::CUSTOM_MENU_GET_VALUE => {
                let v = self.custom.get(&req[1..3].to_vec()).cloned().unwrap_or_default();
                for (i, b) in v.iter().enumerate() {
                    r[3 + i] = *b;
                }
            }
            cmd::CUSTOM_MENU_SET_VALUE => {
                self.custom.insert(req[1..3].to_vec(), req[3..].to_vec());
            }
            cmd::CUSTOM_MENU_SAVE => {}
            cmd::MACRO_GET_COUNT => r[1] = self.macro_count,
            cmd::MACRO_GET_BUFFER_SIZE => r[1..3].copy_from_slice(&(self.macros.len() as u16).to_be_bytes()),
            cmd::MACRO_GET_BUFFER => {
                let (o, n) = (off16(1), arg(3) as usize);
                for i in 0..n {
                    r[4 + i] = self.macros.get(o + i).copied().unwrap_or(0);
                }
            }
            cmd::MACRO_SET_BUFFER => {
                let (o, n) = (off16(1), arg(3) as usize);
                for i in 0..n {
                    if let Some(b) = self.macros.get_mut(o + i) {
                        *b = arg(4 + i);
                    }
                }
            }
            cmd::MACRO_RESET => self.macros.iter_mut().for_each(|b| *b = 0),
            cmd::DYNAMIC_KEYMAP_RESET => {}
            _ => r[0] = cmd::UNHANDLED,
        }
        r
    }
}
