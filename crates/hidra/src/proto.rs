//! VIA protocol commands, mirroring the-via/app src/utils/keyboard-api.ts.

use crate::hid::{DeviceInfo, HidDevice, REPORT_LEN};
use crate::mock::MockKeyboard;
use anyhow::{Result, bail};
use std::time::Duration;

enum Transport {
    Hid(HidDevice),
    Mock(Box<MockKeyboard>, Option<[u8; REPORT_LEN]>),
}

impl Transport {
    fn write_report(&mut self, data: &[u8]) -> Result<()> {
        match self {
            Transport::Hid(d) => d.write_report(data),
            Transport::Mock(m, pending) => {
                *pending = Some(m.handle(data));
                Ok(())
            }
        }
    }

    fn read_report(&mut self, timeout: Duration) -> Result<Option<[u8; REPORT_LEN]>> {
        match self {
            Transport::Hid(d) => d.read_report(timeout),
            Transport::Mock(_, pending) => Ok(pending.take()),
        }
    }
}

pub mod cmd {
    pub const GET_PROTOCOL_VERSION: u8 = 0x01;
    pub const GET_KEYBOARD_VALUE: u8 = 0x02;
    pub const SET_KEYBOARD_VALUE: u8 = 0x03;
    pub const DYNAMIC_KEYMAP_GET_KEYCODE: u8 = 0x04;
    pub const DYNAMIC_KEYMAP_SET_KEYCODE: u8 = 0x05;
    pub const DYNAMIC_KEYMAP_RESET: u8 = 0x06;
    pub const CUSTOM_MENU_SET_VALUE: u8 = 0x07;
    pub const CUSTOM_MENU_GET_VALUE: u8 = 0x08;
    pub const CUSTOM_MENU_SAVE: u8 = 0x09;
    pub const BOOTLOADER_JUMP: u8 = 0x0b;
    pub const MACRO_GET_COUNT: u8 = 0x0c;
    pub const MACRO_GET_BUFFER_SIZE: u8 = 0x0d;
    pub const MACRO_GET_BUFFER: u8 = 0x0e;
    pub const MACRO_SET_BUFFER: u8 = 0x0f;
    pub const MACRO_RESET: u8 = 0x10;
    pub const DYNAMIC_KEYMAP_GET_LAYER_COUNT: u8 = 0x11;
    pub const DYNAMIC_KEYMAP_GET_BUFFER: u8 = 0x12;
    pub const DYNAMIC_KEYMAP_SET_BUFFER: u8 = 0x13;
    pub const DYNAMIC_KEYMAP_GET_ENCODER: u8 = 0x14;
    pub const DYNAMIC_KEYMAP_SET_ENCODER: u8 = 0x15;
    pub const UNHANDLED: u8 = 0xff;
}

pub mod value {
    pub const UPTIME: u8 = 0x01;
    pub const LAYOUT_OPTIONS: u8 = 0x02;
    pub const SWITCH_MATRIX_STATE: u8 = 0x03;
    pub const FIRMWARE_VERSION: u8 = 0x04;
    pub const DEVICE_INDICATION: u8 = 0x05;
}

/// The firmware answered 0xFF: it does not implement this command.
#[derive(Debug)]
pub struct Unhandled(pub u8);

impl std::fmt::Display for Unhandled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "keyboard does not handle command 0x{:02x}", self.0)
    }
}

impl std::error::Error for Unhandled {}

pub fn is_unhandled(e: &anyhow::Error) -> bool {
    e.downcast_ref::<Unhandled>().is_some()
}

/// Largest payload a single buffer read/write can carry.
pub const CHUNK: usize = 28;

pub struct Keyboard {
    dev: Transport,
    info: DeviceInfo,
    pub protocol: u16,
    timeout: Duration,
}

impl Keyboard {
    pub fn open(info: &DeviceInfo) -> Result<Self> {
        let mut dev = HidDevice::open(info)?;
        dev.drain();
        Self::start(Transport::Hid(dev), info.clone())
    }

