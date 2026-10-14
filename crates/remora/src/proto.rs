//! Report layouts of the Attack Shark X6 2.4G receiver (1d57:fa60, interface 2).
//!
//! Every setting is written with HID SET_FEATURE and the receiver answers on the
//! interrupt endpoint with `03 10 50 00 <report id>`. Nothing can be read back
//! (GET_FEATURE stalls), so the app keeps the canonical state itself. The
//! layouts follow blak0p/attack-shark-linux (MIT), which decoded them from USB
//! captures of the vendor app; the tests below replay those captures.

use serde::{Deserialize, Serialize};

pub const VENDOR_ID: u16 = 0x1d57;
pub const PRODUCT_ID: u16 = 0xfa60;

pub const SENSOR_REPORT: u8 = 0x04;
pub const LIGHT_REPORT: u8 = 0x05;
pub const POLLING_REPORT: u8 = 0x06;
pub const BUTTONS_REPORT: u8 = 0x08;

pub const STAGES: usize = 8;
pub const DPI_MIN: u32 = 50;
pub const DPI_MAX: u32 = 26000;
pub const DPI_STEP: u32 = 50;

pub type Rgb = [u8; 3];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stage {
    pub dpi: u32,
    pub color: Rgb,
}

/// Report 0x04: DPI stages, their LED colours and the sensor toggles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sensor {
    pub stages: [Stage; STAGES],
    /// how many stages, counted from the first, the DPI buttons cycle through
    pub stage_count: u8,
    /// 1-based, as on the wire
    pub active: u8,
    pub lift_2mm: bool,
    pub angle_snap: bool,
    pub ripple: bool,
}

impl Default for Sensor {
    /// The vendor app's factory reset.
    fn default() -> Self {
        let dpi = [800, 1200, 1600, 3200, 5600, 26000, 50, 50];
        let colors = [
            [0xff, 0x00, 0x00],
            [0x00, 0xff, 0x00],
            [0x00, 0x00, 0xff],
            [0xff, 0xff, 0x00],
            [0x00, 0xff, 0xff],
            [0xff, 0x00, 0xff],
            [0xff, 0x40, 0x00],
            [0xff, 0xff, 0xff],
        ];
        Sensor {
            stages: std::array::from_fn(|i| Stage { dpi: dpi[i], color: colors[i] }),
            stage_count: 6,
            active: 2,
            lift_2mm: true,
            angle_snap: false,
            ripple: false,
        }
    }
}

pub fn clamp_dpi(dpi: u32) -> u32 {
    let dpi = dpi.clamp(DPI_MIN, DPI_MAX);
    (dpi + DPI_STEP / 2) / DPI_STEP * DPI_STEP
}

impl Sensor {
    pub fn active_dpi(&self) -> u32 {
        self.stages[(self.active.clamp(1, STAGES as u8) - 1) as usize].dpi
    }

    /// Brings every field into the range the mouse accepts.
    pub fn normalize(&mut self) {
        self.stage_count = self.stage_count.clamp(1, STAGES as u8);
        self.active = self.active.clamp(1, self.stage_count);
        for s in &mut self.stages {
            s.dpi = clamp_dpi(s.dpi);
        }
    }

