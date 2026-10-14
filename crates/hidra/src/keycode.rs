//! Keycode naming and parsing for VIA protocol 12 (QMK keycodes v0.19+).
//!
//! Ported from the-via/app src/utils/key.ts and src/utils/advanced-keys.ts.

use crate::keycode_table::V12;
use std::collections::HashMap;
use std::sync::OnceLock;

pub const QK_MODS: u16 = 0x0100;
pub const QK_MODS_MAX: u16 = 0x1fff;
pub const QK_MOD_TAP: u16 = 0x2000;
pub const QK_MOD_TAP_MAX: u16 = 0x3fff;
pub const QK_LAYER_TAP: u16 = 0x4000;
pub const QK_LAYER_TAP_MAX: u16 = 0x4fff;
pub const QK_LAYER_MOD: u16 = 0x5000;
pub const QK_LAYER_MOD_MAX: u16 = 0x51ff;
pub const QK_TO: u16 = 0x5200;
pub const QK_MOMENTARY: u16 = 0x5220;
pub const QK_DEF_LAYER: u16 = 0x5240;
pub const QK_TOGGLE_LAYER: u16 = 0x5260;
pub const QK_ONE_SHOT_LAYER: u16 = 0x5280;
pub const QK_ONE_SHOT_MOD: u16 = 0x52a0;
pub const QK_LAYER_TAP_TOGGLE: u16 = 0x52c0;
pub const QK_LAYER_TAP_TOGGLE_MAX: u16 = 0x52df;
pub const QK_MACRO: u16 = 0x7700;
pub const QK_MACRO_MAX: u16 = 0x777f;
pub const QK_KB: u16 = 0x7e00;
pub const QK_KB_MAX: u16 = 0x7eff;

pub const KC_NO: u16 = 0x0000;
pub const KC_TRNS: u16 = 0x0001;

/// Newer QMK spellings that VIA's v12 table predates; accepted when parsing.
const ALIASES: &[(&str, u16)] = &[
    ("XXXXXXX", 0x0000),
    ("_______", 0x0001),
    ("KC_TRANSPARENT", 0x0001),
    ("KC_ENTER", 0x0028),
    ("KC_ESCAPE", 0x0029),
    ("KC_BACKSPACE", 0x002a),
    ("KC_SPACE", 0x002c),
    ("KC_MINUS", 0x002d),
    ("KC_EQUAL", 0x002e),
    ("KC_LEFT_BRACKET", 0x002f),
    ("KC_RIGHT_BRACKET", 0x0030),
    ("KC_BACKSLASH", 0x0031),
    ("KC_SEMICOLON", 0x0033),
    ("KC_QUOTE", 0x0034),
    ("KC_GRAVE", 0x0035),
    ("KC_COMMA", 0x0036),
    ("KC_SLASH", 0x0038),
    ("KC_CAPS_LOCK", 0x0039),
    ("KC_PRINT_SCREEN", 0x0046),
    ("KC_SCRL", 0x0047),
    ("KC_SCROLL_LOCK", 0x0047),
    ("KC_PAUSE", 0x0048),
    ("KC_INSERT", 0x0049),
    ("KC_PAGE_UP", 0x004b),
    ("KC_DELETE", 0x004c),
    ("KC_PAGE_DOWN", 0x004e),
    ("KC_RIGHT", 0x004f),
    ("KC_NUM", 0x0053),
    ("KC_NUM_LOCK", 0x0053),
    ("KC_APPLICATION", 0x0065),
    ("KC_AUDIO_MUTE", 0x00a8),
    ("KC_AUDIO_VOL_UP", 0x00a9),
    ("KC_AUDIO_VOL_DOWN", 0x00aa),
    ("KC_MEDIA_NEXT_TRACK", 0x00ab),
    ("KC_MEDIA_PREV_TRACK", 0x00ac),
    ("KC_MEDIA_STOP", 0x00ad),
    ("KC_MEDIA_PLAY_PAUSE", 0x00ae),
    ("KC_LEFT_CTRL", 0x00e0),
    ("KC_LEFT_SHIFT", 0x00e1),
    ("KC_LEFT_ALT", 0x00e2),
    ("KC_LEFT_GUI", 0x00e3),
    ("KC_LCMD", 0x00e3),
    ("KC_LWIN", 0x00e3),
    ("KC_RIGHT_CTRL", 0x00e4),
    ("KC_RIGHT_SHIFT", 0x00e5),
    ("KC_RIGHT_ALT", 0x00e6),
    ("KC_ALGR", 0x00e6),
    ("KC_RIGHT_GUI", 0x00e7),
    ("KC_RCMD", 0x00e7),
    ("KC_RWIN", 0x00e7),
    ("QK_BOOT", 0x7c00),
    ("QK_BOOTLOADER", 0x7c00),
    ("QK_REBOOT", 0x7c01),
    ("QK_RBT", 0x7c01),
    ("QK_DEBUG_TOGGLE", 0x7c02),
    ("DB_TOGG", 0x7c02),
    ("EE_CLR", 0x7c03),
    ("QK_GESC", 0x7c16),
    ("QK_GRAVE_ESCAPE", 0x7c16),
    ("NK_TOGG", 0x7013),
    ("CG_SWAP", 0x701b),
    ("CG_NORM", 0x701c),
    ("CG_TOGG", 0x701d),
    ("AG_SWAP", 0x7014),
    ("AG_NORM", 0x7015),
    ("AG_TOGG", 0x7016),
    ("GU_ON", 0x7009),
    ("GU_OFF", 0x700a),
    ("GU_TOGG", 0x700b),
    ("SC_LSPO", 0x7c1a),
    ("SC_RSPC", 0x7c1b),
    ("SC_SENT", 0x7c1e),
    ("BL_TOGG", 0x7802),
    ("RGB_MODE_FORWARD", 0x7821),
    ("RGB_MODE_REVERSE", 0x7822),
    ("UG_TOGG", 0x7820),
    ("UG_NEXT", 0x7821),
    ("UG_PREV", 0x7822),
    ("UG_HUEU", 0x7823),
    ("UG_HUED", 0x7824),
    ("UG_SATU", 0x7825),
    ("UG_SATD", 0x7826),
    ("UG_VALU", 0x7827),
    ("UG_VALD", 0x7828),
    ("UG_SPDU", 0x7829),
    ("UG_SPDD", 0x782a),
    ("KC_MS_L", 0x00cf),
    ("KC_MS_R", 0x00d0),
    ("KC_MS_U", 0x00cd),
    ("KC_MS_D", 0x00ce),
    ("KC_BTN1", 0x00d1),
    ("KC_BTN2", 0x00d2),
    ("KC_BTN3", 0x00d3),
    ("KC_BTN4", 0x00d4),
    ("KC_BTN5", 0x00d5),
    ("KC_WH_U", 0x00d9),
    ("KC_WH_D", 0x00da),
    ("KC_WH_L", 0x00db),
    ("KC_WH_R", 0x00dc),
];

