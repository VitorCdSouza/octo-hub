//! The pedal's USB MIDI port and the simulated pedal behind the same trait.
//!
//! The real port is the ALSA rawmidi device node (`/dev/snd/midiCnD0`), opened
//! non-blocking. That is what `snd_rawmidi_open("hw:n,0,0")` opens underneath,
//! so no libasound is needed. The pedal only ever answers requests, so reads
//! happen inline while a request waits for its reply.

use super::frame;
use super::preset::{self, Preset};
use anyhow::{Context, Result, bail};
use std::fs::File;
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// What the client needs from a MIDI port: whole SysEx messages out and in.
pub trait Port: Send {
    fn name(&self) -> String;
    fn send(&mut self, sysex: &[u8]) -> Result<()>;
    /// The next complete SysEx message, without F0 and F7, or None when none arrives in time.
    fn receive(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>>;
    /// Drops whatever came in unasked, so the next reply is the one to the next request.
    fn discard(&mut self);
}

/// The card whose name has "USB Composite Device" and that carries a MIDI port.
/// `OCTO_PEDAL_PORT` (a device path) overrides the search.
pub fn find() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("OCTO_PEDAL_PORT").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    // " 4 [Device         ]: USB-Audio - USB Composite Device", then a line with the long name
    let cards = std::fs::read_to_string("/proc/asound/cards").ok()?;
    let mut card = None;
    for line in cards.lines() {
        let head = line.trim_start();
        if let Some((n, rest)) = head.split_once(' ')
            && rest.trim_start().starts_with('[')
        {
            card = n.parse::<u32>().ok();
        }
        if let Some(n) = card
            && line.to_lowercase().contains("usb composite device")
            && std::path::Path::new(&format!("/proc/asound/card{n}/midi0")).exists()
        {
            return Some(PathBuf::from(format!("/dev/snd/midiC{n}D0")));
        }
    }
    None
}

pub struct RawMidi {
    file: File,
    path: PathBuf,
    pending: Vec<u8>,
}

impl RawMidi {
    pub fn open(path: PathBuf) -> Result<Self> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&path)
            .map_err(|e| {
                if e.raw_os_error() == Some(libc::EBUSY) {
                    anyhow::anyhow!(
                        "{} is in use by another app, maybe TabJam's pedal panel",
                        path.display()
                    )
                } else {
                    anyhow::anyhow!("{}: {e}", path.display())
                }
            })?;
        Ok(RawMidi {
            file,
            path,
            pending: vec![],
        })
    }

    /// Reads what is waiting; false when nothing was.
    fn fill(&mut self) -> Result<bool> {
        let mut buf = [0u8; 1024];
        match self.file.read(&mut buf) {
            Ok(0) => Ok(false),
            // realtime bytes may sit anywhere, even inside a SysEx; they carry nothing here
            Ok(n) => {
                self.pending.extend(buf[..n].iter().filter(|b| **b < 0xf8));
                Ok(true)
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => Ok(false),
            Err(e) => Err(e).with_context(|| self.path.display().to_string()),
        }
    }

    fn take_message(&mut self) -> Option<Vec<u8>> {
        take_message(&mut self.pending)
    }
}

/// The first whole SysEx in `pending`; a message cut short is dropped, since the
/// last F0 before the F7 starts the whole one.
fn take_message(pending: &mut Vec<u8>) -> Option<Vec<u8>> {
    let Some(start) = pending.iter().position(|b| *b == 0xf0) else {
        pending.clear();
        return None;
    };
    let Some(end) = pending[start + 1..]
        .iter()
        .position(|b| *b == 0xf7)
        .map(|e| e + start + 1)
    else {
        pending.drain(..start);
        return None;
    };
    let start = pending[..end]
        .iter()
        .rposition(|b| *b == 0xf0)
        .unwrap_or(start);
    let msg = pending[start + 1..end].to_vec();
    pending.drain(..=end);
    Some(msg)
}

impl Port for RawMidi {
    fn name(&self) -> String {
        self.path.display().to_string()
    }

    fn send(&mut self, sysex: &[u8]) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut sent = 0;
        while sent < sysex.len() {
            match self.file.write(&sysex[sent..]) {
                Ok(n) => sent += n,
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    if Instant::now() > deadline {
                        bail!("the MIDI port did not take the message");
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(e) => return Err(e).with_context(|| self.path.display().to_string()),
            }
        }
        Ok(())
    }

