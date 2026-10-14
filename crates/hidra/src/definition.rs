//! VIA v2/v3 keyboard definitions and the KLE layout they embed.

use crate::keycode::CustomKeycode;
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Key {
    pub row: u8,
    pub col: u8,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// second rectangle for stepped/ISO keys, relative to x/y
    pub x2: f32,
    pub y2: f32,
    pub w2: f32,
    pub h2: f32,
    pub r: f32,
    pub rx: f32,
    pub ry: f32,
    pub color: String,
    pub encoder: Option<u8>,
    /// (group, option) for keys that belong to a layout option
    pub option: Option<(usize, usize)>,
    pub decal: bool,
}

#[derive(Debug, Clone)]
pub enum LayoutLabel {
    Toggle(String),
    Choice(String, Vec<String>),
}

impl LayoutLabel {
    pub fn choices(&self) -> usize {
        match self {
            LayoutLabel::Toggle(_) => 2,
            LayoutLabel::Choice(_, c) => c.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Definition {
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub rows: u8,
    pub cols: u8,
    pub keys: Vec<Key>,
    pub layout_labels: Vec<LayoutLabel>,
    pub custom_keycodes: Vec<CustomKeycode>,
    pub menus: Vec<Value>,
    pub keycode_modules: Vec<String>,
    pub path: Option<PathBuf>,
}

#[derive(Deserialize)]
struct RawMatrix {
    rows: u8,
    cols: u8,
}

#[derive(Deserialize)]
struct RawLayouts {
    keymap: Value,
    #[serde(default)]
    labels: Vec<Value>,
}

#[derive(Deserialize)]
struct RawDefinition {
    name: String,
    #[serde(rename = "vendorId")]
    vendor_id: String,
    #[serde(rename = "productId")]
    product_id: String,
    matrix: RawMatrix,
    layouts: RawLayouts,
    #[serde(default)]
    menus: Vec<Value>,
    #[serde(default)]
    keycodes: Vec<String>,
    #[serde(default, rename = "customKeycodes")]
    custom_keycodes: Vec<CustomKeycode>,
    #[serde(default)]
    lighting: Option<Value>,
}

fn parse_hex16(s: &str) -> Result<u16> {
    let t = s.trim().trim_start_matches("0x").trim_start_matches("0X");
    u16::from_str_radix(t, 16).with_context(|| format!("bad hex id {s}"))
}

impl Definition {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let mut d = Self::parse(&text).with_context(|| format!("parsing {}", path.display()))?;
        d.path = Some(path.to_path_buf());
        Ok(d)
    }

    pub fn parse(text: &str) -> Result<Self> {
        let raw: RawDefinition = serde_json::from_str(text)?;
        let keys = parse_kle(&raw.layouts.keymap)?;
        let mut layout_labels = Vec::new();
        for l in raw.layouts.labels {
            match l {
                Value::String(s) => layout_labels.push(LayoutLabel::Toggle(s)),
                Value::Array(a) => {
                    let mut it = a.into_iter().filter_map(|v| v.as_str().map(String::from));
                    let name = it.next().unwrap_or_default();
                    layout_labels.push(LayoutLabel::Choice(name, it.collect()));
                }
                _ => {}
            }
        }
        let mut keycode_modules = raw.keycodes;
        // v2 definitions describe lighting with a single field instead of menus
        if raw.lighting.is_some() && !keycode_modules.iter().any(|m| m == "qmk_lighting") {
            keycode_modules.push("qmk_lighting".into());
        }
        let mut def = Definition {
            name: raw.name,
            vendor_id: parse_hex16(&raw.vendor_id)?,
            product_id: parse_hex16(&raw.product_id)?,
            rows: raw.matrix.rows,
            cols: raw.matrix.cols,
            keys,
            layout_labels,
            custom_keycodes: raw.custom_keycodes,
            menus: raw.menus,
            keycode_modules,
            path: None,
        };
        def.align_option_keys();
        Ok(def)
    }

    pub fn vendor_product_id(&self) -> u32 {
        ((self.vendor_id as u32) << 16) | self.product_id as u32
    }

    pub fn has_lighting_keycodes(&self) -> bool {
        self.keycode_modules.iter().any(|m| m.contains("lighting") || m.contains("rgb") || m.contains("backlight"))
    }

    pub fn encoder_count(&self) -> usize {
        self.keys
            .iter()
            .filter_map(|k| k.encoder)
            .map(|e| e as usize + 1)
            .max()
            .unwrap_or(0)
    }

    /// Keys that should be drawn for the given layout option choices.
    pub fn visible_keys<'a>(&'a self, options: &'a [usize]) -> impl Iterator<Item = (usize, &'a Key)> + 'a {
        self.keys.iter().enumerate().filter(move |(_, k)| {
            !k.decal
                && match k.option {
                    None => true,
                    Some((g, o)) => options.get(g).copied().unwrap_or(0) == o,
                }
        })
    }