    /// A keyboard simulated in memory, for `--demo`.
    pub fn mock(def: &crate::definition::Definition) -> Result<Self> {
        let info = DeviceInfo {
            path: "demo".into(),
            vendor_id: def.vendor_id,
            product_id: def.product_id,
            name: def.name.clone(),
        };
        Self::start(Transport::Mock(Box::new(MockKeyboard::new(def)), None), info)
    }

    fn start(dev: Transport, info: DeviceInfo) -> Result<Self> {
        let mut kb = Self {
            dev,
            info,
            protocol: 0,
            timeout: Duration::from_millis(500),
        };
        kb.protocol = kb.protocol_version()?;
        Ok(kb)
    }

    pub fn info(&self) -> &DeviceInfo {
        &self.info
    }

    /// Sends one command and waits for the reply that echoes it back.
    pub fn command(&mut self, command: u8, args: &[u8]) -> Result<[u8; REPORT_LEN]> {
        let mut req = Vec::with_capacity(1 + args.len());
        req.push(command);
        req.extend_from_slice(args);
        // over a 2.4G receiver a report can get lost on the radio; every command
        // used here is safe to repeat (gets, and sets of the same value)
        for _ in 0..3 {
            self.dev.write_report(&req)?;
            loop {
                let Some(resp) = self.dev.read_report(self.timeout)? else {
                    break;
                };
                if resp[0] == cmd::UNHANDLED && command != cmd::UNHANDLED {
                    return Err(Unhandled(command).into());
                }
                // the firmware echoes the command and its arguments; stray reports are skipped
                if resp[..req.len()] == req[..] {
                    return Ok(resp);
                }
            }
        }
        bail!("timeout waiting for reply to command 0x{command:02x}")
    }

    fn protocol_version(&mut self) -> Result<u16> {
        let r = self.command(cmd::GET_PROTOCOL_VERSION, &[])?;
        Ok(u16::from_be_bytes([r[1], r[2]]))
    }

    pub fn keyboard_value(&mut self, id: u8, params: &[u8], len: usize) -> Result<Vec<u8>> {
        let mut args = vec![id];
        args.extend_from_slice(params);
        let r = self.command(cmd::GET_KEYBOARD_VALUE, &args)?;
        let start = 1 + args.len();
        Ok(r[start..(start + len).min(REPORT_LEN)].to_vec())
    }

    pub fn set_keyboard_value(&mut self, id: u8, data: &[u8]) -> Result<()> {
        let mut args = vec![id];
        args.extend_from_slice(data);
        self.command(cmd::SET_KEYBOARD_VALUE, &args)?;
        Ok(())
    }

    /// Firmware-defined build number; many boards leave it at zero.
    pub fn firmware_version(&mut self) -> Result<u32> {
        let v = self.keyboard_value(value::FIRMWARE_VERSION, &[], 4)?;
        Ok(u32::from_be_bytes([v[0], v[1], v[2], v[3]]))
    }

    pub fn uptime_ms(&mut self) -> Result<u32> {
        let v = self.keyboard_value(value::UPTIME, &[], 4)?;
        Ok(u32::from_be_bytes([v[0], v[1], v[2], v[3]]))
    }

    pub fn layout_options(&mut self) -> Result<u32> {
        let v = self.keyboard_value(value::LAYOUT_OPTIONS, &[], 4)?;
        Ok(u32::from_be_bytes([v[0], v[1], v[2], v[3]]))
    }

    pub fn set_layout_options(&mut self, bits: u32) -> Result<()> {
        self.set_keyboard_value(value::LAYOUT_OPTIONS, &bits.to_be_bytes())
    }

    /// Makes the keyboard blink its lights so the user can tell which one is selected.
    pub fn indicate(&mut self) -> Result<()> {
        self.set_keyboard_value(value::DEVICE_INDICATION, &[0])
    }

