//! The M-Vave Tank-G: protocol ported from TabJam, pages from the octo hub design.
//!
//! The port is opened only while the pedal screen is shown and closed when it
//! is left, so TabJam can still reach the pedal. Edits sound live and are only
//! stored on Save; "changed" means the live block differs from the stored preset.

mod frame;
mod port;
pub mod preset;
mod presets;
mod tankg;
mod tone;
mod worker;

use crate::app::Note;
use crate::home::Headline;
use crate::icons::{self, Device};
use crate::{theme, widgets};
use eframe::egui;
use egui::RichText;
use preset::Preset;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use worker::{Command, Event, Snapshot, Worker};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Tone,
    Presets,
}

const TABS: [(Tab, &str, &str); 2] = [
    (Tab::Tone, "Tone", "tone"),
    (Tab::Presets, "Presets", "presets"),
];

pub enum Confirm {
    Restore {
        name: String,
        table: Vec<u8>,
        slots: Vec<usize>,
    },
}

pub struct Pedal {
    worker: Worker,
    pub state: Option<Snapshot>,
    /// why the last connection attempt failed
    pub missing: Option<String>,
    connecting: bool,
    simulated: bool,
    demo: bool,
    pub tab: Tab,
    /// the live block as edited here; follows the pedal between edits
    pub live: Option<Preset>,
    last_edit: Instant,
    /// threshold / IR a block gets back when switched on again
    pub remembered: HashMap<usize, u8>,
    pub confirm: Option<Confirm>,
    pub notes: Vec<Note>,
    /// knob being dragged: (block, control, value at grab, pointer y at grab)
    pub drag: Option<(usize, usize, i32, f32)>,
    /// the preset last seen playing, shown on the home card while the port is closed
    last: Option<(usize, bool)>,
}

impl Pedal {
    pub fn new(demo: bool, wake: impl Fn() + Send + 'static) -> Self {
        Pedal {
            worker: Worker::spawn(wake),
            state: None,
            missing: None,
            connecting: false,
            simulated: demo,
            demo,
            tab: Tab::Tone,
            live: None,
            last_edit: Instant::now() - Duration::from_secs(10),
            remembered: HashMap::new(),
            confirm: None,
            notes: vec![],
            drag: None,
            last: None,
        }
    }