    /// Like VIA, moves every non-default layout option so it sits where option 0 sits.
    fn align_option_keys(&mut self) {
        use std::collections::BTreeMap;
        let mut origins: BTreeMap<(usize, usize), (f32, f32)> = BTreeMap::new();
        for k in &self.keys {
            if let Some(opt) = k.option {
                if k.r != 0.0 {
                    continue;
                }
                let e = origins.entry(opt).or_insert((f32::MAX, f32::MAX));
                e.0 = e.0.min(k.x);
                e.1 = e.1.min(k.y);
            }
        }
        for k in &mut self.keys {
            let Some((g, o)) = k.option else { continue };
            if o == 0 || k.r != 0.0 {
                continue;
            }
            let (Some(base), Some(this)) = (origins.get(&(g, 0)), origins.get(&(g, o))) else {
                continue;
            };
            k.x += base.0 - this.0;
            k.y += base.1 - this.1;
        }
    }

    /// Packs layout choices into the 32-bit value the firmware stores (see VIA bit-pack.ts).
    pub fn pack_options(&self, choices: &[usize]) -> u32 {
        let mut bits: u32 = 0;
        for (i, l) in self.layout_labels.iter().enumerate() {
            let size = bit_size(l.choices());
            bits = (bits << size) | choices.get(i).copied().unwrap_or(0) as u32;
        }
        bits
    }

    pub fn unpack_options(&self, mut bits: u32) -> Vec<usize> {
        let mut out = vec![0; self.layout_labels.len()];
        for (i, l) in self.layout_labels.iter().enumerate().rev() {
            let size = bit_size(l.choices());
            out[i] = (bits & ((1 << size) - 1)) as usize;
            bits >>= size;
        }
        out
    }
}

fn bit_size(choices: usize) -> u32 {
    (0..5).find(|i| (2usize << i) >= choices).map(|i| i + 1).unwrap_or(5)
}

/// KLE's legend reordering, indexed by the `a` alignment flag (from kle-serial).
const LABEL_MAP: [[i8; 12]; 8] = [
    [0, 6, 2, 8, 9, 11, 3, 5, 1, 4, 7, 10],
    [1, 7, -1, -1, 9, 11, 4, -1, -1, -1, -1, 10],
    [3, -1, 5, -1, 9, 11, -1, -1, 4, -1, -1, 10],
    [4, -1, -1, -1, 9, 11, -1, -1, -1, -1, -1, 10],
    [0, 6, 2, 8, 10, -1, 3, 5, 1, 4, 7, -1],
    [1, 7, -1, -1, 10, -1, 4, -1, -1, -1, -1, -1],
    [3, -1, 5, -1, 10, -1, -1, -1, 4, -1, -1, -1],
    [4, -1, -1, -1, 10, -1, -1, -1, -1, -1, -1, -1],
];

fn num(v: &Value) -> f32 {
    v.as_f64().unwrap_or(0.0) as f32
}

