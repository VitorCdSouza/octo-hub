//! Dynamic macro buffer (VIA protocol 11+) and VIA's `{KC_X}` text expressions.
//!
//! Ported from the-via/app src/utils/macro-api/macro-api.v11.ts and macro-api.common.ts.

use crate::keycode;
use anyhow::{Result, bail};

const PREFIX: u8 = 0x01;
const TAP: u8 = 0x01;
const DOWN: u8 = 0x02;
const UP: u8 = 0x03;
const DELAY: u8 = 0x04;
const DELAY_END: u8 = b'|';

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Tap(u8),
    Down(u8),
    Up(u8),
    Delay(u32),
    Text(String),
    Chord(Vec<u8>),
}

/// Splits the raw buffer into `count` macros. Unknown bytes end parsing early,
/// the way VIA does, so a corrupt buffer never yields garbage keycodes.
pub fn parse_buffer(bytes: &[u8], count: usize) -> Vec<Vec<Action>> {
    let mut out: Vec<Vec<Action>> = Vec::new();
    let mut cur: Vec<Action> = Vec::new();
    let mut i = 0;
    while i < bytes.len() && out.len() < count {
        let b = bytes[i];
        match b {
            0 => out.push(std::mem::take(&mut cur)),
            PREFIX => {
                i += 1;
                match bytes.get(i).copied() {
                    Some(TAP) | Some(DOWN) | Some(UP) => {
                        let kind = bytes[i];
                        i += 1;
                        let kc = bytes.get(i).copied().unwrap_or(0);
                        cur.push(match kind {
                            TAP => Action::Tap(kc),
                            DOWN => Action::Down(kc),
                            _ => Action::Up(kc),
                        });
                    }
                    Some(DELAY) => {
                        i += 1;
                        let mut digits = String::new();
                        while i < bytes.len() && bytes[i] != DELAY_END {
                            digits.push(bytes[i] as char);
                            i += 1;
                        }
                        cur.push(Action::Delay(digits.parse().unwrap_or(0)));
                    }
                    _ => break,
                }
            }
            _ => match cur.last_mut() {
                Some(Action::Text(s)) => s.push(b as char),
                _ => cur.push(Action::Text((b as char).to_string())),
            },
        }
        i += 1;
    }
    while out.len() < count {
        out.push(Vec::new());
    }
    out
}

/// Length of the prefix that holds all `count` macros, or None if the buffer
/// ends first. Walks the same grammar as `parse_buffer`, so a 0x00 keycode
/// inside a tap is not mistaken for a terminator.
pub fn used_len(bytes: &[u8], count: usize) -> Option<usize> {
    let mut seen = 0;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            0 => {
                seen += 1;
                if seen == count {
                    return Some(i + 1);
                }
            }
            PREFIX => match bytes.get(i + 1).copied() {
                Some(TAP) | Some(DOWN) | Some(UP) => i += 2,
                Some(DELAY) => {
                    i += 2;
                    while i < bytes.len() && bytes[i] != DELAY_END {
                        i += 1;
                    }
                }
                None => return None,
                _ => {}
            },
            _ => {}
        }
        i += 1;
    }
    None
}

pub fn to_bytes(macros: &[Vec<Action>]) -> Vec<u8> {
    let mut out = Vec::new();
    for m in macros {
        for a in expand_chords(m) {
            match a {
                Action::Tap(k) => out.extend([PREFIX, TAP, k]),
                Action::Down(k) => out.extend([PREFIX, DOWN, k]),
                Action::Up(k) => out.extend([PREFIX, UP, k]),
                Action::Delay(ms) => {
                    out.extend([PREFIX, DELAY]);
                    out.extend(ms.to_string().bytes());
                    out.push(DELAY_END);
                }
                Action::Text(s) => out.extend(s.bytes()),
                Action::Chord(_) => unreachable!(),
            }
        }
        out.push(0);
    }
    out
}

fn expand_chords(seq: &[Action]) -> Vec<Action> {
    let mut out = Vec::new();
    for a in seq {
        match a {
            Action::Chord(keys) => {
                out.extend(keys.iter().map(|k| Action::Down(*k)));
                out.extend(keys.iter().rev().map(|k| Action::Up(*k)));
            }
            other => out.push(other.clone()),
        }
    }
    out
}

/// Folds nested down/up runs into taps and chords, like VIA's convertToTapsAndChords.
fn fold_chords(seq: &[Action]) -> Vec<Action> {
    let flat: Vec<Action> = seq
        .iter()
        .flat_map(|a| match a {
            Action::Tap(k) => vec![Action::Down(*k), Action::Up(*k)],
            other => vec![other.clone()],
        })
        .collect();
    let mut out = Vec::new();
    let mut pending: Vec<Action> = Vec::new();
    let mut downs: Vec<u8> = Vec::new();
    let mut unmatched = 0usize;
    for (idx, a) in flat.iter().enumerate() {
        let mut keep_going = false;
        pending.push(a.clone());
        match a {
            Action::Down(k) if unmatched == downs.len() => {
                downs.push(*k);
                unmatched += 1;
                keep_going = true;
            }
            Action::Up(k) if unmatched > 0 && downs.get(unmatched - 1) == Some(k) => {
                unmatched -= 1;
                if unmatched == 0 {
                    out.push(if downs.len() == 1 {
                        Action::Tap(downs[0])
                    } else {
                        Action::Chord(downs.clone())
                    });
                    pending.clear();
                } else {
                    keep_going = true;
                }
            }
            _ => {}
        }
        if idx == flat.len() - 1 {
            keep_going = false;
        }
        if !keep_going {
            out.append(&mut pending);
            downs.clear();
            unmatched = 0;
        }
    }
    out
}

