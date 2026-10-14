//! The M-Vave frame, shared with the BlackBox: `00 59 op len[3] payload checksum`,
//! sent as one SysEx with its bytes packed base-128 (a continuous LSB-first bit
//! stream). Ported from TabJam's `MvaveFrame.cs`; the map of the Tank-G is in
//! TabJam's `tools/tank-g/README.md`.

pub const OP_ACK: u8 = 0x00;
pub const OP_VERSION: u8 = 0x11;
pub const OP_WRITE: u8 = 0x22;
pub const OP_READ: u8 = 0x23;

pub fn pack(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len() * 8 / 7 + 1);
    let (mut acc, mut bits) = (0u32, 0u32);
    for &b in raw {
        acc |= (b as u32) << bits;
        bits += 8;
        while bits >= 7 {
            out.push((acc & 0x7f) as u8);
            acc >>= 7;
            bits -= 7;
        }
    }
    if bits > 0 {
        out.push((acc & 0x7f) as u8);
    }
    out
}

pub fn unpack(packed: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(packed.len() * 7 / 8);
    let (mut acc, mut bits) = (0u32, 0u32);
    for &b in packed {
        acc |= ((b & 0x7f) as u32) << bits;
        bits += 7;
        while bits >= 8 {
            out.push((acc & 0xff) as u8);
            acc >>= 8;
            bits -= 8;
        }
    }
    out
}

pub fn checksum(payload: &[u8]) -> u8 {
    let sum: u32 = payload.iter().map(|&b| b as u32).sum();
    0xff - (sum & 0xff) as u8
}

pub fn build(op: u8, payload: &[u8]) -> Vec<u8> {
    let n = payload.len();
    let mut f = vec![0x00, 0x59, op, n as u8, (n >> 8) as u8, (n >> 16) as u8];
    f.extend_from_slice(payload);
    f.push(checksum(payload));
    f
}

pub fn to_sysex(frame: &[u8]) -> Vec<u8> {
    let mut s = vec![0xf0];
    s.extend(pack(frame));
    s.push(0xf7);
    s
}

pub fn version() -> Vec<u8> {
    build(OP_VERSION, &[])
}

pub fn read(space: u8, address: u32, size: usize) -> Vec<u8> {
    build(OP_READ, &memory_header(space, address, size))
}

pub fn write(space: u8, address: u32, data: &[u8]) -> Vec<u8> {
    let mut p = memory_header(space, address, data.len()).to_vec();
    p.extend_from_slice(data);
    build(OP_WRITE, &p)
}

/// Space, 32-bit address and 24-bit size, all little endian: the head of every read and write.
pub fn memory_header(space: u8, address: u32, size: usize) -> [u8; 8] {
    let a = address.to_le_bytes();
    [
        space,
        a[0],
        a[1],
        a[2],
        a[3],
        size as u8,
        (size >> 8) as u8,
        (size >> 16) as u8,
    ]
}

/// A raw frame with the right magic, length and checksum; anything else is noise.
pub fn parse(frame: &[u8]) -> Option<(u8, Vec<u8>)> {
    if frame.len() < 7 || frame[0] != 0x00 || frame[1] != 0x59 {
        return None;
    }
    let len = frame[3] as usize | (frame[4] as usize) << 8 | (frame[5] as usize) << 16;
    if frame.len() < 6 + len + 1 {
        return None;
    }
    let body = &frame[6..6 + len];
    if checksum(body) != frame[6 + len] {
        return None;
    }
    Some((frame[2], body.to_vec()))
}

pub struct Memory {
    pub space: u8,
    pub address: u32,
    pub size: usize,
    pub data: Vec<u8>,
}

pub fn parse_memory(payload: &[u8]) -> Option<Memory> {
    if payload.len() < 8 {
        return None;
    }
    let address = u32::from_le_bytes([payload[1], payload[2], payload[3], payload[4]]);
    let size = payload[5] as usize | (payload[6] as usize) << 8 | (payload[7] as usize) << 16;
    let data = &payload[8..];
    Some(Memory {
        space: payload[0],
        address,
        size,
        data: data[..data.len().min(size)].to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_round_trips() {
        let raw: Vec<u8> = (0..=255).collect();
        assert_eq!(unpack(&pack(&raw))[..raw.len()], raw[..]);
        assert!(pack(&raw).iter().all(|b| *b < 0x80));
    }

    #[test]
    fn frame_parses_back() {
        let f = write(4, 0x1000_0004, &[1, 2, 3]);
        let (op, payload) = parse(&f).unwrap();
        assert_eq!(op, OP_WRITE);
        let m = parse_memory(&payload).unwrap();
        assert_eq!(
            (m.space, m.address, m.size, m.data),
            (4, 0x1000_0004, 3, vec![1, 2, 3])
        );
    }

    #[test]
    fn bad_checksum_is_noise() {
        let mut f = version();
        *f.last_mut().unwrap() ^= 1;
        assert!(parse(&f).is_none());
    }
}
