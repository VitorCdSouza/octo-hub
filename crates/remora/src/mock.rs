//! An in-memory X6 for `--demo`: it acknowledges every valid report and sends
//! a heartbeat every two seconds, like the real receiver.

use crate::link::Backend;
use anyhow::{Result, bail};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub struct DemoMouse {
    pending: VecDeque<Vec<u8>>,
    next_beat: Instant,
}

impl DemoMouse {
    pub fn new() -> Self {
        DemoMouse {
            pending: VecDeque::new(),
            next_beat: Instant::now(),
        }
    }
}

impl Backend for DemoMouse {
    fn send_feature(&mut self, report: &[u8]) -> Result<()> {
        let expected = match report[0] {
            0x04 => 52,
            0x05 => 13,
            0x06 => 9,
            0x08 => 59,
            id => bail!("demo mouse has no report {id:02x}"),
        };
        if report.len() != expected {
            bail!("report {:02x} is {} bytes, want {expected}", report[0], report.len());
        }
        self.pending.push_back(vec![0x03, 0x10, 0x50, 0x00, report[0]]);
        Ok(())
    }

    fn read(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>> {
        if let Some(r) = self.pending.pop_front() {
            std::thread::sleep(Duration::from_millis(18));
            return Ok(Some(r));
        }
        let wait = self.next_beat.saturating_duration_since(Instant::now());
        if wait > timeout {
            std::thread::sleep(timeout);
            return Ok(None);
        }
        std::thread::sleep(wait);
        self.next_beat = Instant::now() + Duration::from_secs(2);
        Ok(Some(vec![0x03, 0x10, 0x40, 0x01, 0x07]))
    }

    fn drain(&mut self) {
        self.pending.clear();
    }
}
