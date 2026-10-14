//! A connected keyboard together with its definition and everything read from it.

use crate::definition::{self, Definition};
use crate::hid::{self, DeviceInfo};
use crate::keycode;
use crate::macros;
use crate::menus::{self, Node, Values};
use crate::proto::{self, Keyboard};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub struct Session {
    pub kb: Keyboard,
    pub def: Definition,
    pub layers: usize,
    /// [layer][row * cols + col]
    pub keymap: Vec<Vec<u16>>,
    /// [layer][encoder] = (counter-clockwise, clockwise)
    pub encoders: Vec<Vec<(u16, u16)>>,
    pub macro_count: usize,
    pub macro_buffer_size: usize,
    pub macros: Vec<Vec<macros::Action>>,
    pub menus: Vec<Node>,
    pub menu_values: Values,
    pub layout_options: Vec<usize>,
}

/// Picks a device and the definition that matches it.
pub fn pick(device: Option<&Path>, def_path: Option<&Path>) -> Result<(DeviceInfo, Definition)> {
    let devices = hid::enumerate();
    if devices.is_empty() {
        if let Some(bt) = hid::bluetooth_keyboards().first() {
            bail!(
                "{} is connected over Bluetooth, which does not carry VIA; plug in the USB cable and set the mode switch to USB",
                bt.name
            );
        }
        bail!("no VIA-capable keyboard found (no hidraw node exposes usage page 0xFF60)");
    }
    let defs: Vec<Definition> = match def_path {
        Some(p) => vec![Definition::load(p)?],
        None => definition::load_all(),
    };
    let candidates: Vec<&DeviceInfo> = match device {
        Some(p) => devices.iter().filter(|d| d.path == p).collect(),
        None => devices.iter().collect(),
    };
    for d in &candidates {
        if let Some(def) = defs.iter().find(|x| x.vendor_product_id() == d.vendor_product_id()) {
            return Ok(((*d).clone(), def.clone()));
        }
    }
    let d = candidates.first().context("that device is not a VIA keyboard")?;
    bail!(
        "no definition for {} ({:04x}:{:04x}); install its VIA json with `hidra install-def <file>` (looked in {})",
        d.name,
        d.vendor_id,
        d.product_id,
        definition::definitions_dir().display()
    )
}

impl Session {
    pub fn open(info: &DeviceInfo, def: Definition) -> Result<Self> {
        Self::with_keyboard(Keyboard::open(info)?, def)
    }

    /// A session against the in-memory keyboard, for trying the app without hardware.
    pub fn demo(def_path: Option<&Path>) -> Result<Self> {
        let def = match def_path {
            Some(p) => Definition::load(p)?,
            None => definition::load_all()
                .into_iter()
                .next()
                .context("--demo needs a definition: pass --def or install one")?,
        };
        Self::with_keyboard(Keyboard::mock(&def)?, def)
    }

    pub(crate) fn with_keyboard(kb: Keyboard, def: Definition) -> Result<Self> {
        let mut s = Session {
            kb,
            menus: menus::parse_menus(&def.menus),
            def,
            layers: 0,
            keymap: vec![],
            encoders: vec![],
            macro_count: 0,
            macro_buffer_size: 0,
            macros: vec![],
            menu_values: Values::new(),
            layout_options: vec![],
        };
        s.reload()?;
        Ok(s)
    }

    pub fn reload(&mut self) -> Result<()> {
        let (rows, cols) = (self.def.rows as usize, self.def.cols as usize);
        self.layers = self.kb.layer_count()? as usize;
        self.keymap = self.kb.read_keymap(self.layers, rows, cols)?;
        // firmware may leave out encoder or macro support; that is not a reason to fail
        if let Err(e) = self.reload_encoders() {
            if !proto::is_unhandled(&e) {
                return Err(e);
            }
            self.encoders = vec![vec![(0, 0); self.def.encoder_count()]; self.layers];
        }
        if let Err(e) = self.reload_macros() {
            if !proto::is_unhandled(&e) {
                return Err(e);
            }
            self.macro_count = 0;
            self.macros.clear();
        }
        self.reload_menus();
        if !self.def.layout_labels.is_empty() {
            let bits = self.kb.layout_options().unwrap_or(0);
            self.layout_options = self.def.unpack_options(bits);
        } else {
            self.layout_options.clear();
        }
        Ok(())
    }

    pub fn reload_encoders(&mut self) -> Result<()> {
        let n = self.def.encoder_count();
        self.encoders = Vec::with_capacity(self.layers);
        for l in 0..self.layers {
            let mut row = Vec::with_capacity(n);
            for e in 0..n {
                let ccw = self.kb.get_encoder(l as u8, e as u8, false)?;
                let cw = self.kb.get_encoder(l as u8, e as u8, true)?;
                row.push((ccw, cw));
            }
            self.encoders.push(row);
        }
        Ok(())
    }

