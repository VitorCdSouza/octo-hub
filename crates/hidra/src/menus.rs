//! VIA v3 custom menus: lighting and any vendor controls a definition declares.

use serde_json::{Value, json};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Control {
    Range { min: u16, max: u16 },
    Dropdown(Vec<(String, u8)>),
    Toggle { off: Vec<u8>, on: Vec<u8> },
    Color,
    Keycode,
    Button(u8),
    Label,
}

#[derive(Debug, Clone)]
pub struct Item {
    pub label: String,
    pub id: String,
    /// channel, value id and any extra addressing bytes
    pub command: Vec<u8>,
    pub control: Control,
    pub show_if: Option<String>,
}

impl Item {
    pub fn value_len(&self) -> usize {
        match &self.control {
            Control::Range { max, .. } => {
                if *max > 255 {
                    2
                } else {
                    1
                }
            }
            Control::Dropdown(_) => 1,
            Control::Toggle { on, .. } => on.len().max(1),
            Control::Color | Control::Keycode => 2,
            Control::Button(_) => 0,
            Control::Label => 0,
        }
    }

    pub fn channel(&self) -> u8 {
        self.command.first().copied().unwrap_or(0)
    }

    pub fn readable(&self) -> bool {
        !matches!(self.control, Control::Button(_) | Control::Label) && !self.command.is_empty()
    }
}

#[derive(Debug, Clone)]
pub enum Node {
    Group {
        label: String,
        show_if: Option<String>,
        children: Vec<Node>,
    },
    Item(Item),
}

pub type Values = HashMap<String, Vec<u8>>;

fn bytes_of(v: &Value) -> Vec<u8> {
    match v {
        Value::Array(a) => a.iter().filter_map(|x| x.as_u64()).map(|x| x as u8).collect(),
        Value::Number(n) => vec![n.as_u64().unwrap_or(0) as u8],
        _ => vec![],
    }
}

fn parse_item(o: &serde_json::Map<String, Value>) -> Option<Item> {
    let ty = o.get("type")?.as_str()?;
    let content = o.get("content")?.as_array()?;
    let id = content.first()?.as_str().unwrap_or("").to_string();
    let command: Vec<u8> = content[1..].iter().filter_map(|v| v.as_u64()).map(|v| v as u8).collect();
    let options = o.get("options");
    let control = match ty {
        "range" => {
            let (min, max) = options
                .and_then(|v| v.as_array())
                .map(|a| {
                    (
                        a.first().and_then(|v| v.as_u64()).unwrap_or(0) as u16,
                        a.get(1).and_then(|v| v.as_u64()).unwrap_or(255) as u16,
                    )
                })
                .unwrap_or((0, 255));
            Control::Range { min, max }
        }
        "dropdown" => {
            let list = options.and_then(|v| v.as_array()).cloned().unwrap_or_default();
            Control::Dropdown(
                list.iter()
                    .enumerate()
                    .map(|(i, opt)| match opt {
                        Value::String(s) => (s.clone(), i as u8),
                        Value::Array(a) => (
                            a.first().and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            a.get(1).and_then(|v| v.as_u64()).unwrap_or(i as u64) as u8,
                        ),
                        _ => (format!("{i}"), i as u8),
                    })
                    .collect(),
            )
        }
        "toggle" => {
            let (off, on) = options
                .and_then(|v| v.as_array())
                .map(|a| (a.first().map(bytes_of).unwrap_or(vec![0]), a.get(1).map(bytes_of).unwrap_or(vec![1])))
                .unwrap_or((vec![0], vec![1]));
            Control::Toggle { off, on }
        }
        "color" => Control::Color,
        "keycode" => Control::Keycode,
        "button" => Control::Button(
            options
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_u64())
                .unwrap_or(1) as u8,
        ),
        _ => Control::Label,
    };
    Some(Item {
        label: o.get("label").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        id,
        command,
        control,
        show_if: o.get("showIf").and_then(|v| v.as_str()).map(String::from),
    })
}