#[derive(Debug, Clone)]
pub struct KeycodeEntry {
    pub code: String,
    pub name: String,
    pub title: Option<String>,
    pub short: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Category {
    pub id: &'static str,
    pub label: &'static str,
    pub keys: Vec<KeycodeEntry>,
}

fn e(code: &str, name: &str) -> KeycodeEntry {
    KeycodeEntry {
        code: code.into(),
        name: name.into(),
        title: None,
        short: None,
    }
}

fn et(code: &str, name: &str, title: &str) -> KeycodeEntry {
    KeycodeEntry {
        title: Some(title.into()),
        ..e(code, name)
    }
}

fn es(code: &str, name: &str, short: &str) -> KeycodeEntry {
    KeycodeEntry {
        short: Some(short.into()),
        ..e(code, name)
    }
}

pub struct Db {
    by_name: HashMap<String, u16>,
    by_value: HashMap<u16, String>,
    base_categories: Vec<Category>,
    by_code: HashMap<String, KeycodeEntry>,
}

pub fn db() -> &'static Db {
    static DB: OnceLock<Db> = OnceLock::new();
    DB.get_or_init(Db::build)
}

impl Db {
    fn build() -> Db {
        let base_categories = base_categories();
        let mut by_code = HashMap::new();
        for cat in &base_categories {
            for k in &cat.keys {
                by_code.entry(k.code.clone()).or_insert_with(|| k.clone());
            }
        }
        let mut by_name = HashMap::new();
        let mut by_value: HashMap<u16, String> = HashMap::new();
        for (name, v) in V12 {
            by_name.insert(name.to_string(), *v);
            if name.starts_with('_') {
                continue;
            }
            // like VIA, a value with several names prefers the one shown in a menu
            match by_value.get(v) {
                Some(prev) if by_code.contains_key(prev) || !by_code.contains_key(*name) => {}
                _ => {
                    by_value.insert(*v, name.to_string());
                }
            }
        }
        for (name, v) in ALIASES {
            by_name.entry(name.to_string()).or_insert(*v);
        }
        Db {
            by_name,
            by_value,
            base_categories,
            by_code,
        }
    }

    pub fn basic_name(&self, v: u16) -> Option<&str> {
        self.by_value.get(&v).map(|s| s.as_str())
    }

    pub fn basic_value(&self, name: &str) -> Option<u16> {
        self.by_name.get(name).copied()
    }

    pub fn entry(&self, code: &str) -> Option<&KeycodeEntry> {
        self.by_code.get(code)
    }

    /// Every named keycode that is not in a menu, like VIA's "Other" tab.
    pub fn other_category(&self) -> Category {
        let mut keys: Vec<KeycodeEntry> = V12
            .iter()
            .filter(|(n, _)| !n.starts_with('_') && !self.by_code.contains_key(*n))
            .map(|(n, _)| e(n, &n.trim_start_matches("KC_").replace('_', " ")))
            .collect();
        keys.dedup_by(|a, b| a.code == b.code);
        Category {
            id: "other",
            label: "Other",
            keys,
        }
    }

    pub fn categories(&self, macro_count: usize, custom: &[CustomKeycode], lighting: bool) -> Vec<Category> {
        let mut out = Vec::new();
        for cat in &self.base_categories {
            if cat.id == "qmk_lighting" && !lighting {
                continue;
            }
            if cat.id == "macro" {
                out.push(Category {
                    id: "macro",
                    label: "Macro",
                    keys: (0..macro_count)
                        .map(|i| et(&format!("MACRO({i})"), &format!("M{i}"), &format!("Macro {i}")))
                        .collect(),
                });
                continue;
            }
            out.push(cat.clone());
        }
        if !custom.is_empty() {
            out.push(Category {
                id: "custom",
                label: "Custom",
                keys: custom
                    .iter()
                    .enumerate()
                    .map(|(i, c)| KeycodeEntry {
                        code: format!("CUSTOM({i})"),
                        name: c.name.clone(),
                        title: c.title.clone(),
                        short: c.short_name.clone(),
                    })
                    .collect(),
            });
        }
        out.push(self.other_category());
        out
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct CustomKeycode {
    pub name: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, rename = "shortName")]
    pub short_name: Option<String>,
}