    pub fn reload_macros(&mut self) -> Result<()> {
        if self.kb.protocol < 11 {
            // older macro encoding is not supported; leave macros empty instead of guessing
            self.macro_count = 0;
            self.macros.clear();
            return Ok(());
        }
        self.macro_count = self.kb.macro_count()? as usize;
        if self.macro_count == 0 {
            self.macros.clear();
            return Ok(());
        }
        self.macro_buffer_size = self.kb.macro_buffer_size()? as usize;
        let buf = self.kb.read_macro_buffer(self.macro_count)?;
        self.macros = macros::parse_buffer(&buf, self.macro_count);
        Ok(())
    }

    /// Reads every custom menu value; controls the firmware rejects are left out.
    pub fn reload_menus(&mut self) {
        self.menu_values.clear();
        let items: Vec<menus::Item> = menus::items(&self.menus).into_iter().cloned().collect();
        for item in items {
            if !item.readable() {
                continue;
            }
            if let Ok(v) = self.kb.custom_get(&item.command, item.value_len()) {
                self.menu_values.insert(item.id.clone(), v);
            }
        }
    }

    pub fn key_index(&self, row: u8, col: u8) -> usize {
        row as usize * self.def.cols as usize + col as usize
    }

    pub fn set_key(&mut self, layer: usize, row: u8, col: u8, code: u16) -> Result<()> {
        self.kb.set_key(layer as u8, row, col, code)?;
        let idx = self.key_index(row, col);
        self.keymap[layer][idx] = code;
        Ok(())
    }

    pub fn set_encoder(&mut self, layer: usize, id: usize, clockwise: bool, code: u16) -> Result<()> {
        self.kb.set_encoder(layer as u8, id as u8, clockwise, code)?;
        let e = &mut self.encoders[layer][id];
        if clockwise {
            e.1 = code;
        } else {
            e.0 = code;
        }
        Ok(())
    }

    pub fn macro_bytes_used(&self) -> usize {
        macros::to_bytes(&self.macros).len()
    }

    pub fn write_macros(&mut self, new: Vec<Vec<macros::Action>>) -> Result<()> {
        let bytes = macros::to_bytes(&new);
        if bytes.len() > self.macro_buffer_size {
            bail!(
                "macros need {} bytes but the keyboard has {}",
                bytes.len(),
                self.macro_buffer_size
            );
        }
        self.kb.write_macro_buffer(&bytes)?;
        // keep the same shape a fresh read would produce
        self.macros = macros::parse_buffer(&bytes, self.macro_count);
        Ok(())
    }

    pub fn find_item(&self, id: &str) -> Option<menus::Item> {
        menus::items(&self.menus).into_iter().find(|i| i.id == id).cloned()
    }

    /// Writes a custom menu value and persists its channel, as VIA does after every change.
    pub fn set_menu_value(&mut self, id: &str, data: &[u8]) -> Result<()> {
        let item = self.find_item(id).with_context(|| format!("no menu control {id}"))?;
        self.kb.custom_set(&item.command, data)?;
        self.kb.custom_save(item.channel())?;
        if item.readable() {
            self.menu_values.insert(id.to_string(), data.to_vec());
        }
        Ok(())
    }

    /// Like `set_menu_value` without saving, for live previews while a slider is dragged.
    pub fn preview_menu_value(&mut self, id: &str, data: &[u8]) -> Result<()> {
        let item = self.find_item(id).with_context(|| format!("no menu control {id}"))?;
        self.kb.custom_set(&item.command, data)?;
        self.menu_values.insert(id.to_string(), data.to_vec());
        Ok(())
    }

    pub fn save_menu_channel(&mut self, id: &str) -> Result<()> {
        let item = self.find_item(id).with_context(|| format!("no menu control {id}"))?;
        self.kb.custom_save(item.channel())
    }

    pub fn menu_number(&self, id: &str) -> i64 {
        let Some(raw) = self.menu_values.get(id) else { return 0 };
        match self.find_item(id) {
            Some(item) => menus::numeric(&item, raw) as i64,
            None => raw.first().copied().unwrap_or(0) as i64,
        }
    }

    pub fn set_layout_options(&mut self, choices: Vec<usize>) -> Result<()> {
        let bits = self.def.pack_options(&choices);
        self.kb.set_layout_options(bits)?;
        self.layout_options = choices;
        Ok(())
    }

    pub fn to_backup(&self) -> Backup {
        Backup {
            name: self.def.name.clone(),
            vendor_product_id: self.def.vendor_product_id(),
            macros: self.macros.iter().map(|m| macros::to_expression(m)).collect(),
            layers: self
                .keymap
                .iter()
                .map(|l| l.iter().map(|k| keycode::to_string(*k)).collect())
                .collect(),
            encoders: (0..self.def.encoder_count())
                .map(|e| {
                    self.encoders
                        .iter()
                        .map(|layer| {
                            let (ccw, cw) = layer[e];
                            [keycode::to_string(ccw), keycode::to_string(cw)]
                        })
                        .collect()
                })
                .collect(),
            via_native: Some(Extras {
                menus: self.menu_values.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                layout_options: (!self.layout_options.is_empty()).then(|| self.layout_options.clone()),
            }),
        }
    }

