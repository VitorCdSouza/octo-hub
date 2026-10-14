//! One Tank-G preset: the 21-byte block that sounds at 0x10000000 and that is
//! stored 36 times in the preset table. Bytes 18–20 are not understood yet and
//! travel untouched. Ported from TabJam's `TankGPreset.cs`.

pub const SIZE: usize = 21;
pub const COUNT: usize = 36;
/// `A5 5A` and the 36 presets: the same 758 bytes the vendor editor's "share all" reads.
pub const TABLE_SIZE: usize = 2 + COUNT * SIZE;

/// The bytes written live in one frame: GATE through CAB IR.
pub const KNOBS_OFFSET: usize = 4;
pub const KNOBS_LEN: usize = 14;

pub type Preset = [u8; SIZE];

/// 1A = 0, 1B = 1 … 9D = 35, as the pedal counts them.
pub fn name(slot: usize) -> String {
    format!("{}{}", slot / 4 + 1, ['A', 'B', 'C', 'D'][slot % 4])
}

pub fn from_table(table: &[u8], slot: usize) -> Preset {
    let mut p = [0u8; SIZE];
    p.copy_from_slice(&table[2 + slot * SIZE..2 + (slot + 1) * SIZE]);
    p
}

pub fn table_ok(t: &[u8]) -> bool {
    t.len() == TABLE_SIZE && t[0] == 0xa5 && t[1] == 0x5a
}

/// A control of a block as the panel shows it: `shown` is the stored byte plus `shown_offset`.
#[derive(Clone, Copy)]
pub struct Control {
    pub label: &'static str,
    pub offset: usize,
    pub min: i32,
    pub max: i32,
    pub shown_offset: i32,
}

impl Control {
    const fn new(
        label: &'static str,
        offset: usize,
        min: i32,
        max: i32,
        shown_offset: i32,
    ) -> Self {
        Control {
            label,
            offset,
            min,
            max,
            shown_offset,
        }
    }

    pub fn shown(&self, p: &Preset) -> i32 {
        p[self.offset] as i32 + self.shown_offset
    }

    pub fn stored(&self, shown: i32) -> u8 {
        (shown - self.shown_offset).clamp(0, 255) as u8
    }
}

/// How the first control is drawn when it is not a knob.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Head {
    None,
    /// a − value + stepper with a prefix, e.g. "Model 3"
    Stepper(&'static str),
    /// a segmented control with these names
    Segments(&'static [&'static str]),
}

/// A block of the signal chain. On/off lives in bytes 0–3 for AMP, MOD, DLY and
/// REV; GATE is off when its threshold is 0 and CAB when its IR slot is 0, as on
/// the pedal's knobs.
pub struct Block {
    pub name: &'static str,
    pub switch: Option<usize>,
    pub head: Head,
    pub controls: &'static [Control],
}

impl Block {
    pub fn is_on(&self, p: &Preset) -> bool {
        match self.switch {
            Some(o) => p[o] != 0,
            None => p[self.controls[0].offset] != 0,
        }
    }

    /// The knobs, after the control the head shows.
    pub fn knobs(&self) -> &'static [Control] {
        if self.head == Head::None {
            self.controls
        } else {
            &self.controls[1..]
        }
    }
}

/// The threshold and IR a block gets when switched on without a remembered value.
pub fn on_default(offset: usize) -> Option<u8> {
    match offset {
        4 => Some(30),
        17 => Some(1),
        _ => None,
    }
}

// The vendor editor's ranges were not captured: these come from the manual (9 amps,
// 3 MOD types, 8 IR slots plus off) and from the factory presets, where amp knobs reach 114.
pub static BLOCKS: [Block; 6] = [
    Block {
        name: "Gate",
        switch: None,
        head: Head::None,
        controls: &[Control::new("Threshold", 4, 1, 100, 0)],
    },
    Block {
        name: "Amp",
        switch: Some(0),
        head: Head::Stepper("Model "),
        controls: &[
            Control::new("Model", 5, 1, 9, 1),
            Control::new("Gain", 6, 0, 127, 0),
            Control::new("Treble", 7, 0, 127, 0),
            Control::new("Middle", 8, 0, 127, 0),
            Control::new("Bass", 9, 0, 127, 0),
            Control::new("Volume", 10, 0, 127, 0),
        ],
    },
    Block {
        name: "Mod",
        switch: Some(1),
        head: Head::Segments(&["Chorus", "Phaser", "Tremolo"]),
        controls: &[
            Control::new("FX", 11, 1, 3, 1),
            Control::new("Speed", 12, 0, 100, 0),
        ],
    },
    Block {
        name: "Delay",
        switch: Some(2),
        head: Head::None,
        controls: &[
            Control::new("Mix", 13, 1, 100, 1),
            Control::new("Time", 14, 0, 100, 0),
        ],
    },
    Block {
        name: "Reverb",
        switch: Some(3),
        head: Head::None,
        controls: &[
            Control::new("Decay", 15, 1, 100, 1),
            Control::new("Mix", 16, 0, 100, 0),
        ],
    },
    Block {
        name: "Cab",
        switch: None,
        head: Head::Stepper("IR slot "),
        controls: &[Control::new("IR", 17, 1, 8, 0)],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(name(0), "1A");
        assert_eq!(name(10), "3C");
        assert_eq!(name(35), "9D");
    }

    #[test]
    fn shown_values() {
        let mut p = [0u8; SIZE];
        p[5] = 2;
        p[13] = 0;
        assert_eq!(BLOCKS[1].controls[0].shown(&p), 3);
        assert_eq!(BLOCKS[3].controls[0].shown(&p), 1);
        assert_eq!(BLOCKS[3].controls[0].stored(50), 49);
    }
}
