//! Windows/Mac mode kept in the keymap instead of in a hidden layer.
//!
//! Boards like the GMK70 hold a second base layer (the Mac one) and jump to it
//! with a `TO(n)` key on the Fn layer. VIA cannot read which layer is active,
//! so the app takes over: the `TO` keys are removed and switching writes the
//! chosen layout into layer 0. The mode then lives in the keymap, where it can
//! be read back.

use crate::definition;
use crate::keycode;
use crate::session::Session;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Windows,
    Mac,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::Windows => "Windows",
            Mode::Mac => "Mac",
        }
    }

    pub fn parse(s: &str) -> Option<Mode> {
        match s.to_ascii_lowercase().as_str() {
            "windows" | "win" => Some(Mode::Windows),
            "mac" | "macos" => Some(Mode::Mac),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Template {
    pub keys: Vec<String>,
    /// (counter-clockwise, clockwise) per encoder
    #[serde(default)]
    pub encoders: Vec<[String; 2]>,
}

/// A `TO(n)` key the setup removed, so turning the feature off can put it back.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RemovedKey {
    pub layer: usize,
    pub index: usize,
    pub code: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModeFile {
    pub active: Mode,
    pub windows: Template,
    pub mac: Template,
    /// the layer that held the Mac layout before setup; left untouched on the keyboard
    pub mac_layer: usize,
    pub removed: Vec<RemovedKey>,
    /// set while a switch is being written; cleared once every key is on the keyboard
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<Mode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// the app does not manage the mode for this keyboard
    Unmanaged,
    /// layer 0 matches the stored layout of this mode
    Active(Mode),
    /// layer 0 was edited since the last switch; it still counts as this mode
    Edited(Mode),
    /// a switch to this mode stopped halfway; layer 0 mixes both layouts
    Interrupted(Mode),
}

fn path(s: &Session) -> PathBuf {
    // the demo keyboard shares the real one's ids; keep its mode file apart
    let base = if s.kb.info().path == std::path::Path::new("demo") {
        std::env::temp_dir().join(format!("hidra-demo-{}", std::process::id()))
    } else {
        definition::config_dir()
    };
    base.join("modes")
        .join(format!("{:04x}_{:04x}.json", s.def.vendor_id, s.def.product_id))
}

pub fn load(s: &Session) -> Option<ModeFile> {
    let text = std::fs::read_to_string(path(s)).ok()?;
    serde_json::from_str(&text).ok()
}

fn save(s: &Session, f: &ModeFile) -> Result<()> {
    let p = path(s);
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(&p, serde_json::to_string_pretty(f)? + "\n").with_context(|| format!("writing {}", p.display()))
}

fn template(s: &Session, layer: usize) -> Template {
    Template {
        keys: s.keymap[layer].iter().map(|k| keycode::to_string(*k)).collect(),
        encoders: s
            .encoders
            .get(layer)
            .map(|l| l.iter().map(|(a, b)| [keycode::to_string(*a), keycode::to_string(*b)]).collect())
            .unwrap_or_default(),
    }
}

type Parsed = (Vec<u16>, Vec<(u16, u16)>);

fn parse_template(t: &Template) -> Result<Parsed> {
    let keys = t
        .keys
        .iter()
        .map(|k| keycode::parse(k).with_context(|| format!("unknown keycode {k} in mode file")))
        .collect::<Result<Vec<_>>>()?;
    let enc = t
        .encoders
        .iter()
        .map(|[a, b]| Ok((keycode::parse(a).context("bad encoder keycode")?, keycode::parse(b).context("bad encoder keycode")?)))
        .collect::<Result<Vec<_>>>()?;
    Ok((keys, enc))
}

pub fn state(s: &Session) -> State {
    state_with(s, load(s).as_ref())
}

/// Like `state`, with the mode file already loaded (the GUI caches it).
pub fn state_with(s: &Session, f: Option<&ModeFile>) -> State {
    let Some(f) = f else { return State::Unmanaged };
    let current = template(s, 0).keys;
    if current == f.windows.keys {
        State::Active(Mode::Windows)
    } else if current == f.mac.keys {
        State::Active(Mode::Mac)
    } else if let Some(p) = f.pending {
        State::Interrupted(p)
    } else if is_mix(&current, &f.windows.keys, &f.mac.keys) {
        // older versions saved `active` before writing, so it names the target
        State::Interrupted(f.active)
    } else {
        State::Edited(f.active)
    }
}

/// Every key matches one of the two layouts: the trace of a half-written switch.
fn is_mix(current: &[String], a: &[String], b: &[String]) -> bool {
    current.len() == a.len() && current.len() == b.len() && current.iter().zip(a.iter().zip(b)).all(|(c, (x, y))| c == x || c == y)
}

/// Every `TO(n)` key in the keymap, as (layer, index, target layer).
fn to_keys(s: &Session) -> Vec<ToKey> {
    let mut out = Vec::new();
    for (l, layer) in s.keymap.iter().enumerate() {
        for (i, k) in layer.iter().enumerate() {
            if (keycode::QK_TO..keycode::QK_TO + 32).contains(k) {
                out.push((l, i, (k - keycode::QK_TO) as usize));
            }
        }
    }
    out
}

/// (layer, index, target layer) of one `TO(n)` key.
pub type ToKey = (usize, usize, usize);

/// What setup would do, for showing before the user agrees.
pub fn plan(s: &Session) -> Result<(usize, Vec<ToKey>)> {
    let tos = to_keys(s);
    let mut targets: Vec<usize> = tos.iter().map(|t| t.2).filter(|t| *t != 0).collect();
    targets.sort();
    targets.dedup();
    let [mac_layer] = targets[..] else {
        bail!(
            "expected exactly one other base layer reached with TO(n), found {:?}; this keyboard's mode switch works differently",
            targets
        );
    };
    if mac_layer >= s.layers {
        bail!("TO({mac_layer}) points past the last layer");
    }
    Ok((mac_layer, tos))
}

/// Captures both layouts, removes the TO keys and starts in Windows mode.
pub fn setup(s: &mut Session) -> Result<ModeFile> {
    if load(s).is_some() {
        bail!("the app already controls the mode");
    }
    let (mac_layer, tos) = plan(s)?;
    let f = ModeFile {
        active: Mode::Windows,
        windows: template(s, 0),
        mac: template(s, mac_layer),
        mac_layer,
        removed: tos
            .iter()
            .map(|(l, i, _)| RemovedKey {
                layer: *l,
                index: *i,
                code: keycode::to_string(s.keymap[*l][*i]),
            })
            .collect(),
        pending: None,
    };
    // write the file first, so a failure halfway through still leaves a way back
    save(s, &f)?;
    let mut km = s.keymap.clone();
    for (l, i, _) in &tos {
        km[*l][*i] = keycode::KC_TRNS;
    }
    s.kb.write_keymap(&km)?;
    s.keymap = km;
    Ok(f)
}

/// Writes a layout into layer 0 one changed key at a time. Over the 2.4G
/// receiver every command is a radio round trip, so sending only the few keys
/// that differ is both faster and far less likely to be cut off; and if it is,
/// `s.keymap` still matches what the keyboard holds.
fn apply(s: &mut Session, t: &Template) -> Result<()> {
    let (keys, enc) = parse_template(t)?;
    if keys.len() != s.keymap[0].len() {
        bail!("mode file has {} keys, the keyboard has {}", keys.len(), s.keymap[0].len());
    }
    let cols = s.def.cols as usize;
    for (i, code) in keys.into_iter().enumerate() {
        if s.keymap[0][i] != code {
            s.set_key(0, (i / cols) as u8, (i % cols) as u8, code)?;
        }
    }
    for (e, (ccw, cw)) in enc.into_iter().enumerate() {
        if e < s.def.encoder_count() && s.encoders[0][e] != (ccw, cw) {
            s.set_encoder(0, e, false, ccw)?;
            s.set_encoder(0, e, true, cw)?;
        }
    }
    Ok(())
}

/// Switches layer 0 to `to`. Edits made to layer 0 in the current mode are kept
/// by saving it as that mode's layout first.
///
/// After an interrupted switch, layer 0 is a mix of both layouts; it is then
/// overwritten with `to` and never saved as a layout.
pub fn switch(s: &mut Session, to: Mode) -> Result<()> {
    let mut f = load(s).context("the app does not control the mode yet; run setup first")?;
    match state_with(s, Some(&f)) {
        State::Active(m) if m == to => return Ok(()),
        State::Edited(m) if m == to => return Ok(()),
        State::Edited(Mode::Windows) => f.windows = template(s, 0),
        State::Edited(Mode::Mac) => f.mac = template(s, 0),
        State::Active(_) | State::Interrupted(_) => {}
        State::Unmanaged => unreachable!(),
    }
    let target = match to {
        Mode::Windows => f.windows.clone(),
        Mode::Mac => f.mac.clone(),
    };
    f.pending = Some(to);
    save(s, &f)?;
    apply(s, &target)?;
    f.active = to;
    f.pending = None;
    save(s, &f)
}

/// Goes back to the keyboard's own switching: Windows layout in layer 0, TO keys restored.
pub fn teardown(s: &mut Session) -> Result<()> {
    let f = load(s).context("the app does not control the mode")?;
    if let State::Active(Mode::Windows) | State::Edited(Mode::Windows) = state(s) {
        // keep any edits made to the Windows layout
    } else {
        apply(s, &f.windows)?;
    }
    let cols = s.def.cols as usize;
    for r in &f.removed {
        if let Some(v) = keycode::parse(&r.code)
            && r.layer < s.layers
            && r.index < s.keymap[r.layer].len()
        {
            s.set_key(r.layer, (r.index / cols) as u8, (r.index % cols) as u8, v)?;
        }
    }
    std::fs::remove_file(path(s))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definition::Definition;
    use crate::proto::Keyboard;

    fn gmk70_like() -> Session {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/gmk70.json")).unwrap();
        let def = Definition::parse(&text).unwrap();
        let mut s = Session::with_keyboard(Keyboard::mock(&def).unwrap(), def).unwrap();
        // shape of the real board: Mac layout on layer 2, TO(2) on Fn layer 1, TO(0) on layer 3
        let mut km = s.keymap.clone();
        km[2] = km[0].clone();
        km[2][4 * 9 + 2] = keycode::parse("MO(3)").unwrap();
        km[1][2 * 9 + 3] = keycode::parse("TO(2)").unwrap();
        km[3][2 * 9 + 2] = keycode::parse("TO(0)").unwrap();
        s.kb.write_keymap(&km).unwrap();
        s.keymap = km;
        s
    }

    #[test]
    fn plan_finds_mac_layer() {
        let s = gmk70_like();
        let (mac, tos) = plan(&s).unwrap();
        assert_eq!(mac, 2);
        assert_eq!(tos.len(), 2);
    }

    // the mock keyboard keeps its mode file in a per-process temp folder
    #[test]
    fn setup_switch_teardown() {
        let mut s = gmk70_like();
        let original = s.keymap.clone();
        setup(&mut s).unwrap();
        assert_eq!(state(&s), State::Active(Mode::Windows));
        assert!(to_keys(&s).is_empty());

        switch(&mut s, Mode::Mac).unwrap();
        assert_eq!(state(&s), State::Active(Mode::Mac));
        assert_eq!(s.keymap[0], original[2]);

        // an edit made in Mac mode survives a round trip
        let q = keycode::parse("KC_Q").unwrap();
        s.set_key(0, 2, 2, q).unwrap();
        assert_eq!(state(&s), State::Edited(Mode::Mac));
        switch(&mut s, Mode::Windows).unwrap();
        assert_eq!(s.keymap[0], original[0]);
        switch(&mut s, Mode::Mac).unwrap();
        assert_eq!(s.keymap[0][2 * 9 + 2], q);

        // a switch cut off after one key: finishing it must not save the mix as a layout
        let mac_before = load(&s).unwrap().mac.keys.clone();
        let mut f = load(&s).unwrap();
        f.pending = Some(Mode::Windows);
        save(&s, &f).unwrap();
        let first_diff = (0..s.keymap[0].len()).find(|i| original[0][*i] != s.keymap[0][*i]).unwrap();
        s.set_key(0, (first_diff / 9) as u8, (first_diff % 9) as u8, original[0][first_diff]).unwrap();
        assert_eq!(state(&s), State::Interrupted(Mode::Windows));
        switch(&mut s, Mode::Mac).unwrap();
        assert_eq!(state(&s), State::Active(Mode::Mac));
        assert_eq!(load(&s).unwrap().mac.keys, mac_before);

        teardown(&mut s).unwrap();
        assert_eq!(state(&s), State::Unmanaged);
        assert_eq!(s.keymap[0], original[0]);
        assert_eq!(s.keymap[1], original[1]);
        assert_eq!(s.keymap[3], original[3]);
    }
}
