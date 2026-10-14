//! The M-Vave Tank-G over its USB MIDI port: one request at a time, each
//! answered by the pedal (a read by its data, a write by the 8-byte ack).
//! Ported from TabJam's `TankG.cs`.
//!
//! Live edits are refused in bypass and while the sounding preset is unknown,
//! and the table is only ever written one preset at a time: its `A5 5A` header
//! is never touched, and Factory Restore is never sent.

use super::frame;
use super::port::Port;
use super::preset::{self, Preset};
use anyhow::{Context, Result, bail};
use std::time::{Duration, Instant};

const SPACE: u8 = 0x04;
const LIVE: u32 = 0x1000_0000;
const REGISTER: u32 = 0x2000_0000;
const LOAD_PRESET: u32 = 0xe000_0000;
const BYPASS_PRESET: u32 = 0xe100_0000;
const COMMIT: u32 = 0xf000_0000;

const REPLY_TIMEOUT: Duration = Duration::from_secs(1);

/// The active preset register: bank, bank, letter, 01, with letter FF in bypass
/// (1A = 00 00 00 01, 3C = 02 02 02 01, bypass on 3B = 02 02 FF 01). Which of the
/// two bank bytes is the one shown while BANK± flashes is not known, so they
/// have to agree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Register {
    pub bank: usize,
    pub slot: Option<usize>,
    pub bypassed: bool,
}

pub fn decode_register(r: &[u8]) -> Option<Register> {
    if r.len() != 4 || r[0] > 8 || r[0] != r[1] {
        return None;
    }
    let bank = r[0] as usize;
    if r[2] == 0xff {
        return Some(Register {
            bank,
            slot: None,
            bypassed: true,
        });
    }
    if r[2] > 3 {
        return None;
    }
    Some(Register {
        bank,
        slot: Some(bank * 4 + r[2] as usize),
        bypassed: false,
    })
}

pub fn slot_address(slot: usize) -> u32 {
    2 + (slot * preset::SIZE) as u32
}

pub struct TankG {
    port: Box<dyn Port>,
    pub version: String,
    /// The preset sounding now, when known; None means the client cannot tell, and then it edits nothing.
    pub sounding: Option<usize>,
    /// In bypass the live block reads all zero, so nothing is edited until a preset is loaded again.
    pub bypassed: bool,
}

impl TankG {
    pub fn connect(port: Box<dyn Port>) -> Result<Self> {
        let name = port.name();
        let mut p = TankG {
            port,
            version: String::new(),
            sounding: None,
            bypassed: false,
        };
        let reply = p.transact(&frame::version(), frame::OP_VERSION, None)?;
        p.version = String::from_utf8_lossy(&reply)
            .trim_end_matches('\0')
            .to_string();
        if !p.version.starts_with("TANK-G") {
            bail!("{name} is not a Tank-G (it answered “{}”)", p.version);
        }
        Ok(p)
    }

    pub fn read_register(&mut self) -> Result<Vec<u8>> {
        self.read(REGISTER, 4)
    }

    pub fn read_live(&mut self) -> Result<Preset> {
        Ok(self.read(LIVE, preset::SIZE)?.try_into().unwrap())
    }

    #[cfg(test)]
    pub fn read_slot(&mut self, slot: usize) -> Result<Preset> {
        Ok(self
            .read(slot_address(slot), preset::SIZE)?
            .try_into()
            .unwrap())
    }

    pub fn read_table(&mut self) -> Result<Vec<u8>> {
        let t = self.read(0, preset::TABLE_SIZE)?;
        if !preset::table_ok(&t) {
            bail!("the preset table does not start with A5 5A");
        }
        Ok(t)
    }

    pub fn load(&mut self, slot: usize) -> Result<()> {
        check_slot(slot)?;
        self.write(LOAD_PRESET + slot as u32, &[1])?;
        self.sounding = Some(slot);
        self.bypassed = false;
        Ok(())
    }

    pub fn bypass(&mut self) -> Result<()> {
        let slot = self.sounding.context("the playing preset is not known")?;
        self.write(BYPASS_PRESET + slot as u32, &[1])?;
        self.bypassed = true;
        Ok(())
    }

    /// AMP, MOD, DLY and REV on/off, bytes 0–3 of the sounding preset.
    pub fn set_switch(&mut self, offset: usize, on: bool) -> Result<()> {
        if offset > 3 {
            bail!("switch offset {offset} out of range");
        }
        self.editable()?;
        self.write(LIVE + offset as u32, &[on as u8])
    }