/// Parses the KLE "serial" format the way kle-serial does, keeping what VIA needs.
pub fn parse_kle(keymap: &Value) -> Result<Vec<Key>> {
    let Some(rows) = keymap.as_array() else {
        bail!("layouts.keymap is not an array");
    };
    let mut keys = Vec::new();
    // persistent state
    let (mut x, mut y) = (0.0f32, 0.0f32);
    let (mut r, mut rx, mut ry) = (0.0f32, 0.0f32, 0.0f32);
    let mut color = "#cccccc".to_string();
    let mut align: usize = 4;
    // per-key state, reset after each key
    let mut w = 1.0f32;
    let mut h = 1.0f32;
    let (mut x2, mut y2, mut w2, mut h2) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let mut decal = false;
    let mut size_set = false;

    for row in rows {
        let Some(items) = row.as_array() else {
            // the first element may be keyboard metadata
            continue;
        };
        for item in items {
            match item {
                Value::Object(o) => {
                    if let Some(v) = o.get("r") {
                        r = num(v);
                    }
                    if let Some(v) = o.get("rx") {
                        rx = num(v);
                        x = rx;
                        y = ry;
                    }
                    if let Some(v) = o.get("ry") {
                        ry = num(v);
                        x = rx;
                        y = ry;
                    }
                    if let Some(v) = o.get("a") {
                        align = (num(v) as usize).min(7);
                    }
                    if let Some(v) = o.get("c").and_then(|v| v.as_str()) {
                        color = v.to_string();
                    }
                    if let Some(v) = o.get("x") {
                        x += num(v);
                    }
                    if let Some(v) = o.get("y") {
                        y += num(v);
                    }
                    if let Some(v) = o.get("w") {
                        w = num(v);
                        if !size_set {
                            w2 = w;
                        }
                    }
                    if let Some(v) = o.get("h") {
                        h = num(v);
                        if !size_set {
                            h2 = h;
                        }
                    }
                    if let Some(v) = o.get("x2") {
                        x2 = num(v);
                        size_set = true;
                    }
                    if let Some(v) = o.get("y2") {
                        y2 = num(v);
                        size_set = true;
                    }
                    if let Some(v) = o.get("w2") {
                        w2 = num(v);
                        size_set = true;
                    }
                    if let Some(v) = o.get("h2") {
                        h2 = num(v);
                        size_set = true;
                    }
                    if let Some(v) = o.get("d") {
                        decal = v.as_bool().unwrap_or(false);
                    }
                }
                Value::String(text) => {
                    let mut labels: [Option<String>; 12] = Default::default();
                    for (i, part) in text.split('\n').enumerate().take(12) {
                        if part.is_empty() {
                            continue;
                        }
                        let pos = LABEL_MAP[align][i];
                        if pos >= 0 {
                            labels[pos as usize] = Some(part.to_string());
                        }
                    }
                    let matrix = labels[0].as_deref().and_then(|s| {
                        let (a, b) = s.split_once(',')?;
                        Some((a.trim().parse::<u8>().ok()?, b.trim().parse::<u8>().ok()?))
                    });
                    let encoder = labels[4]
                        .as_deref()
                        .and_then(|s| s.strip_prefix('e'))
                        .and_then(|s| s.parse::<u8>().ok());
                    let option = labels[8].as_deref().and_then(|s| {
                        let (a, b) = s.split_once(',')?;
                        Some((a.trim().parse::<usize>().ok()?, b.trim().parse::<usize>().ok()?))
                    });
                    if !size_set {
                        w2 = w;
                        h2 = h;
                    }
                    if let Some((row, col)) = matrix {
                        keys.push(Key {
                            row,
                            col,
                            x,
                            y,
                            w,
                            h,
                            x2,
                            y2,
                            w2,
                            h2,
                            r,
                            rx,
                            ry,
                            color: color.clone(),
                            encoder,
                            option,
                            decal,
                        });
                    }
                    x += w;
                    w = 1.0;
                    h = 1.0;
                    x2 = 0.0;
                    y2 = 0.0;
                    w2 = 0.0;
                    h2 = 0.0;
                    decal = false;
                    size_set = false;
                }
                _ => {}
            }
        }
        y += 1.0;
        x = rx;
    }
    Ok(keys)
}

/// The app's config folder. The app used to be called via-native; its old
/// folder is moved over the first time, or used in place if the move fails.
pub fn config_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("hidra");
    let old = base.join("via-native");
    if !dir.exists() && old.is_dir() && std::fs::rename(&old, &dir).is_err() {
        return old;
    }
    dir
}

/// True when the old via-native folder is still in use because it could not be moved.
pub fn using_old_config() -> bool {
    config_dir().ends_with("via-native")
}

/// Where definitions are looked up.
pub fn definitions_dir() -> PathBuf {
    config_dir().join("definitions")
}

/// Loads every definition in the config folder, skipping files that fail to parse.
pub fn load_all() -> Vec<Definition> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(definitions_dir()) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        match Definition::load(&p) {
            Ok(d) => out.push(d),
            Err(err) => eprintln!("skipping {}: {err:#}", p.display()),
        }
    }
    out
}

/// Copies a definition into the config folder so it is found on the next start.
pub fn install(path: &Path) -> Result<Definition> {
    let def = Definition::load(path)?;
    let dir = definitions_dir();
    std::fs::create_dir_all(&dir)?;
    let file = format!("{:04x}_{:04x}.json", def.vendor_id, def.product_id);
    let dest = dir.join(file);
    std::fs::copy(path, &dest)?;
    Definition::load(&dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kle_positions() {
        let v: Value = serde_json::from_str(
            r##"[[{"c":"#777777"},"0,0\n\n\n\n\n\n\n\n\ne0",{"x":0.25,"w":2},"0,1","0,2"],
                ["1,0",{"x":0.5},"1,1"]]"##,
        )
        .unwrap();
        let keys = parse_kle(&v).unwrap();
        assert_eq!(keys.len(), 5);
        assert_eq!(keys[0].encoder, Some(0));
        assert_eq!((keys[1].x, keys[1].w), (1.25, 2.0));
        assert_eq!(keys[2].x, 3.25);
        assert_eq!((keys[4].x, keys[4].y), (1.5, 1.0));
    }

    #[test]
    fn option_bits_roundtrip() {
        let mut d = Definition::parse(
            r#"{"name":"t","vendorId":"0x1","productId":"0x2","matrix":{"rows":1,"cols":1},
                "layouts":{"labels":["Split BS",["Bottom","ANSI","Tsangan","WKL"]],"keymap":[["0,0"]]}}"#,
        )
        .unwrap();
        d.path = None;
        let bits = d.pack_options(&[1, 2]);
        assert_eq!(bits, 0b1_10);
        assert_eq!(d.unpack_options(bits), vec![1, 2]);
    }
}