const MOD_MASKS: &[(&str, u8)] = &[
    ("MOD_LCTL", 0x01),
    ("MOD_LSFT", 0x02),
    ("MOD_LALT", 0x04),
    ("MOD_LGUI", 0x08),
    ("MOD_RCTL", 0x11),
    ("MOD_RSFT", 0x12),
    ("MOD_RALT", 0x14),
    ("MOD_RGUI", 0x18),
    ("MOD_HYPR", 0x0f),
    ("MOD_MEH", 0x07),
];

/// Modifier wrappers like `LCTL(kc)`; the value is the QK_MODS bits (mask << 8).
const MOD_WRAPPERS: &[(&str, u16)] = &[
    ("LCTL", 0x0100),
    ("C", 0x0100),
    ("LSFT", 0x0200),
    ("S", 0x0200),
    ("LALT", 0x0400),
    ("A", 0x0400),
    ("LOPT", 0x0400),
    ("LGUI", 0x0800),
    ("LCMD", 0x0800),
    ("LWIN", 0x0800),
    ("G", 0x0800),
    ("RCTL", 0x1100),
    ("RSFT", 0x1200),
    ("ALGR", 0x1400),
    ("RALT", 0x1400),
    ("ROPT", 0x1400),
    ("RCMD", 0x1800),
    ("RWIN", 0x1800),
    ("RGUI", 0x1800),
    ("SCMD", 0x0a00),
    ("SWIN", 0x0a00),
    ("SGUI", 0x0a00),
    ("LSG", 0x0a00),
    ("LAG", 0x0c00),
    ("RSG", 0x1a00),
    ("RAG", 0x1c00),
    ("LCA", 0x0500),
    ("LSA", 0x0600),
    ("SAGR", 0x1600),
    ("RSA", 0x1600),
    ("RCS", 0x1300),
    ("LCAG", 0x0d00),
    ("MEH", 0x0700),
    ("HYPR", 0x0f00),
];

/// Names used when printing mod-wrapped keycodes; other combinations nest.
const CANONICAL_WRAPPERS: &[(&str, u16)] = &[
    ("LCTL", 0x0100),
    ("LSFT", 0x0200),
    ("LALT", 0x0400),
    ("LGUI", 0x0800),
    ("RCTL", 0x1100),
    ("RSFT", 0x1200),
    ("RALT", 0x1400),
    ("RGUI", 0x1800),
    ("SGUI", 0x0a00),
    ("LAG", 0x0c00),
    ("RSG", 0x1a00),
    ("RAG", 0x1c00),
    ("LCA", 0x0500),
    ("LSA", 0x0600),
    ("RSA", 0x1600),
    ("RCS", 0x1300),
    ("LCAG", 0x0d00),
    ("MEH", 0x0700),
    ("HYPR", 0x0f00),
];

/// Formats a 5-bit QMK mod mask as `MOD_LCTL | MOD_LSFT`.
pub fn mod_mask_to_string(mask: u8) -> String {
    let right = mask & 0x10 != 0;
    let names: Vec<&str> = [
        (0x01, "CTL"),
        (0x02, "SFT"),
        (0x04, "ALT"),
        (0x08, "GUI"),
    ]
    .iter()
    .filter(|(bit, _)| mask & bit != 0)
    .map(|(_, n)| *n)
    .collect();
    if names.is_empty() {
        return "0".into();
    }
    names
        .iter()
        .map(|n| format!("MOD_{}{}", if right { 'R' } else { 'L' }, n))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn parse_mods(s: &str) -> Option<u8> {
    let mut mask = 0u8;
    for part in s.split('|') {
        let part = part.trim();
        let (_, v) = MOD_MASKS.iter().find(|(n, _)| *n == part)?;
        mask |= v;
    }
    (mask != 0).then_some(mask)
}

fn parse_number(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(h, 16).ok()
    } else {
        s.parse().ok()
    }
}

/// Splits `NAME(args)` into its name and inner argument text.
fn split_call(s: &str) -> Option<(&str, &str)> {
    let open = s.find('(')?;
    if !s.ends_with(')') {
        return None;
    }
    Some((s[..open].trim(), &s[open + 1..s.len() - 1]))
}