    pub fn encode(&self) -> [u8; 52] {
        let mut s = self.clone();
        s.normalize();
        let mut r = [0u8; 52];
        r[0] = SENSOR_REPORT;
        r[1] = 0x38;
        r[2] = 0x01;
        r[3] = s.angle_snap as u8;
        r[4] = s.ripple as u8;
        r[5] = ((1u16 << s.stage_count) - 1) as u8;
        r[6] = s.angle_snap as u8;
        r[7] = s.lift_2mm as u8;
        for (i, st) in s.stages.iter().enumerate() {
            let raw = (st.dpi / DPI_STEP - 1) as u16;
            r[8 + i] = raw as u8;
            r[16 + i] = (raw >> 8) as u8;
            // RGB order: the vendor app copies a little-endian COLORREF
            r[25 + i * 3..28 + i * 3].copy_from_slice(&st.color);
        }
        r[24] = s.active;
        r[49] = 0x01;
        let sum: u16 = r[3..50].iter().map(|&b| b as u16).sum();
        r[50..52].copy_from_slice(&sum.to_be_bytes());
        r
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightMode {
    Off,
    Steady,
    Breathing,
    Neon,
    ColorBreathing,
    /// lit in the active DPI stage's colour
    StageColor,
    /// breathing in the active DPI stage's colour
    StageBreathing,
}

impl LightMode {
    pub const ALL: [LightMode; 7] = [
        LightMode::StageColor,
        LightMode::StageBreathing,
        LightMode::Steady,
        LightMode::Breathing,
        LightMode::Neon,
        LightMode::ColorBreathing,
        LightMode::Off,
    ];

    pub fn byte(self) -> u8 {
        match self {
            LightMode::Off => 0x00,
            LightMode::Steady => 0x10,
            LightMode::Breathing => 0x20,
            LightMode::Neon => 0x30,
            LightMode::ColorBreathing => 0x40,
            LightMode::StageColor => 0x50,
            LightMode::StageBreathing => 0x60,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            LightMode::Off => "Off",
            LightMode::Steady => "Steady",
            LightMode::Breathing => "Breathing",
            LightMode::Neon => "Neon",
            LightMode::ColorBreathing => "Colour breathing",
            LightMode::StageColor => "DPI colour",
            LightMode::StageBreathing => "DPI breathing",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            LightMode::Off => "off",
            LightMode::Steady => "steady",
            LightMode::Breathing => "breathing",
            LightMode::Neon => "neon",
            LightMode::ColorBreathing => "colour-breathing",
            LightMode::StageColor => "dpi",
            LightMode::StageBreathing => "dpi-breathing",
        }
    }

    pub fn from_key(s: &str) -> Option<LightMode> {
        LightMode::ALL.into_iter().find(|m| m.key() == s || (s == "color-breathing" && *m == LightMode::ColorBreathing))
    }

    /// Whether the mode uses the colour picked on the lighting page.
    pub fn uses_color(self) -> bool {
        matches!(self, LightMode::Steady | LightMode::Breathing)
    }

    pub fn uses_speed(self) -> bool {
        !matches!(self, LightMode::Off | LightMode::Steady | LightMode::StageColor)
    }
}

pub const SPEED_MAX: u8 = 5;
pub const SLEEP_HALF_MINUTES_MAX: u8 = 120;
pub const DEEP_SLEEP_MAX: u8 = 60;
pub const DEBOUNCE_MIN: u8 = 2;
pub const DEBOUNCE_MAX: u8 = 25;

/// Report 0x05. Besides the LED effect it carries the sleep timers and the
/// button debounce, so all of them are always written together.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Light {
    pub mode: LightMode,
    pub color: Rgb,
    /// 1 (slow) to 5; the captures only show 2 to 4
    pub speed: u8,
    /// idle time before the LEDs and sensor sleep, in half minutes
    pub sleep_half_minutes: u8,
    /// idle time before the radio goes to deep sleep, in minutes
    pub deep_sleep_minutes: u8,
    pub debounce_ms: u8,
}

impl Default for Light {
    /// The vendor app's factory reset.
    fn default() -> Self {
        Light {
            mode: LightMode::StageColor,
            color: [0x00, 0xff, 0x00],
            speed: 3,
            sleep_half_minutes: 1,
            deep_sleep_minutes: 10,
            debounce_ms: 4,
        }
    }
}

impl Light {
    pub fn normalize(&mut self) {
        self.speed = self.speed.clamp(1, SPEED_MAX);
        self.sleep_half_minutes = self.sleep_half_minutes.clamp(1, SLEEP_HALF_MINUTES_MAX);
        self.deep_sleep_minutes = self.deep_sleep_minutes.clamp(1, DEEP_SLEEP_MAX);
        self.debounce_ms = self.debounce_ms.clamp(DEBOUNCE_MIN, DEBOUNCE_MAX);
    }

