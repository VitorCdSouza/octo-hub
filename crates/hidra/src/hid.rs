//! Linux hidraw transport for the VIA raw HID interface.

use anyhow::{Context, Result, bail};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const REPORT_LEN: usize = 32;

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub path: PathBuf,
    pub vendor_id: u16,
    pub product_id: u16,
    pub name: String,
}

impl DeviceInfo {
    pub fn vendor_product_id(&self) -> u32 {
        ((self.vendor_id as u32) << 16) | self.product_id as u32
    }
}

/// Lists every hidraw node that exposes the QMK raw HID usage (page 0xFF60, usage 0x61).
pub fn enumerate() -> Vec<DeviceInfo> {
    scan(|_, desc| has_collection(desc, 0xFF60, 0x61))
}

/// Lists keyboards connected over Bluetooth that have no raw HID interface.
/// Wireless boards like the GMK70 only speak VIA over the cable, so these are
/// worth naming when nothing else is found.
pub fn bluetooth_keyboards() -> Vec<DeviceInfo> {
    scan(|bus, desc| bus == BUS_BLUETOOTH && has_collection(desc, 0x01, 0x06) && !has_collection(desc, 0xFF60, 0x61))
}

const BUS_BLUETOOTH: u16 = 0x0005;

fn scan(keep: impl Fn(u16, &[u8]) -> bool) -> Vec<DeviceInfo> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/class/hidraw") else {
        return out;
    };
    for entry in entries.flatten() {
        let sys = entry.path();
        let Ok(desc) = fs::read(sys.join("device/report_descriptor")) else {
            continue;
        };
        let Ok(uevent) = fs::read_to_string(sys.join("device/uevent")) else {
            continue;
        };
        let mut bus = 0;
        let mut info = DeviceInfo {
            path: Path::new("/dev").join(entry.file_name()),
            vendor_id: 0,
            product_id: 0,
            name: String::new(),
        };
        for line in uevent.lines() {
            if let Some(id) = line.strip_prefix("HID_ID=") {
                // format: bus:vendor:product, each in hex
                let parts: Vec<&str> = id.split(':').collect();
                if parts.len() == 3 {
                    bus = u32::from_str_radix(parts[0], 16).unwrap_or(0) as u16;
                    info.vendor_id = u32::from_str_radix(parts[1], 16).unwrap_or(0) as u16;
                    info.product_id = u32::from_str_radix(parts[2], 16).unwrap_or(0) as u16;
                }
            } else if let Some(name) = line.strip_prefix("HID_NAME=") {
                info.name = name.to_string();
            }
        }
        if keep(bus, &desc) {
            out.push(info);
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// Walks the report descriptor and looks for an application collection with
/// the given usage page and usage.
fn has_collection(desc: &[u8], page: u32, wanted: u32) -> bool {
    let mut i = 0;
    let mut usage_page: u32 = 0;
    let mut usage: u32 = 0;
    while i < desc.len() {
        let prefix = desc[i];
        if prefix == 0xFE {
            // long item: skip it entirely
            let size = *desc.get(i + 1).unwrap_or(&0) as usize;
            i += 3 + size;
            continue;
        }
        let size = match prefix & 0x03 {
            0 => 0,
            1 => 1,
            2 => 2,
            _ => 4,
        };
        let mut value: u32 = 0;
        for b in 0..size {
            value |= (*desc.get(i + 1 + b).unwrap_or(&0) as u32) << (8 * b);
        }
        match prefix & 0xFC {
            0x04 => usage_page = value,
            0x08 => usage = value,
            0xA0 => {
                if usage_page == page && usage == wanted {
                    return true;
                }
            }
            _ => {}
        }
        i += 1 + size;
    }
    false
}

pub struct HidDevice {
    file: File,
}

impl HidDevice {
    pub fn open(info: &DeviceInfo) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&info.path)
            .with_context(|| {
                format!(
                    "could not open {} (is your user allowed to access hidraw? see README)",
                    info.path.display()
                )
            })?;
        Ok(Self { file })
    }

    /// Drops any input reports that arrived while nobody was listening.
    pub fn drain(&mut self) {
        let mut buf = [0u8; 64];
        while let Ok(n) = self.file.read(&mut buf) {
            if n == 0 {
                break;
            }
        }
    }

    pub fn write_report(&mut self, data: &[u8]) -> Result<()> {
        if data.len() > REPORT_LEN {
            bail!("report longer than {REPORT_LEN} bytes");
        }
        // first byte is the report number; the VIA interface does not use numbered reports
        let mut buf = [0u8; REPORT_LEN + 1];
        buf[1..1 + data.len()].copy_from_slice(data);
        self.file.write_all(&buf).context("hid write failed")?;
        Ok(())
    }

    pub fn read_report(&mut self, timeout: Duration) -> Result<Option<[u8; REPORT_LEN]>> {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let mut pfd = libc::pollfd {
                fd: self.file.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let ms = left.as_millis().min(i32::MAX as u128) as i32;
            // SAFETY: pfd is a valid pollfd for the lifetime of the call
            let rc = unsafe { libc::poll(&mut pfd, 1, ms) };
            if rc < 0 {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(err).context("poll failed");
            }
            if rc == 0 {
                return Ok(None);
            }
            if pfd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
                bail!("device disconnected");
            }
            let mut buf = [0u8; 64];
            match self.file.read(&mut buf) {
                Ok(0) => bail!("device disconnected"),
                Ok(n) => {
                    let mut out = [0u8; REPORT_LEN];
                    let n = n.min(REPORT_LEN);
                    out[..n].copy_from_slice(&buf[..n]);
                    return Ok(Some(out));
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e).context("hid read failed"),
            }
        }
    }
}