/// Parses any keycode expression VIA accepts, plus raw hex/decimal numbers.
pub fn parse(input: &str) -> Option<u16> {
    let s: String = input.trim().to_string();
    if s.is_empty() {
        return None;
    }
    let upper = s.to_uppercase();
    let d = db();
    if let Some(v) = d.basic_value(&upper) {
        return Some(v);
    }
    if let Some(v) = d.basic_value(&format!("KC_{upper}")) {
        return Some(v);
    }
    if upper.starts_with("0X") || upper.chars().all(|c| c.is_ascii_digit()) {
        return parse_number(&upper).and_then(|n| u16::try_from(n).ok());
    }
    let (name, inner) = split_call(&upper)?;
    let args: Vec<&str> = inner.splitn(2, ',').map(|a| a.trim()).collect();
    let layer_fn = |base: u16, max_layer: u32| -> Option<u16> {
        let n = parse_number(args.first()?)?;
        (n <= max_layer).then(|| base + n as u16)
    };
    match name {
        "MO" => layer_fn(QK_MOMENTARY, 31),
        "TO" => layer_fn(QK_TO, 31),
        "DF" => layer_fn(QK_DEF_LAYER, 31),
        "TG" => layer_fn(QK_TOGGLE_LAYER, 31),
        "OSL" => layer_fn(QK_ONE_SHOT_LAYER, 31),
        "TT" => layer_fn(QK_LAYER_TAP_TOGGLE, 31),
        "MACRO" => layer_fn(QK_MACRO, (QK_MACRO_MAX - QK_MACRO) as u32),
        "CUSTOM" | "USER" | "QK_KB" => layer_fn(QK_KB, (QK_KB_MAX - QK_KB) as u32),
        "OSM" => Some(QK_ONE_SHOT_MOD | parse_mods(inner)? as u16),
        "LT" => {
            let layer = parse_number(args.first()?)?;
            let kc = parse(args.get(1)?)?;
            (layer <= 15 && kc <= 0xff).then(|| QK_LAYER_TAP | ((layer as u16) << 8) | kc)
        }
        "LM" => {
            let layer = parse_number(args.first()?)?;
            let mods = parse_mods(args.get(1)?)?;
            (layer <= 15).then(|| QK_LAYER_MOD | ((layer as u16) << 5) | mods as u16)
        }
        "MT" => {
            let mods = parse_mods(args.first()?)?;
            let kc = parse(args.get(1)?)?;
            (kc <= 0xff).then(|| QK_MOD_TAP | ((mods as u16 & 0x1f) << 8) | kc)
        }
        _ => {
            if let Some(mt) = mod_tap_shorthand(name) {
                let kc = parse(inner)?;
                return (kc <= 0xff).then(|| QK_MOD_TAP | ((mt as u16) << 8) | kc);
            }
            let (_, bits) = MOD_WRAPPERS.iter().find(|(n, _)| *n == name)?;
            let kc = parse(inner)?;
            // nested wrappers like LCTL(LSFT(KC_A)) combine their mod bits
            if kc > QK_MODS_MAX || (kc >= QK_MODS && (kc & 0x1000) != (bits & 0x1000)) {
                return None;
            }
            Some(kc | bits)
        }
    }
}

/// QMK's `LCTL_T(kc)` style mod-tap helpers.
fn mod_tap_shorthand(name: &str) -> Option<u8> {
    Some(match name {
        "LCTL_T" | "CTL_T" => 0x01,
        "LSFT_T" | "SFT_T" => 0x02,
        "LALT_T" | "ALT_T" | "LOPT_T" | "OPT_T" => 0x04,
        "LGUI_T" | "GUI_T" | "LCMD_T" | "LWIN_T" | "CMD_T" | "WIN_T" => 0x08,
        "RCTL_T" => 0x11,
        "RSFT_T" => 0x12,
        "RALT_T" | "ALGR_T" | "ROPT_T" => 0x14,
        "RGUI_T" | "RCMD_T" | "RWIN_T" => 0x18,
        "MEH_T" => 0x07,
        "HYPR_T" | "ALL_T" => 0x0f,
        "LCA_T" => 0x05,
        "LSA_T" => 0x06,
        "LCAG_T" => 0x0d,
        "SGUI_T" | "SCMD_T" | "SWIN_T" => 0x0a,
        "LAG_T" => 0x0c,
        _ => return None,
    })
}

fn basic_or_hex(v: u16) -> String {
    db().basic_name(v)
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("0x{v:04x}"))
}

/// Canonical text form of a keycode, the same strings VIA writes to its save files.
pub fn to_string(v: u16) -> String {
    let d = db();
    if let Some(n) = d.basic_name(v) {
        return n.to_string();
    }
    match v {
        QK_MODS..=QK_MODS_MAX => {
            let bits = v & 0x1f00;
            let kc = basic_or_hex(v & 0xff);
            if let Some((n, _)) = CANONICAL_WRAPPERS.iter().find(|(_, b)| *b == bits) {
                return format!("{n}({kc})");
            }
            let right = bits & 0x1000 != 0;
            let names: Vec<&str> = [(0x0100, "CTL"), (0x0200, "SFT"), (0x0400, "ALT"), (0x0800, "GUI")]
                .iter()
                .filter(|(b, _)| bits & b != 0)
                .map(|(_, n)| *n)
                .collect();
            let mut s = String::new();
            for n in &names {
                s.push_str(&format!("{}{}(", if right { 'R' } else { 'L' }, n));
            }
            s.push_str(&kc);
            s.push_str(&")".repeat(names.len()));
            s
        }
        QK_MOD_TAP..=QK_MOD_TAP_MAX => format!(
            "MT({},{})",
            mod_mask_to_string(((v >> 8) & 0x1f) as u8),
            basic_or_hex(v & 0xff)
        ),
        QK_LAYER_TAP..=QK_LAYER_TAP_MAX => format!("LT({},{})", (v >> 8) & 0xf, basic_or_hex(v & 0xff)),
        QK_LAYER_MOD..=QK_LAYER_MOD_MAX => format!(
            "LM({},{})",
            (v >> 5) & 0xf,
            mod_mask_to_string((v & 0x1f) as u8)
        ),
        0x5200..=0x521f => format!("TO({})", v - QK_TO),
        0x5220..=0x523f => format!("MO({})", v - QK_MOMENTARY),
        0x5240..=0x525f => format!("DF({})", v - QK_DEF_LAYER),
        0x5260..=0x527f => format!("TG({})", v - QK_TOGGLE_LAYER),
        0x5280..=0x529f => format!("OSL({})", v - QK_ONE_SHOT_LAYER),
        0x52a0..=0x52bf => format!("OSM({})", mod_mask_to_string((v & 0x1f) as u8)),
        QK_LAYER_TAP_TOGGLE..=QK_LAYER_TAP_TOGGLE_MAX => format!("TT({})", v - QK_LAYER_TAP_TOGGLE),
        QK_MACRO..=QK_MACRO_MAX => format!("MACRO({})", v - QK_MACRO),
        QK_KB..=QK_KB_MAX => format!("CUSTOM({})", v - QK_KB),
        _ => format!("0x{v:04x}"),
    }
}