    pub fn encode(&self) -> [u8; 13] {
        let mut l = self.clone();
        l.normalize();
        // deep sleep is a 12-bit count of 1/16 minutes, offset by half a minute;
        // its top four bits share a byte with the speed
        let deep = l.deep_sleep_minutes as u16 * 16 + 8;
        let mut r = [0u8; 13];
        r[0] = LIGHT_REPORT;
        r[1] = 0x0f;
        r[2] = 0x01;
        r[3] = l.mode.byte();
        r[4] = ((deep >> 8) as u8) << 4 | l.speed;
        r[5] = deep as u8;
        // BGR here, unlike report 0x04
        r[6] = l.color[2];
        r[7] = l.color[1];
        r[8] = l.color[0];
        r[9] = l.sleep_half_minutes;
        r[10] = l.debounce_ms;
        let sum: u16 = r[3..11].iter().map(|&b| b as u16).sum();
        r[11..13].copy_from_slice(&sum.to_be_bytes());
        r
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Polling {
    Hz125,
    Hz250,
    Hz500,
    Hz1000,
}

impl Polling {
    pub const ALL: [Polling; 4] = [Polling::Hz125, Polling::Hz250, Polling::Hz500, Polling::Hz1000];

    pub fn hz(self) -> u32 {
        match self {
            Polling::Hz125 => 125,
            Polling::Hz250 => 250,
            Polling::Hz500 => 500,
            Polling::Hz1000 => 1000,
        }
    }

    pub fn from_hz(hz: u32) -> Option<Polling> {
        Polling::ALL.into_iter().find(|p| p.hz() == hz)
    }

    pub fn encode(self) -> [u8; 9] {
        // the interval in milliseconds and its complement
        let v = (1000 / self.hz()) as u8;
        [POLLING_REPORT, 0x09, 0x01, v, 0xff - v, 0, 0, 0, 0]
    }
}

/// One button binding: an action id and two parameter bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Action(pub u8, pub u8, pub u8);

pub const SHORTCUT: u8 = 0x11;
pub const EASY_AIM: u8 = 0x10;

pub const MOD_CTRL: u8 = 0x01;
pub const MOD_SHIFT: u8 = 0x02;
pub const MOD_ALT: u8 = 0x04;
pub const MOD_SUPER: u8 = 0x08;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Mouse,
    Dpi,
    Media,
    Launch,
    Special,
}

impl Category {
    pub const ALL: [Category; 5] = [Category::Mouse, Category::Dpi, Category::Media, Category::Launch, Category::Special];

    pub fn name(self) -> &'static str {
        match self {
            Category::Mouse => "Mouse",
            Category::Dpi => "DPI",
            Category::Media => "Media",
            Category::Launch => "Launch",
            Category::Special => "Special",
        }
    }
}

/// Fixed actions (without parameters) in the order the picker shows them.
pub const ACTIONS: &[(u8, &str, Category)] = &[
    (0x02, "Left click", Category::Mouse),
    (0x03, "Right click", Category::Mouse),
    (0x04, "Middle click", Category::Mouse),
    (0x05, "Back", Category::Mouse),
    (0x06, "Forward", Category::Mouse),
    (0x07, "Double click", Category::Mouse),
    (0x08, "Fire (rapid click)", Category::Mouse),
    (0x09, "Scroll up", Category::Mouse),
    (0x0a, "Scroll down", Category::Mouse),
    (0x01, "Disabled", Category::Mouse),
    (0x0d, "DPI cycle", Category::Dpi),
    (0x0e, "DPI +", Category::Dpi),
    (0x0f, "DPI −", Category::Dpi),
    (0x18, "Play / pause", Category::Media),
    (0x19, "Stop", Category::Media),
    (0x16, "Previous track", Category::Media),
    (0x17, "Next track", Category::Media),
    (0x1b, "Volume +", Category::Media),
    (0x1c, "Volume −", Category::Media),
    (0x1a, "Mute", Category::Media),
    (0x15, "Media player", Category::Media),
    (0x20, "Browser forward", Category::Launch),
    (0x21, "Browser back", Category::Launch),
    (0x24, "Browser refresh", Category::Launch),
    (0x22, "Browser stop", Category::Launch),
    (0x25, "Browser home", Category::Launch),
    (0x26, "Search", Category::Launch),
    (0x1d, "Calculator", Category::Launch),
    (0x1e, "E-mail", Category::Launch),
    (0x23, "File manager", Category::Launch),
];

impl Action {
    pub const fn simple(id: u8) -> Action {
        Action(id, 0, 0)
    }

