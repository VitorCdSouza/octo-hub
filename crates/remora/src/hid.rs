//! Linux hidraw transport for the receiver's configuration interface.

use crate::proto;
use anyhow::{Context, Result, bail};
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub path: PathBuf,
    pub name: String,
    /// USB topology path, e.g. `usb-0000:00:14.0-1/input2`
    pub phys: String,
}

/// Lists the hidraw nodes of X6 receivers that carry the configuration reports.
pub fn enumerate() -> Vec<DeviceInfo> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/class/hidraw") else {
        return out;
    };
    for entry in entries.flatten() {
        let sys = entry.path();
        let Ok(uevent) = fs::read_to_string(sys.join("device/uevent")) else {
            continue;
        };
        let mut ids = None;
        let mut name = String::new();
        let mut phys = String::new();
        for line in uevent.lines() {
            if let Some(id) = line.strip_prefix("HID_ID=") {
                // format: bus:vendor:product, each in hex
                let parts: Vec<u32> = id.split(':').filter_map(|p| u32::from_str_radix(p, 16).ok()).collect();
                if parts.len() == 3 {
                    ids = Some((parts[1] as u16, parts[2] as u16));
                }
            } else if let Some(n) = line.strip_prefix("HID_NAME=") {
                name = n.to_string();
            } else if let Some(p) = line.strip_prefix("HID_PHYS=") {
                phys = p.to_string();
            }
        }
        if ids != Some((proto::VENDOR_ID, proto::PRODUCT_ID)) {
            continue;
        }
        let Ok(desc) = fs::read(sys.join("device/report_descriptor")) else {
            continue;
        };
        if !is_config_interface(&desc) {
            continue;
        }
        out.push(DeviceInfo {
            path: Path::new("/dev").join(entry.file_name()),
            name,
            phys,
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// The configuration interface declares feature reports 0x04, 0x05, 0x06 and 0x08.
fn is_config_interface(desc: &[u8]) -> bool {
    let mut features = Vec::new();
    let mut report_id = 0u32;
    let mut i = 0;
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
            0x84 => report_id = value,
            0xB0 => features.push(report_id),
            _ => {}
        }
        i += 1 + size;
    }
    [proto::SENSOR_REPORT, proto::LIGHT_REPORT, proto::POLLING_REPORT, proto::BUTTONS_REPORT]
        .iter()
        .all(|id| features.contains(&(*id as u32)))
}

pub struct HidDevice {
    file: File,
}

/// HIDIOCSFEATURE(len): _IOC(_IOC_WRITE | _IOC_READ, 'H', 0x06, len)
fn hidiocsfeature(len: usize) -> libc::c_ulong {
    ((3 << 30) | ((len as libc::c_ulong) << 16) | ((b'H' as libc::c_ulong) << 8) | 0x06) as libc::c_ulong
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

    /// Sends a feature report; the first byte is the report id.
    pub fn send_feature(&mut self, report: &[u8]) -> Result<()> {
        let mut buf = report.to_vec();
        // SAFETY: buf outlives the call and the ioctl size matches its length
        let rc = unsafe { libc::ioctl(self.file.as_raw_fd(), hidiocsfeature(buf.len()) as _, buf.as_mut_ptr()) };
        if rc < 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::ENODEV) {
                bail!("device disconnected");
            }
            return Err(err).with_context(|| format!("SET_FEATURE {:02x} failed", report[0]));
        }
        if rc as usize != buf.len() {
            bail!("SET_FEATURE {:02x} wrote {rc} of {} bytes", report[0], buf.len());
        }
        Ok(())
    }

    /// Waits up to `timeout` for one input report.
    pub fn read(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>> {
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
                Ok(n) => return Ok(Some(buf[..n].to_vec())),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) if e.raw_os_error() == Some(libc::ENODEV) => bail!("device disconnected"),
                Err(e) => return Err(e).context("hid read failed"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn finds_config_interface_in_x6_descriptor() {
        // interface 2 of the receiver, trimmed after report 0x08
        let desc = [
            0x05, 0x01, 0x09, 0x80, 0xa1, 0x01, 0x85, 0x01, 0x05, 0x01, 0x19, 0x81, 0x29, 0x83, 0x15, 0x00, 0x25, 0x01, 0x95, 0x03,
            0x75, 0x01, 0x81, 0x06, 0x95, 0x01, 0x75, 0x05, 0x81, 0x01, 0xc0, 0x05, 0x0b, 0x09, 0x00, 0xa1, 0x01, 0x85, 0x04, 0x95,
            0x33, 0xb1, 0x01, 0x85, 0x05, 0x95, 0x0c, 0xb1, 0x01, 0x85, 0x06, 0x95, 0x08, 0xb1, 0x01, 0x85, 0x07, 0x95, 0x07, 0xb1,
            0x01, 0x85, 0x08, 0x95, 0x3a, 0xb1, 0x01, 0xc0,
        ];
        assert!(super::is_config_interface(&desc));
        // the mouse interface has no feature reports
        assert!(!super::is_config_interface(&[0x05, 0x01, 0x09, 0x02, 0xa1, 0x01, 0xc0]));
    }
}