fn short_mods(mask: u8) -> String {
    let p = if mask & 0x10 != 0 { "R" } else { "" };
    let mut parts = Vec::new();
    if mask & 0x0f == 0x0f {
        return "Hyper".into();
    }
    if mask & 0x0f == 0x07 {
        return "Meh".into();
    }
    for (bit, n) in [(0x01, "Ctl"), (0x02, "Sft"), (0x04, "Alt"), (0x08, "Gui")] {
        if mask & bit != 0 {
            parts.push(format!("{p}{n}"));
        }
    }
    parts.join("+")
}

fn entry_label(k: &KeycodeEntry, width_u: f32) -> String {
    if width_u < 1.5 {
        if let Some(s) = &k.short {
            return s.clone();
        }
    }
    k.name.clone()
}

/// Text drawn on a keycap. Multi-line strings put the shifted legend on top.
pub fn label(v: u16, custom: &[CustomKeycode], width_u: f32) -> String {
    let d = db();
    let code = to_string(v);
    if let QK_KB..=QK_KB_MAX = v {
        if let Some(c) = custom.get((v - QK_KB) as usize) {
            return c.short_name.clone().unwrap_or_else(|| c.name.clone());
        }
    }
    if let Some(k) = d.entry(&code) {
        return entry_label(k, width_u);
    }
    let tap = |kc: u16| -> String {
        let s = to_string(kc);
        d.entry(&s)
            .map(|k| entry_label(k, 1.0).replace('\n', " "))
            .unwrap_or_else(|| s.trim_start_matches("KC_").to_string())
    };
    match v {
        QK_MODS..=QK_MODS_MAX => format!("{}+\n{}", short_mods(((v >> 8) & 0x1f) as u8), tap(v & 0xff)),
        QK_MOD_TAP..=QK_MOD_TAP_MAX => format!("{}\n{}", tap(v & 0xff), short_mods(((v >> 8) & 0x1f) as u8)),
        QK_LAYER_TAP..=QK_LAYER_TAP_MAX => format!("{}\nLT{}", tap(v & 0xff), (v >> 8) & 0xf),
        QK_LAYER_MOD..=QK_LAYER_MOD_MAX => format!("LM{}\n{}", (v >> 5) & 0xf, short_mods((v & 0x1f) as u8)),
        0x52a0..=0x52bf => format!("OSM\n{}", short_mods((v & 0x1f) as u8)),
        QK_MACRO..=QK_MACRO_MAX => format!("M{}", v - QK_MACRO),
        // vendor keycodes the definition does not name
        QK_KB..=QK_KB_MAX => format!("Custom\n{}", v - QK_KB),
        _ => code.trim_start_matches("KC_").to_string(),
    }
}

/// Hover text for a keycode: its title if the menus know one, else the code itself.
pub fn describe(v: u16, custom: &[CustomKeycode]) -> String {
    let code = to_string(v);
    if let QK_KB..=QK_KB_MAX = v {
        if let Some(c) = custom.get((v - QK_KB) as usize) {
            return format!("{} ({code})", c.title.clone().unwrap_or_else(|| c.name.clone()));
        }
    }
    match db().entry(&code).and_then(|k| k.title.clone()) {
        Some(t) => format!("{t} ({code})"),
        None => code,
    }
}