    pub fn tab_names(&self) -> Vec<&'static str> {
        if self.state.is_some() {
            TABS.iter().map(|t| t.1).collect()
        } else {
            vec![]
        }
    }
    pub fn tab_index(&self) -> usize {
        TABS.iter().position(|t| t.0 == self.tab).unwrap_or(0)
    }
    pub fn set_tab_index(&mut self, i: usize) {
        self.tab = TABS[i].0;
    }
    pub fn set_tab(&mut self, key: &str) {
        if let Some(t) = TABS.iter().find(|t| t.2 == key) {
            self.tab = t.0;
        }
    }

    /// The pedal screen opened: take the port.
    pub fn open(&mut self) {
        self.connect(self.simulated);
    }

    /// The pedal screen closed: give the port back so other apps can use it.
    pub fn close(&mut self) {
        if !self.simulated {
            self.worker.send(Command::Release);
            self.state = None;
            self.live = None;
        }
    }

    pub fn connect(&mut self, simulated: bool) {
        self.simulated = simulated;
        self.connecting = true;
        self.worker.send(Command::Connect { simulated });
    }

    pub fn update(&mut self, on_screen: bool) {
        for e in self.worker.poll() {
            match e {
                Event::Connected(s) => {
                    self.connecting = false;
                    self.missing = None;
                    self.live = s.live;
                    self.state = Some(s);
                    if !on_screen && !self.simulated {
                        self.worker.send(Command::Release);
                        self.state = None;
                    }
                }
                Event::State(s) => {
                    // a poll that started before an edit must not undo it on screen
                    if self.last_edit.elapsed() > Duration::from_millis(1500) || s.live.is_none() {
                        self.live = s.live;
                    }
                    self.state = Some(s);
                }
                Event::NotFound { why, quiet } => {
                    self.connecting = false;
                    if !quiet || self.missing.is_none() {
                        self.missing = Some(why);
                    }
                }
                Event::Lost(why) => {
                    self.state = None;
                    self.live = None;
                    self.missing = Some(why.clone());
                    self.notes.push((format!("Lost the pedal: {why}"), false));
                }
                Event::Done(msg) => self.notes.push((msg, true)),
                Event::Failed(msg) => self.notes.push((msg, false)),
            }
            self.remember();
        }
    }

    fn remember(&mut self) {
        if let Some(s) = self.sounding() {
            self.last = Some((s, self.bypassed()));
        }
    }

    pub fn sounding(&self) -> Option<usize> {
        self.state.as_ref().and_then(|s| s.sounding)
    }

    pub fn bypassed(&self) -> bool {
        self.state.as_ref().is_some_and(|s| s.bypassed)
    }

    pub fn stored(&self, slot: usize) -> Option<Preset> {
        self.state
            .as_ref()
            .filter(|s| preset::table_ok(&s.table))
            .map(|s| preset::from_table(&s.table, slot))
    }

    pub fn dirty(&self) -> bool {
        match (self.sounding(), self.live) {
            (Some(s), Some(l)) => self.stored(s).is_some_and(|st| st != l),
            _ => false,
        }
    }

    pub fn can_edit(&self) -> bool {
        self.state.is_some() && self.sounding().is_some() && !self.bypassed() && self.live.is_some()
    }

    pub fn send(&mut self, c: Command) {
        self.worker.send(c);
    }

    /// Changes one byte of the live block and sends it the way the pedal takes it.
    pub fn set_byte(&mut self, offset: usize, value: u8) {
        let Some(live) = self.live.as_mut() else {
            return;
        };
        if live[offset] == value || !self.can_edit() {
            return;
        }
        let live = self.live.as_mut().unwrap();
        live[offset] = value;
        self.last_edit = Instant::now();
        if offset <= 3 {
            self.worker.send(Command::Switch {
                offset,
                on: value != 0,
            });
        } else {
            let p = *live;
            if preset::on_default(offset).is_some() && value != 0 {
                self.remembered.insert(offset, value);
            }
            self.worker.send(Command::Knobs(p));
        }
    }

    pub fn summary(&self) -> (bool, String) {
        match (&self.state, self.sounding()) {
            (Some(_), Some(s)) if self.bypassed() => {
                (true, format!("Preset {} · bypassed", preset::name(s)))
            }
            (Some(_), Some(s)) => (true, format!("Preset {}", preset::name(s))),
            (Some(_), None) => (true, "Connected".into()),
            (None, _) if self.present() => (true, "Connected · USB MIDI".into()),
            (None, _) => (false, "Not connected".into()),
        }
    }

    /// Whether the pedal is plugged in, without opening its port.
    fn present(&self) -> bool {
        self.simulated || port::find().is_some_and(|p| p.exists())
    }

    pub fn headline(&self) -> Headline {
        let connected = self.state.is_some() || self.present();
        let now = self.sounding().map(|s| (s, self.bypassed()));
        let (head, unit) = match now.or(self.last) {
            Some((s, bypassed)) => (
                preset::name(s),
                if bypassed { "bypassed" } else { "preset" },
            ),
            None if connected => ("—".into(), "preset"),
            None => ("Not connected".into(), ""),
        };
        Headline {
            connected,
            dot: None,
            head,
            unit: unit.into(),
            battery: None,
            conn: if self.simulated {
                "Connected · simulated pedal".into()
            } else if connected {
                "Connected · USB MIDI".into()
            } else {
                "Not found. Plug it in over USB".into()
            },
        }
    }

    /// Top bar: "Playing 1A", plus "· bypassed".
    pub fn status(&mut self, ui: &mut egui::Ui) {
        let Some(s) = self.sounding().filter(|_| self.state.is_some()) else {
            return;
        };
        ui.add_space(6.0);
        ui.spacing_mut().item_spacing.x = 8.0;
        if self.bypassed() {
            ui.label(widgets::dim("· bypassed", 13.0));
        }
        ui.label(
            RichText::new(preset::name(s))
                .font(widgets::mono(13.0))
                .strong()
                .color(theme::ACCENT),
        );
        ui.label(widgets::dim("Playing", 13.0));
    }

    pub fn page(&mut self, ui: &mut egui::Ui) {
        if self.state.is_none() {
            self.missing_page(ui);
            return;
        }
        match self.tab {
            Tab::Tone => tone::page(self, ui),
            Tab::Presets => presets::page(self, ui),
        }
    }

    fn missing_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(60.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(420.0);
            ui.spacing_mut().item_spacing.y = 14.0;
            let r = icons::allocate(ui, 72.0 + 14.0);
            icons::device(ui.painter(), icons::square(r.center() - egui::vec2(0.0, 7.0), 72.0), Device::Pedal, theme::TEXT_FAINT, 2.0, true);
            if self.connecting && self.missing.is_none() {
                ui.label(RichText::new("Looking for the Tank-G…").size(18.0).color(theme::TEXT));
                return;
            }
            ui.label(RichText::new("Tank-G not found").size(18.0).color(theme::TEXT));
            let busy = self.missing.as_deref().is_some_and(|m| m.contains("in use"));
            let text = if busy {
                "Another app has the pedal open. Close TabJam's pedal panel, then try again."
            } else {
                "Plug it in over USB. If it's passed through to a virtual machine, give it back to Linux first."
            };
            ui.add(egui::Label::new(RichText::new(text).size(13.5).color(theme::TEXT_DIM)).wrap());
            ui.add_space(6.0);
            let w = 100.0 + 8.0 + 190.0;
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - w) / 2.0).max(0.0));
                if widgets::primary(ui, "Try again").clicked() {
                    self.connect(false);
                }
                if widgets::secondary(ui, "Use a simulated pedal").clicked() {
                    self.connect(true);
                }
            });
            if self.demo {
                return;
            }
        });
    }

    pub fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(Confirm::Restore { name, table, slots }) = &self.confirm else {
            return;
        };
        let names: Vec<String> = slots.iter().map(|s| preset::name(*s)).collect();
        let text = format!(
            "Write {} presets from {name} to the pedal: {}? A copy of what is on the pedal now is kept first.",
            slots.len(),
            names.join(" ")
        );
        match crate::keyboard::dialog::modal(
            ctx,
            "pedal-restore",
            "Restore presets?",
            &[text.as_str()],
            "Restore",
            false,
        ) {
            Some(true) => {
                let (table, slots) = (table.clone(), slots.clone());
                self.confirm = None;
                self.worker.send(Command::Restore { table, slots });
            }
            Some(false) => self.confirm = None,
            None => {}
        }
    }

    pub fn backup_dir() -> std::path::PathBuf {
        worker::backup_dir()
    }

    pub fn stamp() -> String {
        worker::stamp()
    }
}