    pub fn shortcut(mods: u8, key: u8) -> Action {
        Action(SHORTCUT, mods, key)
    }

    pub fn easy_aim(level: u8) -> Action {
        Action(EASY_AIM, 0, level)
    }

    pub fn label(self) -> String {
        match self.0 {
            SHORTCUT => shortcut_label(self.1, self.2),
            EASY_AIM => format!("Easy aim (level {})", self.2),
            id => match ACTIONS.iter().find(|a| a.0 == id) {
                Some(a) => a.1.to_string(),
                None => format!("Action {:02x} {:02x} {:02x}", self.0, self.1, self.2),
            },
        }
    }

    pub fn category(self) -> Category {
        match self.0 {
            SHORTCUT | EASY_AIM => Category::Special,
            id => ACTIONS.iter().find(|a| a.0 == id).map(|a| a.2).unwrap_or(Category::Special),
        }
    }

    /// Actions the left button may take, so it can never lose its click
    /// along with every way of getting it back.
    pub fn allowed_on_left(self) -> bool {
        matches!(self.0, 0x02..=0x08)
    }
}

pub fn shortcut_label(mods: u8, key: u8) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (bit, name) in [(MOD_CTRL, "Ctrl"), (MOD_SHIFT, "Shift"), (MOD_ALT, "Alt"), (MOD_SUPER, "Super")] {
        if mods & bit != 0 {
            parts.push(name.into());
        }
    }
    if key != 0 {
        parts.push(crate::keys::name(key));
    }
    if parts.is_empty() { "Shortcut (empty)".into() } else { parts.join("+") }
}

/// The physical buttons, in the order the app lists them.
pub const BUTTONS: [&str; 7] = ["Left", "Right", "Wheel", "Forward", "Back", "DPI +", "DPI −"];
/// Slot of each physical button in report 0x08 (1-based, three bytes each).
const BUTTON_GROUPS: [usize; 7] = [1, 2, 3, 7, 8, 5, 6];

pub const DEFAULT_BUTTONS: [Action; 7] = [
    Action::simple(0x02),
    Action::simple(0x03),
    Action::simple(0x04),
    Action::simple(0x06),
    Action::simple(0x05),
    Action::simple(0x0e),
    Action::simple(0x0f),
];

/// The vendor app's factory report 0x08; slots the app does not expose
/// (DPI cycle, 9-16 and the wheel scroll in 17-18) keep these values.
const BUTTONS_BASELINE: [u8; 59] = [
    0x08, 0x3b, 0x01, 0x02, 0x00, 0x00, 0x03, 0x00, 0x00, 0x04, 0x00, 0x00, 0x0d, 0x00, 0x00, 0x0e, 0x00, 0x00, 0x0f, 0x00,
    0x00, 0x06, 0x00, 0x00, 0x05, 0x00, 0x00, 0x3c, 0x00, 0x00, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x01,
    0x00, 0x00, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x0a, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00, 0x94,
];

