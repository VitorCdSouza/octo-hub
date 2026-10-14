//! HID keyboard usages a shortcut binding can send.

/// (usage, name) for every key the shortcut picker offers.
pub fn table() -> Vec<(u8, String)> {
    let mut t = Vec::new();
    for (i, c) in ('A'..='Z').enumerate() {
        t.push((0x04 + i as u8, c.to_string()));
    }
    for (i, c) in "1234567890".chars().enumerate() {
        t.push((0x1e + i as u8, c.to_string()));
    }
    for (u, n) in [
        (0x28, "Enter"),
        (0x29, "Esc"),
        (0x2a, "Backspace"),
        (0x2b, "Tab"),
        (0x2c, "Space"),
        (0x2d, "-"),
        (0x2e, "="),
        (0x2f, "["),
        (0x30, "]"),
        (0x31, "\\"),
        (0x33, ";"),
        (0x34, "'"),
        (0x35, "`"),
        (0x36, ","),
        (0x37, "."),
        (0x38, "/"),
        (0x39, "Caps Lock"),
    ] {
        t.push((u, n.to_string()));
    }
    for i in 0..12u8 {
        t.push((0x3a + i, format!("F{}", i + 1)));
    }
    for (u, n) in [
        (0x46, "Print Screen"),
        (0x47, "Scroll Lock"),
        (0x48, "Pause"),
        (0x49, "Insert"),
        (0x4a, "Home"),
        (0x4b, "Page Up"),
        (0x4c, "Delete"),
        (0x4d, "End"),
        (0x4e, "Page Down"),
        (0x4f, "Right"),
        (0x50, "Left"),
        (0x51, "Down"),
        (0x52, "Up"),
    ] {
        t.push((u, n.to_string()));
    }
    for i in 0..12u8 {
        t.push((0x68 + i, format!("F{}", i + 13)));
    }
    t
}

pub fn name(usage: u8) -> String {
    table()
        .into_iter()
        .find(|(u, _)| *u == usage)
        .map(|(_, n)| n)
        .unwrap_or_else(|| format!("key {usage:02x}"))
}

/// Parses a name from `table`, case-insensitive.
pub fn from_name(s: &str) -> Option<u8> {
    let s = s.trim();
    table().into_iter().find(|(_, n)| n.eq_ignore_ascii_case(s) || n.replace(' ', "").eq_ignore_ascii_case(s)).map(|(u, _)| u)
}