    /// Writes a backup to the keyboard. Every keycode is validated before anything is sent.
    pub fn restore(&mut self, b: &Backup) -> Result<()> {
        if b.vendor_product_id != self.def.vendor_product_id() {
            bail!("this backup is for {} (vendorProductId {:#x}), not this keyboard", b.name, b.vendor_product_id);
        }
        let per_layer = self.def.rows as usize * self.def.cols as usize;
        let mut keymap = self.keymap.clone();
        for (l, layer) in b.layers.iter().enumerate().take(self.layers) {
            if layer.len() != per_layer {
                bail!("layer {l} has {} keys, expected {per_layer}", layer.len());
            }
            for (i, s) in layer.iter().enumerate() {
                keymap[l][i] = if s.is_empty() {
                    0
                } else {
                    keycode::parse(s).with_context(|| format!("layer {l} key {i}: unknown keycode {s}"))?
                };
            }
        }
        let mut encoders = self.encoders.clone();
        for (e, per_layer) in b.encoders.iter().enumerate().take(self.def.encoder_count()) {
            for (l, [ccw, cw]) in per_layer.iter().enumerate().take(self.layers) {
                let p = |s: &str| keycode::parse(s).with_context(|| format!("encoder {e} layer {l}: unknown keycode {s}"));
                encoders[l][e] = (p(ccw)?, p(cw)?);
            }
        }
        let new_macros: Vec<Vec<macros::Action>> = if self.macro_count > 0 {
            let mut m: Vec<Vec<macros::Action>> = b
                .macros
                .iter()
                .take(self.macro_count)
                .enumerate()
                .map(|(i, e)| macros::from_expression(e).with_context(|| format!("macro {i}")))
                .collect::<Result<_>>()?;
            m.resize(self.macro_count, Vec::new());
            m
        } else {
            vec![]
        };

        self.kb.write_keymap(&keymap)?;
        self.keymap = keymap;
        for (l, layer) in encoders.iter().enumerate() {
            for (e, (ccw, cw)) in layer.iter().enumerate() {
                if self.encoders[l][e] != (*ccw, *cw) {
                    self.set_encoder(l, e, false, *ccw)?;
                    self.set_encoder(l, e, true, *cw)?;
                }
            }
        }
        if self.macro_count > 0 && macros::to_bytes(&new_macros) != macros::to_bytes(&self.macros) {
            self.write_macros(new_macros)?;
        }
        if let Some(extra) = &b.via_native {
            for (id, data) in &extra.menus {
                if self.find_item(id).is_some() && self.menu_values.get(id) != Some(data) {
                    self.set_menu_value(id, data)?;
                }
            }
            if let Some(opts) = &extra.layout_options {
                if !self.def.layout_labels.is_empty() {
                    self.set_layout_options(opts.clone())?;
                }
            }
        }
        Ok(())
    }
}

/// VIA's "Save + Load" file, plus an optional section VIA ignores.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Backup {
    pub name: String,
    #[serde(rename = "vendorProductId")]
    pub vendor_product_id: u32,
    #[serde(default)]
    pub macros: Vec<String>,
    pub layers: Vec<Vec<String>>,
    #[serde(default)]
    pub encoders: Vec<Vec<[String; 2]>>,
    #[serde(default, rename = "hidra", alias = "viaNative", skip_serializing_if = "Option::is_none")]
    pub via_native: Option<Extras>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Extras {
    #[serde(default)]
    pub menus: BTreeMap<String, Vec<u8>>,
    #[serde(default, rename = "layoutOptions", skip_serializing_if = "Option::is_none")]
    pub layout_options: Option<Vec<usize>>,
}

impl Backup {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("{} is not a VIA layout file", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        std::fs::write(path, serde_json::to_string_pretty(self)? + "\n")
            .with_context(|| format!("writing {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo() -> Session {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/gmk70.json")).unwrap();
        let def = Definition::parse(&text).unwrap();
        Session::with_keyboard(Keyboard::mock(&def).unwrap(), def).unwrap()
    }

    #[test]
    fn backup_roundtrip() {
        let mut a = demo();
        a.set_key(2, 1, 3, keycode::parse("LT(3,KC_ESC)").unwrap()).unwrap();
        a.set_encoder(1, 0, true, keycode::parse("KC_MNXT").unwrap()).unwrap();
        let mut m = a.macros.clone();
        m[5] = macros::from_expression("abc{KC_LSFT,KC_TAB}{50}").unwrap();
        a.write_macros(m).unwrap();
        a.set_menu_value("id_qmk_rgb_matrix_color", &[42, 99]).unwrap();
        let backup = a.to_backup();

        let mut b = demo();
        b.restore(&backup).unwrap();
        b.reload().unwrap();
        assert_eq!(a.keymap, b.keymap);
        assert_eq!(a.encoders, b.encoders);
        assert_eq!(a.macros, b.macros);
        assert_eq!(b.menu_values.get("id_qmk_rgb_matrix_color"), Some(&vec![42, 99]));
    }

    #[test]
    fn restore_rejects_other_keyboards() {
        let mut a = demo();
        let mut backup = a.to_backup();
        backup.vendor_product_id ^= 1;
        assert!(a.restore(&backup).is_err());
    }
}