pub fn encode_buttons(buttons: &[Action; 7]) -> [u8; 59] {
    let mut r = BUTTONS_BASELINE;
    for (i, a) in buttons.iter().enumerate() {
        let a = if i == 0 && !a.allowed_on_left() { DEFAULT_BUTTONS[0] } else { *a };
        let at = 3 + (BUTTON_GROUPS[i] - 1) * 3;
        r[at] = a.0;
        r[at + 1] = a.1;
        r[at + 2] = a.2;
    }
    let sum: u16 = r[3..57].iter().map(|&b| b as u16).sum();
    r[57..59].copy_from_slice(&sum.to_be_bytes());
    r
}

/// Where the mouse draws power from, byte 3 of the heartbeat. The values come
/// from the R1 driver (jlucaso1/attack-shark-r1), which shares this receiver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Power {
    Battery,
    Cable,
    Wireless,
    Dock,
    Unknown(u8),
}

impl Power {
    fn from_byte(b: u8) -> Power {
        match b {
            0x01 => Power::Battery,
            0x02 => Power::Cable,
            0x03 => Power::Wireless,
            b if b & 0x80 != 0 => Power::Dock,
            b => Power::Unknown(b),
        }
    }

    pub fn charging(self) -> bool {
        matches!(self, Power::Cable | Power::Wireless | Power::Dock)
    }

    pub fn describe(self) -> String {
        match self {
            Power::Battery => "on battery".into(),
            Power::Cable => "charging by cable".into(),
            Power::Wireless => "charging wirelessly".into(),
            Power::Dock => "charging on the dock".into(),
            Power::Unknown(b) => format!("power state {b:02x}"),
        }
    }
}

/// The mouse reports charge in tenths, so 100 means somewhere in the top step.
pub fn battery_text(percent: u8) -> String {
    format!("~{percent}%")
}

/// Input report 0x03, pushed by the receiver on interface 2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// every two seconds while the mouse is awake; battery in steps of 10%
    Heartbeat { battery: u8, power: Power },
    /// the DPI buttons changed the active stage (1-based)
    Stage(u8),
    /// a feature report was accepted
    Ack(u8),
    Other(Vec<u8>),
}