    /// Raw switch state, one bit per matrix position, rows packed big-endian.
    pub fn matrix_state(&mut self, rows: usize, cols: usize) -> Result<Vec<bool>> {
        let bytes_per_row = cols.div_ceil(8);
        let rows_per_query = (CHUNK / bytes_per_row).max(1);
        let mut flat = Vec::with_capacity(rows * bytes_per_row);
        let mut row = 0;
        while row < rows {
            let n = rows_per_query.min(rows - row);
            let params: Vec<u8> = if self.protocol >= 12 { vec![row as u8] } else { vec![] };
            let v = self.keyboard_value(value::SWITCH_MATRIX_STATE, &params, n * bytes_per_row)?;
            flat.extend_from_slice(&v);
            if self.protocol < 12 {
                // older firmware returns everything it can fit in one reply
                break;
            }
            row += n;
        }
        let mut pressed = vec![false; rows * cols];
        for r in 0..rows {
            for c in 0..cols {
                let byte_idx = r * bytes_per_row + (bytes_per_row - 1 - c / 8);
                if let Some(b) = flat.get(byte_idx) {
                    pressed[r * cols + c] = (b >> (c % 8)) & 1 == 1;
                }
            }
        }
        Ok(pressed)
    }

    pub fn layer_count(&mut self) -> Result<u8> {
        if self.protocol < 8 {
            return Ok(4);
        }
        Ok(self.command(cmd::DYNAMIC_KEYMAP_GET_LAYER_COUNT, &[])?[1])
    }

    pub fn get_key(&mut self, layer: u8, row: u8, col: u8) -> Result<u16> {
        let r = self.command(cmd::DYNAMIC_KEYMAP_GET_KEYCODE, &[layer, row, col])?;
        Ok(u16::from_be_bytes([r[4], r[5]]))
    }

    pub fn set_key(&mut self, layer: u8, row: u8, col: u8, keycode: u16) -> Result<()> {
        let [hi, lo] = keycode.to_be_bytes();
        self.command(cmd::DYNAMIC_KEYMAP_SET_KEYCODE, &[layer, row, col, hi, lo])?;
        Ok(())
    }

    /// Reads the whole dynamic keymap: `layers` blocks of rows*cols keycodes.
    pub fn read_keymap(&mut self, layers: usize, rows: usize, cols: usize) -> Result<Vec<Vec<u16>>> {
        let total_bytes = layers * rows * cols * 2;
        let mut raw = Vec::with_capacity(total_bytes);
        let mut offset = 0;
        while offset < total_bytes {
            let size = CHUNK.min(total_bytes - offset);
            let [hi, lo] = (offset as u16).to_be_bytes();
            let r = self.command(cmd::DYNAMIC_KEYMAP_GET_BUFFER, &[hi, lo, size as u8])?;
            raw.extend_from_slice(&r[4..4 + size]);
            offset += size;
        }
        let per_layer = rows * cols;
        Ok(raw
            .chunks(2)
            .map(|p| u16::from_be_bytes([p[0], p[1]]))
            .collect::<Vec<_>>()
            .chunks(per_layer)
            .map(|c| c.to_vec())
            .collect())
    }

    pub fn write_keymap(&mut self, keymap: &[Vec<u16>]) -> Result<()> {
        let raw: Vec<u8> = keymap
            .iter()
            .flatten()
            .flat_map(|k| k.to_be_bytes())
            .collect();
        for (i, chunk) in raw.chunks(CHUNK).enumerate() {
            let [hi, lo] = ((i * CHUNK) as u16).to_be_bytes();
            let mut args = vec![hi, lo, chunk.len() as u8];
            args.extend_from_slice(chunk);
            self.command(cmd::DYNAMIC_KEYMAP_SET_BUFFER, &args)?;
        }
        Ok(())
    }

    pub fn get_encoder(&mut self, layer: u8, id: u8, clockwise: bool) -> Result<u16> {
        let r = self.command(cmd::DYNAMIC_KEYMAP_GET_ENCODER, &[layer, id, clockwise as u8])?;
        Ok(u16::from_be_bytes([r[4], r[5]]))
    }

    pub fn set_encoder(&mut self, layer: u8, id: u8, clockwise: bool, keycode: u16) -> Result<()> {
        let [hi, lo] = keycode.to_be_bytes();
        self.command(cmd::DYNAMIC_KEYMAP_SET_ENCODER, &[layer, id, clockwise as u8, hi, lo])?;
        Ok(())
    }