fn kc_name(k: u8) -> String {
    keycode::to_string(k as u16)
}

pub fn to_expression(seq: &[Action]) -> String {
    let mut s = String::new();
    for a in fold_chords(seq) {
        match a {
            Action::Tap(k) => s.push_str(&format!("{{{}}}", kc_name(k))),
            Action::Down(k) => s.push_str(&format!("{{+{}}}", kc_name(k))),
            Action::Up(k) => s.push_str(&format!("{{-{}}}", kc_name(k))),
            Action::Delay(ms) => s.push_str(&format!("{{{ms}}}")),
            Action::Chord(keys) => {
                let names: Vec<String> = keys.iter().map(|k| kc_name(*k)).collect();
                s.push_str(&format!("{{{}}}", names.join(",")));
            }
            Action::Text(t) => s.push_str(&t.replace('{', "\\{")),
        }
    }
    s
}

fn basic_keycode(name: &str) -> Result<u8> {
    match keycode::parse(name) {
        Some(v) if v <= 0xff => Ok(v as u8),
        Some(_) => bail!("{name} is not a basic keycode; macros only take keycodes up to 0xFF"),
        None => bail!("unknown keycode {name}"),
    }
}

/// Parses VIA's macro text: plain text is typed, `{KC_A}` taps, `{KC_LCTL,KC_C}`
/// presses a chord, `{+KC_X}`/`{-KC_X}` press/release, `{100}` waits 100 ms.
pub fn from_expression(expr: &str) -> Result<Vec<Action>> {
    let mut out = Vec::new();
    let mut text = String::new();
    let chars: Vec<char> = expr.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && chars.get(i + 1) == Some(&'{') {
            text.push('{');
            i += 2;
            continue;
        }
        if c == '{' {
            let Some(end) = chars[i..].iter().position(|&c| c == '}') else {
                bail!("a {{ block is not closed");
            };
            let inner: String = chars[i + 1..i + end].iter().collect::<String>();
            let inner: String = inner.chars().filter(|c| !c.is_whitespace()).collect();
            if inner.is_empty() {
                bail!("empty {{}} block; write \\{{}} to type braces");
            }
            if !text.is_empty() {
                out.push(Action::Text(std::mem::take(&mut text)));
            }
            if inner.chars().all(|c| c.is_ascii_digit()) {
                if inner.len() > 4 {
                    bail!("delay {inner} is too long, use 9999 ms or less");
                }
                out.push(Action::Delay(inner.parse()?));
            } else if let Some(k) = inner.strip_prefix('+') {
                out.push(Action::Down(basic_keycode(k)?));
            } else if let Some(k) = inner.strip_prefix('-') {
                out.push(Action::Up(basic_keycode(k)?));
            } else {
                let keys: Vec<u8> = inner
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(basic_keycode)
                    .collect::<Result<_>>()?;
                out.push(if keys.len() == 1 {
                    Action::Tap(keys[0])
                } else {
                    Action::Chord(keys)
                });
            }
            i += end + 1;
            continue;
        }
        if !c.is_ascii() {
            bail!("'{c}' cannot be typed by a macro; only ASCII text works");
        }
        text.push(c);
        i += 1;
    }
    if !text.is_empty() {
        out.push(Action::Text(text));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expression_roundtrip() {
        for e in [
            "hello",
            "{KC_LCTL,KC_C}",
            "{+KC_LSFT}abc{-KC_LSFT}",
            "{100}x{KC_ENT}",
            "brace \\{ here",
        ] {
            let seq = from_expression(e).unwrap();
            let bytes = to_bytes(&[seq]);
            let back = parse_buffer(&bytes, 1);
            assert_eq!(to_expression(&back[0]), e);
        }
    }

    #[test]
    fn used_len_skips_keycode_zero() {
        let m = vec![vec![Action::Tap(0)], vec![Action::Text("ab".into())]];
        let mut bytes = to_bytes(&m);
        let n = bytes.len();
        bytes.extend([0u8; 10]);
        assert_eq!(used_len(&bytes, 2), Some(n));
        assert_eq!(used_len(&bytes[..3], 2), None);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(from_expression("{KC_A").is_err());
        assert!(from_expression("{}").is_err());
        assert!(from_expression("{MO(1)}").is_err());
        assert!(from_expression("{99999}").is_err());
    }
}