fn base_categories() -> Vec<Category> {
    let mut basic = vec![
        et("KC_NO", "", "Nothing"),
        et("KC_TRNS", "▽", "Pass-through"),
        e("KC_ESC", "Esc"),
    ];
    for c in 'A'..='Z' {
        basic.push(e(&format!("KC_{c}"), &c.to_string()));
    }
    for (code, name) in [
        ("KC_1", "!\n1"),
        ("KC_2", "@\n2"),
        ("KC_3", "#\n3"),
        ("KC_4", "$\n4"),
        ("KC_5", "%\n5"),
        ("KC_6", "^\n6"),
        ("KC_7", "&\n7"),
        ("KC_8", "*\n8"),
        ("KC_9", "(\n9"),
        ("KC_0", ")\n0"),
        ("KC_MINS", "_\n-"),
        ("KC_EQL", "+\n="),
        ("KC_GRV", "~\n`"),
        ("KC_LBRC", "{\n["),
        ("KC_RBRC", "}\n]"),
        ("KC_BSLS", "|\n\\"),
        ("KC_SCLN", ":\n;"),
        ("KC_QUOT", "\"\n'"),
        ("KC_COMM", "<\n,"),
        ("KC_DOT", ">\n."),
        ("KC_SLSH", "?\n/"),
        ("KC_PEQL", "="),
        ("KC_PCMM", ","),
    ] {
        basic.push(e(code, name));
    }
    for i in 1..=12 {
        basic.push(e(&format!("KC_F{i}"), &format!("F{i}")));
    }
    basic.extend([
        es("KC_PSCR", "Print Screen", "Print"),
        es("KC_SLCK", "Scroll Lock", "Scroll"),
        e("KC_PAUS", "Pause"),
        e("KC_TAB", "Tab"),
        es("KC_BSPC", "Backspace", "Bksp"),
        es("KC_INS", "Insert", "Ins"),
        e("KC_DEL", "Del"),
        e("KC_HOME", "Home"),
        e("KC_END", "End"),
        es("KC_PGUP", "Page Up", "PgUp"),
        es("KC_PGDN", "Page Down", "PgDn"),
        es("KC_NLCK", "Num\nLock", "N.Lck"),
        es("KC_CAPS", "Caps Lock", "Caps"),
        e("KC_ENT", "Enter"),
    ]);
    for i in 1..=9 {
        basic.push(et(&format!("KC_P{i}"), &i.to_string(), &format!("Numpad {i}")));
    }
    basic.extend([
        et("KC_P0", "0", "Numpad 0"),
        et("KC_PSLS", "÷", "Numpad ÷"),
        et("KC_PAST", "×", "Numpad ×"),
        et("KC_PMNS", "-", "Numpad -"),
        et("KC_PPLS", "+", "Numpad +"),
        et("KC_PDOT", ".", "Numpad ."),
        KeycodeEntry {
            short: Some("N.Ent".into()),
            ..et("KC_PENT", "Num\nEnter", "Numpad Enter")
        },
        es("KC_LSFT", "Left Shift", "LShft"),
        es("KC_RSFT", "Right Shift", "RShft"),
        es("KC_LCTL", "Left Ctrl", "LCtl"),
        es("KC_RCTL", "Right Ctrl", "RCtl"),
        es("KC_LGUI", "Left Win", "LWin"),
        es("KC_RGUI", "Right Win", "RWin"),
        es("KC_LALT", "Left Alt", "LAlt"),
        es("KC_RALT", "Right Alt", "RAlt"),
        e("KC_SPC", "Space"),
        es("KC_APP", "Menu", "RApp"),
        es("KC_LEFT", "Left", "←"),
        es("KC_DOWN", "Down", "↓"),
        es("KC_UP", "Up", "↑"),
        es("KC_RGHT", "Right", "→"),
    ]);

    let media = vec![
        et("KC_VOLD", "Vol -", "Volume Down"),
        et("KC_VOLU", "Vol +", "Volume Up"),
        et("KC_MUTE", "Mute", "Mute Audio"),
        et("KC_MPLY", "Play", "Play/Pause"),
        et("KC_MSTP", "Media Stop", "Media Stop"),
        et("KC_MPRV", "Previous", "Media Previous"),
        et("KC_MNXT", "Next", "Media Next"),
        et("KC_MRWD", "Rewind", "Rewind"),
        et("KC_MFFD", "Fast Forward", "Fast Forward"),
        et("KC_MSEL", "Select", "Media Select"),
        et("KC_EJCT", "Eject", "Media Eject"),
    ];

    let mut layers = vec![
        KeycodeEntry {
            short: Some("Fn1(3)".into()),
            ..et("FN_MO13", "Fn1\n(Fn3)", "Hold = Layer 1, Hold with Fn2 = Layer 3")
        },
        KeycodeEntry {
            short: Some("Fn2(3)".into()),
            ..et("FN_MO23", "Fn2\n(Fn3)", "Hold = Layer 2, Hold with Fn1 = Layer 3")
        },
        et("LT(1,KC_SPC)", "Space\nFn1", "Hold = Layer 1, Tap = Space"),
        et("LT(2,KC_SPC)", "Space\nFn2", "Hold = Layer 2, Tap = Space"),
        et("LT(3,KC_SPC)", "Space\nFn3", "Hold = Layer 3, Tap = Space"),
    ];
    for (f, title) in [
        ("MO", "Momentarily turn on layer"),
        ("TG", "Toggle layer on/off"),
        ("TT", "Acts like MO unless tapped several times, which toggles the layer"),
        ("OSL", "Switch to layer for one keypress"),
        ("TO", "Turn on layer and turn off every other layer above the default"),
        ("DF", "Set the default layer"),
    ] {
        for i in 0..10 {
            layers.push(et(&format!("{f}({i})"), &format!("{f}({i})"), &format!("{title} {i}")));
        }
    }

    let mut special = Vec::new();
    for (code, name, title) in [
        ("S(KC_GRV)", "~", "Shift + `"),
        ("S(KC_1)", "!", "Shift + 1"),
        ("S(KC_2)", "@", "Shift + 2"),
        ("S(KC_3)", "#", "Shift + 3"),
        ("S(KC_4)", "$", "Shift + 4"),
        ("S(KC_5)", "%", "Shift + 5"),
        ("S(KC_6)", "^", "Shift + 6"),
        ("S(KC_7)", "&", "Shift + 7"),
        ("S(KC_8)", "*", "Shift + 8"),
        ("S(KC_9)", "(", "Shift + 9"),
        ("S(KC_0)", ")", "Shift + 0"),
        ("S(KC_MINS)", "_", "Shift + -"),
        ("S(KC_EQL)", "+", "Shift + ="),
        ("S(KC_LBRC)", "{", "Shift + ["),
        ("S(KC_RBRC)", "}", "Shift + ]"),
        ("S(KC_BSLS)", "|", "Shift + \\"),
        ("S(KC_SCLN)", ":", "Shift + ;"),
        ("S(KC_QUOT)", "\"", "Shift + '"),
        ("S(KC_COMM)", "<", "Shift + ,"),
        ("S(KC_DOT)", ">", "Shift + ."),
        ("S(KC_SLSH)", "?", "Shift + /"),
        ("KC_NUHS", "NUHS", "Non-US # and ~"),
        ("KC_NUBS", "NUBS", "Non-US \\ and |"),
        ("KC_RO", "Ro", "JIS \\ and |"),
        ("KC_JYEN", "¥", "JPN Yen"),
        ("KC_MHEN", "無変換", "JIS Muhenkan"),
        ("KC_HANJ", "漢字", "Hanja"),
        ("KC_HAEN", "한영", "HanYeong"),
        ("KC_HENK", "変換", "JIS Henkan"),
        ("KC_KANA", "かな", "JIS Katakana/Hiragana"),
        ("KC_GESC", "Esc `", "Esc normally, ` when Shift or Win is held"),
        ("KC_LSPO", "LS (", "Left Shift when held, ( when tapped"),
        ("KC_RSPC", "RS )", "Right Shift when held, ) when tapped"),
        ("KC_LCPO", "LC (", "Left Control when held, ( when tapped"),
        ("KC_RCPC", "RC )", "Right Control when held, ) when tapped"),
        ("KC_LAPO", "LA (", "Left Alt when held, ( when tapped"),
        ("KC_RAPC", "RA )", "Right Alt when held, ) when tapped"),
        ("KC_SFTENT", "SftEnt", "Right Shift when held, Enter when tapped"),
        ("RESET", "Reset", "Jump to bootloader"),
        ("QK_CLEAR_EEPROM", "Clear\nEEPROM", "Wipe the keyboard's saved settings"),
        ("DEBUG", "Debug", "Toggle debug mode"),
        ("MAGIC_TOGGLE_NKRO", "NKRO", "Toggle NKRO"),
        ("MAGIC_SWAP_CTL_GUI", "CG Swap", "Swap Ctrl and GUI"),
        ("MAGIC_UNSWAP_CTL_GUI", "CG Unswap", "Unswap Ctrl and GUI"),
        ("MAGIC_TOGGLE_CTL_GUI", "CG Togg", "Toggle Ctrl and GUI swap"),
        ("MAGIC_SWAP_ALT_GUI", "AG Swap", "Swap Alt and GUI"),
        ("MAGIC_UNSWAP_ALT_GUI", "AG Unswap", "Unswap Alt and GUI"),
        ("MAGIC_TOGGLE_ALT_GUI", "AG Togg", "Toggle Alt and GUI swap"),
        ("MAGIC_GUI_ON", "GUI On", "Enable GUI keys"),
        ("MAGIC_GUI_OFF", "GUI Off", "Disable GUI keys"),
        ("MAGIC_TOGGLE_GUI", "GUI Togg", "Toggle GUI keys"),
        ("KC_LNUM", "Lock Num", "Locking Num Lock"),
        ("KC_LCAP", "Lock Caps", "Locking Caps Lock"),
        ("KC_LSCR", "Lock Scroll", "Locking Scroll Lock"),
        ("KC_PWR", "Power", "System Power"),
        ("KC_POWER", "Power OSX", "Power (macOS)"),
        ("KC_SLEP", "Sleep", "System Sleep"),
        ("KC_WAKE", "Wake", "System Wake"),
        ("KC_CALC", "Calc", "Calculator"),
        ("KC_MAIL", "Mail", "Mail"),
        ("KC_HELP", "Help", "Help"),
        ("KC_STOP", "Stop", "Stop"),
        ("KC_ERAS", "Alt Erase", "Alternate Erase"),
        ("KC_AGAIN", "Again", "Again"),
        ("KC_MENU", "Menu", "Menu"),
        ("KC_UNDO", "Undo", "Undo"),
        ("KC_SELECT", "Select", "Select"),
        ("KC_EXECUTE", "Exec", "Execute"),
        ("KC_CUT", "Cut", "Cut"),
        ("KC_COPY", "Copy", "Copy"),
        ("KC_PASTE", "Paste", "Paste"),
        ("KC_FIND", "Find", "Find"),
        ("KC_MYCM", "My Comp", "Open My Computer"),
        ("KC_WWW_HOME", "Home", "Browser Home"),
        ("KC_WWW_BACK", "Back", "Browser Back"),
        ("KC_WWW_FORWARD", "Forward", "Browser Forward"),
        ("KC_WWW_STOP", "Stop", "Browser Stop"),
        ("KC_WWW_REFRESH", "Refresh", "Browser Refresh"),
        ("KC_WWW_FAVORITES", "Favorites", "Browser Favorites"),
        ("KC_WWW_SEARCH", "Search", "Browser Search"),
        ("KC_BRIU", "Scr +", "Screen Brightness Up"),
        ("KC_BRID", "Scr -", "Screen Brightness Down"),
        ("KC_MCTL", "Mctl", "Mission Control (macOS)"),
        ("KC_LPAD", "Lpad", "Launchpad (macOS)"),
    ] {
        special.push(et(code, name, title));
    }
    for i in 13..=24 {
        special.push(e(&format!("KC_F{i}"), &format!("F{i}")));
    }
    for (code, name) in [
        ("KC_MS_UP", "Mouse ↑"),
        ("KC_MS_DOWN", "Mouse ↓"),
        ("KC_MS_LEFT", "Mouse ←"),
        ("KC_MS_RIGHT", "Mouse →"),
        ("KC_MS_BTN1", "Mouse Btn1"),
        ("KC_MS_BTN2", "Mouse Btn2"),
        ("KC_MS_BTN3", "Mouse Btn3"),
        ("KC_MS_BTN4", "Mouse Btn4"),
        ("KC_MS_BTN5", "Mouse Btn5"),
        ("KC_MS_BTN6", "Mouse Btn6"),
        ("KC_MS_BTN7", "Mouse Btn7"),
        ("KC_MS_BTN8", "Mouse Btn8"),
        ("KC_MS_WH_UP", "Mouse Wh ↑"),
        ("KC_MS_WH_DOWN", "Mouse Wh ↓"),
        ("KC_MS_WH_LEFT", "Mouse Wh ←"),
        ("KC_MS_WH_RIGHT", "Mouse Wh →"),
        ("KC_MS_ACCEL0", "Mouse Acc0"),
        ("KC_MS_ACCEL1", "Mouse Acc1"),
        ("KC_MS_ACCEL2", "Mouse Acc2"),
        ("AU_ON", "Audio On"),
        ("AU_OFF", "Audio Off"),
        ("AU_TOG", "Audio Toggle"),
        ("CLICKY_TOGGLE", "Clicky Toggle"),
        ("CLICKY_ENABLE", "Clicky Enable"),
        ("CLICKY_DISABLE", "Clicky Disable"),
        ("CLICKY_UP", "Clicky Up"),
        ("CLICKY_DOWN", "Clicky Down"),
        ("CLICKY_RESET", "Clicky Reset"),
        ("MU_ON", "Music On"),
        ("MU_OFF", "Music Off"),
        ("MU_TOG", "Music Toggle"),
        ("MU_MOD", "Music Mode"),
    ] {
        special.push(e(code, name));
    }

    let lighting = vec![
        e("BL_TOGG", "BL Toggle"),
        e("BL_ON", "BL On"),
        e("BL_OFF", "BL Off"),
        e("BL_DEC", "BL -"),
        e("BL_INC", "BL +"),
        e("BL_STEP", "BL Cycle"),
        e("BL_BRTG", "BR Toggle"),
        e("RGB_TOG", "RGB Toggle"),
        e("RGB_RMOD", "RGB Mode -"),
        e("RGB_MOD", "RGB Mode +"),
        e("RGB_HUD", "Hue -"),
        e("RGB_HUI", "Hue +"),
        e("RGB_SAD", "Sat -"),
        e("RGB_SAI", "Sat +"),
        e("RGB_VAD", "Bright -"),
        e("RGB_VAI", "Bright +"),
        e("RGB_SPD", "Effect Speed-"),
        e("RGB_SPI", "Effect Speed+"),
        et("RGB_M_P", "RGB Mode P", "Plain"),
        et("RGB_M_B", "RGB Mode B", "Breathe"),
        et("RGB_M_R", "RGB Mode R", "Rainbow"),
        et("RGB_M_SW", "RGB Mode SW", "Swirl"),
        et("RGB_M_SN", "RGB Mode SN", "Snake"),
        et("RGB_M_K", "RGB Mode K", "Knight"),
        et("RGB_M_X", "RGB Mode X", "Xmas"),
        et("RGB_M_G", "RGB Mode G", "Gradient"),
    ];

    vec![
        Category {
            id: "basic",
            label: "Basic",
            keys: basic,
        },
        Category {
            id: "media",
            label: "Media",
            keys: media,
        },
        Category {
            id: "macro",
            label: "Macro",
            keys: vec![],
        },
        Category {
            id: "layers",
            label: "Layers",
            keys: layers,
        },
        Category {
            id: "special",
            label: "Special",
            keys: special,
        },
        Category {
            id: "qmk_lighting",
            label: "Lighting",
            keys: lighting,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_common_codes() {
        for s in [
            "KC_A",
            "KC_MUTE",
            "MO(1)",
            "TG(3)",
            "LT(1,KC_SPC)",
            "MT(MOD_LCTL,KC_A)",
            "MT(MOD_RSFT,KC_ENT)",
            "LM(2,MOD_LALT)",
            "OSM(MOD_LSFT)",
            "LCTL(KC_C)",
            "HYPR(KC_F1)",
            "MACRO(3)",
            "CUSTOM(5)",
            "RGB_TOG",
            "RESET",
        ] {
            let v = parse(s).unwrap_or_else(|| panic!("parse {s}"));
            assert_eq!(parse(&to_string(v)), Some(v), "{s} -> {}", to_string(v));
        }
    }

    #[test]
    fn known_values() {
        assert_eq!(parse("KC_MUTE"), Some(0x00a8));
        assert_eq!(parse("LT(1,KC_SPC)"), Some(0x412c));
        assert_eq!(parse("MO(1)"), Some(0x5221));
        assert_eq!(parse("LCTL(LSFT(KC_A))"), Some(0x0304));
        assert_eq!(parse("CUSTOM(0)"), Some(0x7e00));
        assert_eq!(parse("QK_BOOT"), Some(0x7c00));
        assert_eq!(parse("LCTL_T(KC_A)"), Some(0x2104));
        assert_eq!(parse("a"), Some(0x0004));
        assert_eq!(parse("0x7e03"), Some(0x7e03));
        assert_eq!(to_string(0x0304), "LCTL(LSFT(KC_A))");
        assert_eq!(to_string(0x0a04), "SGUI(KC_A)");
        assert_eq!(to_string(0x2104), "MT(MOD_LCTL,KC_A)");
    }
}