    /// GATE through CAB IR of the sounding preset in one frame, as the vendor editor sends every knob turn.
    pub fn write_knobs(&mut self, p: &Preset) -> Result<()> {
        self.editable()?;
        self.write(
            LIVE + preset::KNOBS_OFFSET as u32,
            &p[preset::KNOBS_OFFSET..preset::KNOBS_OFFSET + preset::KNOBS_LEN],
        )
    }

    /// Stores a block without committing; `commit` makes the stored presets stick.
    pub fn store(&mut self, slot: usize, p: &Preset) -> Result<()> {
        check_slot(slot)?;
        // what the live block reads in bypass; never a preset anyone meant to keep
        if p.iter().all(|b| *b == 0) {
            bail!("refusing to store an all-zero preset");
        }
        self.write(slot_address(slot), p)
    }

    pub fn commit(&mut self) -> Result<()> {
        self.write(COMMIT, &[])
    }

    /// The vendor editor's save: store, commit, then load the slot so it sounds as stored.
    pub fn save(&mut self, slot: usize, p: &Preset) -> Result<()> {
        self.store(slot, p)?;
        self.commit()?;
        self.load(slot)
    }

    fn editable(&self) -> Result<usize> {
        let slot = self.sounding.context("the playing preset is not known")?;
        if self.bypassed {
            bail!("bypassed: load the preset again to edit it");
        }
        Ok(slot)
    }

    fn read(&mut self, address: u32, size: usize) -> Result<Vec<u8>> {
        let data = self.transact(
            &frame::read(SPACE, address, size),
            frame::OP_READ,
            Some(address),
        )?;
        if data.len() != size {
            bail!("read of {address:08x}: got {} of {size} bytes", data.len());
        }
        Ok(data)
    }

    fn write(&mut self, address: u32, data: &[u8]) -> Result<()> {
        // whatever the caller meant: the table's `A5 5A` header is never written
        if address < slot_address(0) {
            bail!("write refused: it would reach the preset table header");
        }
        self.transact(&frame::write(SPACE, address, data), frame::OP_ACK, None)?;
        Ok(())
    }

    /// Sends one frame and waits for the reply with `op` (and the same address, for reads).
    fn transact(&mut self, f: &[u8], op: u8, address: Option<u32>) -> Result<Vec<u8>> {
        self.port.discard();
        self.port.send(&frame::to_sysex(f))?;
        let deadline = Instant::now() + REPLY_TIMEOUT;
        while Instant::now() < deadline {
            let Some(msg) = self
                .port
                .receive(deadline.saturating_duration_since(Instant::now()))?
            else {
                break;
            };
            let Some((reply_op, payload)) = frame::parse(&frame::unpack(&msg)) else {
                continue;
            };
            if reply_op != op {
                continue;
            }
            if op == frame::OP_READ {
                match frame::parse_memory(&payload) {
                    Some(m) if Some(m.address) == address => return Ok(m.data),
                    _ => continue,
                }
            }
            return Ok(payload);
        }
        bail!("the pedal did not answer")
    }
}

fn check_slot(slot: usize) -> Result<()> {
    if slot >= preset::COUNT {
        bail!("slot {slot} out of range");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pedal::port::Simulated;

    #[test]
    fn registers() {
        assert_eq!(decode_register(&[0, 0, 0, 1]).unwrap().slot, Some(0));
        assert_eq!(decode_register(&[2, 2, 2, 1]).unwrap().slot, Some(10));
        assert!(decode_register(&[2, 2, 0xff, 1]).unwrap().bypassed);
        assert!(decode_register(&[2, 1, 0, 1]).is_none());
    }

    #[test]
    fn talks_to_the_simulator() {
        let mut p = TankG::connect(Box::new(Simulated::new())).unwrap();
        assert!(p.version.starts_with("TANK-G"));
        let table = p.read_table().unwrap();
        p.load(5).unwrap();
        let mut live = p.read_live().unwrap();
        assert_eq!(live, preset::from_table(&table, 5));
        assert_eq!(
            decode_register(&p.read_register().unwrap()).unwrap().slot,
            Some(5)
        );
        live[6] = 99;
        p.write_knobs(&live).unwrap();
        assert_eq!(p.read_live().unwrap()[6], 99);
        p.save(5, &live).unwrap();
        assert_eq!(p.read_slot(5).unwrap()[6], 99);
        p.bypass().unwrap();
        assert!(p.read_live().unwrap().iter().all(|b| *b == 0));
        assert!(p.write_knobs(&live).is_err());
    }
}