fn parse_node(v: &Value) -> Option<Node> {
    match v {
        Value::String(name) => builtin(name).and_then(|b| parse_node(&b)),
        Value::Object(o) => {
            if o.contains_key("type") {
                return parse_item(o).map(Node::Item);
            }
            let children = o
                .get("content")
                .and_then(|c| c.as_array())
                .map(|a| a.iter().filter_map(parse_node).collect())
                .unwrap_or_default();
            Some(Node::Group {
                label: o.get("label").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                show_if: o.get("showIf").and_then(|v| v.as_str()).map(String::from),
                children,
            })
        }
        _ => None,
    }
}

pub fn parse_menus(menus: &[Value]) -> Vec<Node> {
    menus.iter().filter_map(parse_node).collect()
}

pub fn items(nodes: &[Node]) -> Vec<&Item> {
    let mut out = Vec::new();
    fn walk<'a>(n: &'a [Node], out: &mut Vec<&'a Item>) {
        for node in n {
            match node {
                Node::Group { children, .. } => walk(children, out),
                Node::Item(i) => out.push(i),
            }
        }
    }
    walk(nodes, &mut out);
    out
}

/// Decodes a stored value to the number showIf expressions and sliders work with.
pub fn numeric(item: &Item, raw: &[u8]) -> u32 {
    match (&item.control, raw.len()) {
        (Control::Range { max, .. }, n) if *max > 255 && n >= 2 => u16::from_be_bytes([raw[0], raw[1]]) as u32,
        (_, 0) => 0,
        _ => raw[0] as u32,
    }
}

const RGB_MATRIX_EFFECTS: &[&str] = &[
    "All Off",
    "Solid Color",
    "Alphas Mods",
    "Gradient Up Down",
    "Gradient Left Right",
    "Breathing",
    "Band Sat",
    "Band Val",
    "Band Pinwheel Sat",
    "Band Pinwheel Val",
    "Band Spiral Sat",
    "Band Spiral Val",
    "Cycle All",
    "Cycle Left Right",
    "Cycle Up Down",
    "Rainbow Moving Chevron",
    "Cycle Out In",
    "Cycle Out In Dual",
    "Cycle Pinwheel",
    "Cycle Spiral",
    "Dual Beacon",
    "Rainbow Beacon",
    "Rainbow Pinwheels",
    "Raindrops",
    "Jellybean Raindrops",
    "Hue Breathing",
    "Hue Pendulum",
    "Hue Wave",
    "Pixel Rain",
    "Pixel Flow",
    "Pixel Fractal",
    "Typing Heatmap",
    "Digital Rain",
    "Solid Reactive Simple",
    "Solid Reactive",
    "Solid Reactive Wide",
    "Solid Reactive Multiwide",
    "Solid Reactive Cross",
    "Solid Reactive Multicross",
    "Solid Reactive Nexus",
    "Solid Reactive Multinexus",
    "Splash",
    "Multisplash",
    "Solid Splash",
    "Solid Multisplash",
];

const RGBLIGHT_EFFECTS: &[&str] = &[
    "All Off",
    "Solid Color",
    "Breathing 1",
    "Breathing 2",
    "Breathing 3",
    "Breathing 4",
    "Rainbow Mood 1",
    "Rainbow Mood 2",
    "Rainbow Mood 3",
    "Rainbow Swirl 1",
    "Rainbow Swirl 2",
    "Rainbow Swirl 3",
    "Rainbow Swirl 4",
    "Rainbow Swirl 5",
    "Rainbow Swirl 6",
    "Snake 1",
    "Snake 2",
    "Snake 3",
    "Snake 4",
    "Snake 5",
    "Snake 6",
    "Knight 1",
    "Knight 2",
    "Knight 3",
    "Christmas",
    "Gradient 1",
    "Gradient 2",
    "Gradient 3",
    "Gradient 4",
    "Gradient 5",
    "Gradient 6",
    "Gradient 7",
    "Gradient 8",
    "Gradient 9",
    "Gradient 10",
    "RGB Test",
    "Alternating",
    "Twinkle 1",
    "Twinkle 2",
    "Twinkle 3",
    "Twinkle 4",
    "Twinkle 5",
    "Twinkle 6",
];