    /// `command` is the content tail of a VIA menu item: channel, value id and any extra bytes.
    pub fn custom_get(&mut self, command: &[u8], len: usize) -> Result<Vec<u8>> {
        let r = self.command(cmd::CUSTOM_MENU_GET_VALUE, command)?;
        let start = 1 + command.len();
        Ok(r[start..(start + len).min(REPORT_LEN)].to_vec())
    }

    pub fn custom_set(&mut self, command: &[u8], data: &[u8]) -> Result<()> {
        let mut args = command.to_vec();
        args.extend_from_slice(data);
        self.command(cmd::CUSTOM_MENU_SET_VALUE, &args)?;
        Ok(())
    }

    pub fn custom_save(&mut self, channel: u8) -> Result<()> {
        self.command(cmd::CUSTOM_MENU_SAVE, &[channel])?;
        Ok(())
    }

    pub fn macro_count(&mut self) -> Result<u8> {
        Ok(self.command(cmd::MACRO_GET_COUNT, &[])?[1])
    }

    pub fn macro_buffer_size(&mut self) -> Result<u16> {
        let r = self.command(cmd::MACRO_GET_BUFFER_SIZE, &[])?;
        Ok(u16::from_be_bytes([r[1], r[2]]))
    }

    /// Reads the macro buffer only as far as the last macro reaches; on a slow
    /// keyboard the full buffer costs over a hundred round trips.
    pub fn read_macro_buffer(&mut self, count: usize) -> Result<Vec<u8>> {
        let size = self.macro_buffer_size()? as usize;
        let mut out = Vec::with_capacity(size);
        let mut offset = 0;
        while offset < size {
            let n = CHUNK.min(size - offset);
            let [hi, lo] = (offset as u16).to_be_bytes();
            let r = self.command(cmd::MACRO_GET_BUFFER, &[hi, lo, n as u8])?;
            out.extend_from_slice(&r[4..4 + n]);
            offset += n;
            if crate::macros::used_len(&out, count).is_some() {
                break;
            }
        }
        Ok(out)
    }

    /// Rewrites the macro buffer the same way VIA does, flagging the last byte
    /// while the write is in progress so the firmware ignores a half-written buffer.
    pub fn write_macro_buffer(&mut self, data: &[u8]) -> Result<()> {
        let size = self.macro_buffer_size()? as usize;
        if data.len() > size {
            bail!("macros take {} bytes but the keyboard only has {size}", data.len());
        }
        let last = ((size - 1) as u16).to_be_bytes();
        self.command(cmd::MACRO_RESET, &[])?;
        self.command(cmd::MACRO_SET_BUFFER, &[last[0], last[1], 1, 0xff])?;
        let result = (|| {
            for (i, chunk) in data.chunks(CHUNK).enumerate() {
                let [hi, lo] = ((i * CHUNK) as u16).to_be_bytes();
                let mut args = vec![hi, lo, chunk.len() as u8];
                args.extend_from_slice(chunk);
                self.command(cmd::MACRO_SET_BUFFER, &args)?;
            }
            Ok(())
        })();
        self.command(cmd::MACRO_SET_BUFFER, &[last[0], last[1], 1, 0x00])?;
        result
    }

    pub fn reset_keymap(&mut self) -> Result<()> {
        self.command(cmd::DYNAMIC_KEYMAP_RESET, &[])?;
        Ok(())
    }

    /// The keyboard reboots into its bootloader and never answers.
    pub fn jump_to_bootloader(&mut self) -> Result<()> {
        self.dev.write_report(&[cmd::BOOTLOADER_JUMP])
    }

    /// Sends arbitrary bytes and returns the raw reply, for poking at vendor extensions.
    pub fn raw(&mut self, bytes: &[u8]) -> Result<[u8; REPORT_LEN]> {
        if bytes.is_empty() {
            bail!("empty command");
        }
        self.dev.write_report(bytes)?;
        match self.dev.read_report(self.timeout)? {
            Some(r) => Ok(r),
            None => bail!("timeout"),
        }
    }
}