    fn receive(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(m) = self.take_message() {
                return Ok(Some(m));
            }
            if !self.fill()? {
                if Instant::now() >= deadline {
                    return Ok(None);
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    fn discard(&mut self) {
        while matches!(self.fill(), Ok(true)) {}
        self.pending.clear();
    }
}

/// A Tank-G in memory, answering the frames the real one answered in the
/// vendor editor's captures. Ported from TabJam's `SimulatedTankG.cs`.
pub struct Simulated {
    table: Vec<u8>,
    live: Preset,
    sounding: usize,
    bypassed: bool,
    replies: std::collections::VecDeque<Vec<u8>>,
}

impl Simulated {
    pub fn new() -> Self {
        let mut s = Simulated {
            table: factory_like(),
            live: [0; preset::SIZE],
            sounding: 0,
            bypassed: false,
            replies: Default::default(),
        };
        s.load(0);
        s
    }

    fn load(&mut self, slot: usize) {
        self.sounding = slot;
        self.bypassed = false;
        self.live = preset::from_table(&self.table, slot);
    }

    /// Bank, bank, letter, 01; the letter is FF in bypass.
    fn register(&self) -> Vec<u8> {
        let bank = (self.sounding / 4) as u8;
        vec![
            bank,
            bank,
            if self.bypassed {
                0xff
            } else {
                (self.sounding % 4) as u8
            },
            1,
        ]
    }

    fn handle(&mut self, op: u8, payload: &[u8]) -> Option<Vec<u8>> {
        if op == frame::OP_VERSION {
            let mut text = vec![0u8; 27];
            text[..12].copy_from_slice(b"TANK-Gv2_097");
            return Some(frame::build(frame::OP_VERSION, &text));
        }
        let m = frame::parse_memory(payload)?;
        if op == frame::OP_READ {
            let a = m.address as usize;
            let data = if m.space != 4 {
                vec![0; m.size]
            } else {
                match m.address {
                    // in bypass the real pedal reads the live block as zeros
                    0x1000_0000 if self.bypassed => vec![0; m.size.min(preset::SIZE)],
                    0x1000_0000 => self.live[..m.size.min(preset::SIZE)].to_vec(),
                    0x2000_0000 => self.register(),
                    0 => self.table[..m.size.min(self.table.len())].to_vec(),
                    _ if a >= 2 && a + m.size <= self.table.len() => {
                        self.table[a..a + m.size].to_vec()
                    }
                    _ => vec![0; m.size],
                }
            };
            let mut p = frame::memory_header(m.space, m.address, data.len()).to_vec();
            p.extend(data);
            return Some(frame::build(frame::OP_READ, &p));
        }
        if op != frame::OP_WRITE {
            return None;
        }
        if m.space == 4 {
            let a = m.address as usize;
            match m.address {
                0x1000_0000..0x1000_0015 => {
                    let o = a - 0x1000_0000;
                    let n = m.data.len().min(preset::SIZE - o);
                    self.live[o..o + n].copy_from_slice(&m.data[..n]);
                }
                0xe000_0000..0xe000_0024 => self.load(a - 0xe000_0000),
                0xe100_0000..0xe100_0024 => self.bypassed = true,
                0xf000_0000 => {}
                _ if a + m.data.len() <= self.table.len() => {
                    self.table[a..a + m.data.len()].copy_from_slice(&m.data)
                }
                _ => {}
            }
        }
        Some(vec![0x00, 0x59, 0x00, 0x01, 0x00, 0x00, 0x00, 0xff])
    }
}

/// Presets shaped like the factory ones: amp on, delay and reverb on, a model and IR per bank.
fn factory_like() -> Vec<u8> {
    let mut t = vec![0u8; preset::TABLE_SIZE];
    t[0] = 0xa5;
    t[1] = 0x5a;
    let mut seed = 97u32;
    let mut rng = |lo: u32, hi: u32| {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        (lo + (seed >> 16) % (hi - lo)) as u8
    };
    for slot in 0..preset::COUNT {
        let p: Preset = [
            1,
            (slot % 4 == 1) as u8,
            1,
            1,
            if slot % 3 == 2 { 20 + slot as u8 } else { 0 },
            (slot % 9) as u8,
            rng(40, 115),
            rng(40, 100),
            rng(40, 100),
            rng(40, 100),
            rng(50, 100),
            (slot % 3) as u8,
            rng(10, 90),
            6,
            35,
            5,
            1,
            (slot / 4 % 9) as u8,
            0xff,
            0,
            0,
        ];
        t[2 + slot * preset::SIZE..2 + (slot + 1) * preset::SIZE].copy_from_slice(&p);
    }
    t
}

impl Port for Simulated {
    fn name(&self) -> String {
        "simulated pedal".into()
    }

    fn send(&mut self, sysex: &[u8]) -> Result<()> {
        if sysex.len() < 2 || sysex[0] != 0xf0 || sysex[sysex.len() - 1] != 0xf7 {
            return Ok(());
        }
        let raw = frame::unpack(&sysex[1..sysex.len() - 1]);
        if let Some((op, payload)) = frame::parse(&raw)
            && let Some(reply) = self.handle(op, &payload)
        {
            self.replies.push_back(frame::pack(&reply));
        }
        Ok(())
    }

    fn receive(&mut self, _timeout: Duration) -> Result<Option<Vec<u8>>> {
        Ok(self.replies.pop_front())
    }

    fn discard(&mut self) {
        self.replies.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sysex_splitting() {
        let mut p = vec![0x01, 0xf0, 0x10, 0xf0, 0x20, 0x21, 0xf7, 0xf0, 0x30];
        assert_eq!(take_message(&mut p), Some(vec![0x20, 0x21]));
        assert_eq!(p, vec![0xf0, 0x30]);
        assert_eq!(take_message(&mut p), None);
        assert_eq!(p, vec![0xf0, 0x30]);
    }
}