/// VIA's stock menus that definitions can reference by name.
fn builtin(name: &str) -> Option<Value> {
    let effect_list = |names: &[&str]| -> Value { names.iter().enumerate().map(|(i, n)| json!([n, i])).collect() };
    Some(match name {
        "qmk_backlight" => json!({"label": "Lighting", "content": [{"label": "Backlight", "content": [
            {"label": "Brightness", "type": "range", "options": [0, 255], "content": ["id_qmk_backlight_brightness", 1, 1]},
            {"label": "Effect", "type": "dropdown", "options": [["Disabled", 0], ["Breathing", 1]], "content": ["id_qmk_backlight_effect", 1, 2]}
        ]}]}),
        "qmk_rgblight" => json!({"label": "Lighting", "content": [{"label": "Underglow", "content": [
            {"label": "Brightness", "type": "range", "options": [0, 255], "content": ["id_qmk_rgblight_brightness", 2, 1]},
            {"label": "Effect", "type": "dropdown", "options": effect_list(RGBLIGHT_EFFECTS), "content": ["id_qmk_rgblight_effect", 2, 2]},
            {"showIf": "{id_qmk_rgblight_effect} != 0", "label": "Effect Speed", "type": "range", "options": [0, 255], "content": ["id_qmk_rgblight_effect_speed", 2, 3]},
            {"showIf": "{id_qmk_rgblight_effect} != 0 && {id_qmk_rgblight_effect} != 35", "label": "Color", "type": "color", "content": ["id_qmk_rgblight_color", 2, 4]}
        ]}]}),
        "qmk_backlight_rgblight" => json!({"label": "Lighting", "content": [
            builtin("qmk_backlight")?["content"][0].clone(),
            builtin("qmk_rgblight")?["content"][0].clone()
        ]}),
        "qmk_rgb_matrix" => json!({"label": "Lighting", "content": [{"label": "Backlight", "content": [
            {"label": "Brightness", "type": "range", "options": [0, 255], "content": ["id_qmk_rgb_matrix_brightness", 3, 1]},
            {"label": "Effect", "type": "dropdown", "options": effect_list(RGB_MATRIX_EFFECTS), "content": ["id_qmk_rgb_matrix_effect", 3, 2]},
            {"showIf": "{id_qmk_rgb_matrix_effect} != 0", "label": "Effect Speed", "type": "range", "options": [0, 255], "content": ["id_qmk_rgb_matrix_effect_speed", 3, 3]},
            {"showIf": "{id_qmk_rgb_matrix_effect} != 0", "label": "Color", "type": "color", "content": ["id_qmk_rgb_matrix_color", 3, 4]}
        ]}]}),
        "qmk_led_matrix" => json!({"label": "Lighting", "content": [{"label": "Backlight", "content": [
            {"label": "Brightness", "type": "range", "options": [0, 255], "content": ["id_qmk_led_matrix_brightness", 5, 1]},
            {"label": "Effect", "type": "range", "options": [0, 255], "content": ["id_qmk_led_matrix_effect", 5, 2]},
            {"label": "Effect Speed", "type": "range", "options": [0, 255], "content": ["id_qmk_led_matrix_effect_speed", 5, 3]}
        ]}]}),
        "qmk_audio" => json!({"label": "Audio", "content": [{"label": "Audio", "content": [
            {"label": "Enable Audio", "type": "toggle", "content": ["id_qmk_audio_enable", 4, 1]},
            {"label": "Enable Audio Clicky", "type": "toggle", "content": ["id_qmk_audio_clicky_enable", 4, 2]}
        ]}]}),
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// showIf expressions: `{id_x} != 0 && ({id_y} == 2 || !{id_z})`

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(i64),
    Var(String),
    Op(&'static str),
    LParen,
    RParen,
}

fn tokenize(s: &str) -> Option<Vec<Tok>> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }
        if ch == '{' {
            let end = c[i..].iter().position(|&x| x == '}')? + i;
            out.push(Tok::Var(c[i + 1..end].iter().collect::<String>().trim().to_string()));
            i = end + 1;
            continue;
        }
        if ch.is_ascii_digit() {
            let start = i;
            while i < c.len() && (c[i].is_ascii_alphanumeric()) {
                i += 1;
            }
            let t: String = c[start..i].iter().collect();
            let n = if let Some(h) = t.strip_prefix("0x") {
                i64::from_str_radix(h, 16).ok()?
            } else {
                t.parse().ok()?
            };
            out.push(Tok::Num(n));
            continue;
        }
        let two: String = c[i..(i + 2).min(c.len())].iter().collect();
        let op = ["==", "!=", "<=", ">=", "&&", "||"].into_iter().find(|o| *o == two);
        if let Some(op) = op {
            out.push(Tok::Op(op));
            i += 2;
            continue;
        }
        match ch {
            '(' => out.push(Tok::LParen),
            ')' => out.push(Tok::RParen),
            '<' => out.push(Tok::Op("<")),
            '>' => out.push(Tok::Op(">")),
            '!' => out.push(Tok::Op("!")),
            _ => return None,
        }
        i += 1;
    }
    Some(out)
}