pub fn decode_status(r: &[u8]) -> Option<Status> {
    if r.first() != Some(&0x03) {
        return None;
    }
    if r.len() < 5 || r[1] != 0x10 {
        return Some(Status::Other(r.to_vec()));
    }
    Some(match r[2] {
        0x40 => Status::Heartbeat {
            battery: r[4].saturating_mul(10).min(100),
            power: Power::from_byte(r[3]),
        },
        0x10 => Status::Stage(r[3]),
        0x50 => Status::Ack(r[4]),
        _ => Status::Other(r.to_vec()),
    })
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unhex(s: &str) -> Vec<u8> {
        s.split_whitespace().map(|h| u8::from_str_radix(h, 16).unwrap()).collect()
    }

    #[test]
    fn factory_sensor_matches_reset_capture() {
        let want = unhex(
            "04 38 01 00 00 3f 00 01 0f 17 1f 3f 6f 07 00 00 00 00 00 00 00 02 00 00 02 ff 00 00 00 ff
             00 00 00 ff ff ff 00 00 ff ff ff 00 ff ff 40 00 ff ff ff 01 0e 72",
        );
        assert_eq!(Sensor::default().encode().to_vec(), want);
    }

    #[test]
    fn sensor_fields_land_where_the_captures_put_them() {
        let mut s = Sensor::default();
        s.active = 4;
        assert_eq!(&s.encode()[50..], &[0x0e, 0x74]);
        s.active = 2;
        s.ripple = true;
        assert_eq!(s.encode()[4], 1);
        assert_eq!(&s.encode()[50..], &[0x0e, 0x73]);
        s.ripple = false;
        s.angle_snap = true;
        let r = s.encode();
        assert_eq!((r[3], r[6]), (1, 1));
        assert_eq!(&r[50..], &[0x0e, 0x74]);
        // the DPI bar capture: 5900 / 14400 / 9600
        s.stages[0].dpi = 5900;
        s.stages[4].dpi = 14400;
        s.stages[5].dpi = 9600;
        let r = s.encode();
        assert_eq!((r[8], r[16]), (0x75, 0x00));
        assert_eq!((r[12], r[20]), (0x1f, 0x01));
        assert_eq!((r[13], r[21]), (0xbf, 0x00));
    }

    #[test]
    fn light_matches_captures() {
        assert_eq!(Light::default().encode().to_vec(), unhex("05 0f 01 50 03 a8 00 ff 00 01 04 01 ff"));
        let base = Light {
            mode: LightMode::Breathing,
            debounce_ms: 4,
            ..Light::default()
        };
        assert_eq!(base.encode().to_vec(), unhex("05 0f 01 20 03 a8 00 ff 00 01 04 01 cf"));
        // colour picks: the payload holds blue, green, red
        let pink = Light { color: [0xfe, 0x5e, 0xf9], ..base.clone() };
        assert_eq!(pink.encode().to_vec(), unhex("05 0f 01 20 03 a8 f9 5e fe 01 04 03 25"));
        let orange = Light { color: [0xff, 0x7f, 0x00], ..base.clone() };
        assert_eq!(orange.encode().to_vec(), unhex("05 0f 01 20 03 a8 00 7f ff 01 04 02 4e"));
        let yellow = Light { color: [0xff, 0xff, 0x00], ..base.clone() };
        let faster = Light { speed: 4, ..yellow.clone() };
        assert_eq!(faster.encode().to_vec(), unhex("05 0f 01 20 04 a8 00 ff ff 01 04 02 cf"));
        // sleep 22 min and deep sleep 45 min
        let sleepy = Light { sleep_half_minutes: 44, deep_sleep_minutes: 45, ..yellow.clone() };
        assert_eq!(sleepy.encode().to_vec(), unhex("05 0f 01 20 23 d8 00 ff ff 2c 04 03 49"));
        for (min, b4, b5) in [(4, 0x03, 0x48), (22, 0x13, 0x68), (30, 0x13, 0xe8)] {
            let r = Light { deep_sleep_minutes: min, ..yellow.clone() }.encode();
            assert_eq!((r[4], r[5]), (b4, b5), "deep sleep {min}");
        }
        let neon = Light { mode: LightMode::Neon, speed: 2, color: [0xff, 0, 0], ..base };
        assert_eq!(neon.encode().to_vec(), unhex("05 0f 01 30 02 a8 00 00 ff 01 04 01 de"));
    }

    #[test]
    fn polling_matches_captures() {
        assert_eq!(Polling::Hz1000.encode().to_vec(), unhex("06 09 01 01 fe 00 00 00 00"));
        assert_eq!(Polling::Hz125.encode().to_vec(), unhex("06 09 01 08 f7 00 00 00 00"));
    }

    #[test]
    fn buttons_match_captures() {
        assert_eq!(encode_buttons(&DEFAULT_BUTTONS), BUTTONS_BASELINE);
        let mut b = DEFAULT_BUTTONS;
        b[5] = Action::simple(0x02);
        let r = encode_buttons(&b);
        assert_eq!(r[15], 0x02);
        assert_eq!(&r[57..], &[0x00, 0x88]);
        b = DEFAULT_BUTTONS;
        b[0] = Action::simple(0x1b);
        assert_eq!(encode_buttons(&b), BUTTONS_BASELINE, "left button refuses media keys");
    }

    #[test]
    fn status_reports() {
        assert_eq!(decode_status(&[3, 0x10, 0x40, 1, 0x0a]), Some(Status::Heartbeat { battery: 100, power: Power::Battery }));
        assert_eq!(
            decode_status(&[3, 0x10, 0x40, 0x80, 0x07]),
            Some(Status::Heartbeat { battery: 70, power: Power::Dock })
        );
        assert_eq!(decode_status(&[3, 0x10, 0x50, 0, 4]), Some(Status::Ack(4)));
        assert_eq!(decode_status(&[3, 0x10, 0x10, 3, 0]), Some(Status::Stage(3)));
        assert_eq!(decode_status(&[1, 0]), None);
    }
}