struct Parser<'a> {
    toks: Vec<Tok>,
    pos: usize,
    vars: &'a dyn Fn(&str) -> i64,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn eat_op(&mut self, op: &'static str) -> bool {
        if self.peek() == Some(&Tok::Op(op)) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn or(&mut self) -> Option<i64> {
        let mut v = self.and()?;
        while self.eat_op("||") {
            let r = self.and()?;
            v = ((v != 0) || (r != 0)) as i64;
        }
        Some(v)
    }
    fn and(&mut self) -> Option<i64> {
        let mut v = self.cmp()?;
        while self.eat_op("&&") {
            let r = self.cmp()?;
            v = ((v != 0) && (r != 0)) as i64;
        }
        Some(v)
    }
    fn cmp(&mut self) -> Option<i64> {
        let l = self.unary()?;
        for op in ["==", "!=", "<=", ">=", "<", ">"] {
            if self.eat_op(op) {
                let r = self.unary()?;
                return Some(match op {
                    "==" => l == r,
                    "!=" => l != r,
                    "<=" => l <= r,
                    ">=" => l >= r,
                    "<" => l < r,
                    _ => l > r,
                } as i64);
            }
        }
        Some(l)
    }
    fn unary(&mut self) -> Option<i64> {
        if self.eat_op("!") {
            return Some((self.unary()? == 0) as i64);
        }
        match self.peek()?.clone() {
            Tok::Num(n) => {
                self.pos += 1;
                Some(n)
            }
            Tok::Var(name) => {
                self.pos += 1;
                Some((self.vars)(&name))
            }
            Tok::LParen => {
                self.pos += 1;
                let v = self.or()?;
                if self.peek() == Some(&Tok::RParen) {
                    self.pos += 1;
                }
                Some(v)
            }
            _ => None,
        }
    }
}

/// Evaluates a showIf expression; malformed expressions show the control.
pub fn eval_show_if(expr: &str, lookup: &dyn Fn(&str) -> i64) -> bool {
    let Some(toks) = tokenize(expr) else { return true };
    let mut p = Parser {
        toks,
        pos: 0,
        vars: lookup,
    };
    p.or().map(|v| v != 0).unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_if() {
        let look = |n: &str| match n {
            "id_a" => 3,
            "id_b" => 0,
            _ => 0,
        };
        assert!(eval_show_if("{id_a} != 0", &look));
        assert!(!eval_show_if("{id_b} != 0", &look));
        assert!(eval_show_if("{id_a} == 3 && ({id_b} == 1 || !{id_b})", &look));
        assert!(!eval_show_if("{id_a} != 0 && {id_a} != 3", &look));
        assert!(eval_show_if("{id_a} >= 0x3", &look));
    }
}
