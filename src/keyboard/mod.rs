//! The ZUOYA GMK70 through hidra's VIA session.
//!
//! Connecting reads the whole keymap, about a second over the 2.4G receiver,
//! so it runs on a thread; edits afterwards are single VIA commands (~18 ms)
//! and run in place, as in hidra.

mod board;
mod device;
pub mod dialog;
mod keymap;
mod lighting;
mod macros_page;
mod tester;

use crate::app::Note;
use crate::home::Headline;
use crate::icons::{self, Device};
use crate::{theme, widgets};
use anyhow::Result;
use eframe::egui;
use egui::{RichText, Sense, Stroke, vec2};
use hidra::hid;
use hidra::modes::{self, Mode, State};
use hidra::session::{self, Session};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Keymap,
    Lighting,
    Macros,
    Tester,
    Device,
}

const TABS: [(Tab, &str, &str); 5] = [
    (Tab::Keymap, "Keymap", "keymap"),
    (Tab::Lighting, "Lighting", "lighting"),
    (Tab::Macros, "Macros", "macros"),
    (Tab::Tester, "Key tester", "tester"),
    (Tab::Device, "Device", "device"),
];

/// What the keycode picker writes to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    Key,
    EncoderCcw,
    EncoderCw,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Selection {
    /// index into `Definition::keys`
    pub key: usize,
    pub slot: Slot,
}

#[derive(Clone)]
pub enum Confirm {
    Restore(PathBuf),
    ResetKeymap,
    Bootloader,
    CopyLayer(usize, usize),
    ModeSetup,
    ModeOff,
}

pub struct Keyboard {
    pub session: Option<Session>,
    connecting: Option<Receiver<Result<Session>>>,
    pub connect_error: Option<String>,
    demo: bool,
    last_scan: Instant,
    devices: Vec<PathBuf>,
    pub tab: Tab,
    pub layer: usize,
    pub selected: Option<Selection>,
    pub category: usize,
    pub search: String,
    pub any_input: String,
    pub macro_edit: Vec<String>,
    pub macro_loaded: bool,
    pub selected_macro: usize,
    pub tester: tester::Tester,
    pub confirm: Option<Confirm>,
    pub layer_copy: (usize, usize),
    pub menu_drag: Option<String>,
    pub mode_file: Option<modes::ModeFile>,
    pub fx_filter: String,
    pub notes: Vec<Note>,
}

impl Keyboard {
    pub fn new(demo: bool) -> Self {
        let mut k = Keyboard {
            session: None,
            connecting: None,
            connect_error: None,
            demo,
            last_scan: Instant::now(),
            devices: vec![],
            tab: Tab::Keymap,
            layer: 0,
            selected: None,
            category: 0,
            search: String::new(),
            any_input: String::new(),
            macro_edit: vec![],
            macro_loaded: false,
            selected_macro: 0,
            tester: tester::Tester::new(),
            confirm: None,
            layer_copy: (0, 1),
            menu_drag: None,
            mode_file: None,
            fx_filter: String::new(),
            notes: vec![],
        };
        k.devices = hid::enumerate().into_iter().map(|d| d.path).collect();
        k.connect();
        k
    }

    pub fn tab_names(&self) -> Vec<&'static str> {
        if self.session.is_some() {
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

    /// True once the first connection attempt is over (for screenshots).
    pub fn settled(&self) -> bool {
        self.connecting.is_none()
    }

    pub fn connect(&mut self) {
        if self.connecting.is_some() {
            return;
        }
        let demo = self.demo;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let r = if demo {
                Session::demo(None)
            } else {
                session::pick(None, None).and_then(|(info, def)| Session::open(&info, def))
            };
            let _ = tx.send(r);
        });
        self.connecting = Some(rx);
    }

    pub fn update(&mut self) {
        if let Some(rx) = &self.connecting {
            match rx.try_recv() {
                Ok(Ok(s)) => {
                    self.connecting = None;
                    self.layer = self.layer.min(s.layers.saturating_sub(1));
                    self.selected = None;
                    self.macro_loaded = false;
                    self.tester.reset(s.def.rows as usize * s.def.cols as usize);
                    self.mode_file = modes::load(&s);
                    self.connect_error = None;
                    self.session = Some(s);
                }
                Ok(Err(e)) => {
                    self.connecting = None;
                    self.connect_error = Some(format!("{e:#}"));
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => self.connecting = None,
            }
        }
        // reconnect on hotplug, but not in a loop against a keyboard that does not answer
        if self.session.is_none()
            && self.connecting.is_none()
            && !self.demo
            && self.last_scan.elapsed() > Duration::from_secs(3)
        {
            self.last_scan = Instant::now();
            let now: Vec<PathBuf> = hid::enumerate().into_iter().map(|d| d.path).collect();
            if now != self.devices && !now.is_empty() {
                self.connect();
            }
            self.devices = now;
        }
    }

    /// Runs a keyboard operation and turns its error into a toast.
    pub fn with_session<T>(&mut self, f: impl FnOnce(&mut Session) -> Result<T>) -> Option<T> {
        let s = self.session.as_mut()?;
        match f(s) {
            Ok(v) => Some(v),
            Err(e) => {
                let mut msg = format!("{e:#}");
                if msg.contains("timeout") {
                    msg.push_str(" (keyboard asleep? press a key on it and try again)");
                }
                if msg.contains("disconnected") {
                    self.session = None;
                    self.connect_error = Some("keyboard disconnected".into());
                }
                self.notes.push((msg, false));
                None
            }
        }
    }

    pub fn ok(&mut self, msg: impl Into<String>) {
        self.notes.push((msg.into(), true));
    }

    pub fn current_code(&self, sel: Selection) -> Option<u16> {
        let s = self.session.as_ref()?;
        let k = &s.def.keys[sel.key];
        Some(match sel.slot {
            Slot::Key => s.keymap.get(self.layer)?[s.key_index(k.row, k.col)],
            Slot::EncoderCcw => s.encoders.get(self.layer)?.get(k.encoder? as usize)?.0,
            Slot::EncoderCw => s.encoders.get(self.layer)?.get(k.encoder? as usize)?.1,
        })
    }

    /// Writes a keycode to the selection and, like VIA, moves on: to the next
    /// key, or for the knob to its next slot (press, left, right).
    pub fn assign(&mut self, code: u16) {
        let Some(sel) = self.selected else {
            self.notes.push(("Click a key first".into(), false));
            return;
        };
        let layer = self.layer;
        let done = self.with_session(|s| {
            let k = s.def.keys[sel.key].clone();
            match sel.slot {
                Slot::Key => s.set_key(layer, k.row, k.col, code),
                Slot::EncoderCcw => {
                    s.set_encoder(layer, k.encoder.unwrap_or(0) as usize, false, code)
                }
                Slot::EncoderCw => {
                    s.set_encoder(layer, k.encoder.unwrap_or(0) as usize, true, code)
                }
            }
        });
        if done.is_none() {
            return;
        }
        self.ok("Written to the keyboard");
        let is_knob = self
            .session
            .as_ref()
            .is_some_and(|s| s.def.keys[sel.key].encoder.is_some());
        self.selected = if is_knob {
            let slot = match sel.slot {
                Slot::Key => Slot::EncoderCcw,
                Slot::EncoderCcw => Slot::EncoderCw,
                Slot::EncoderCw => Slot::Key,
            };
            Some(Selection { slot, ..sel })
        } else {
            self.next_key(sel.key).map(|key| Selection {
                key,
                slot: Slot::Key,
            })
        };
    }

    /// The key after `idx` in reading order, skipping the knob.
    fn next_key(&self, idx: usize) -> Option<usize> {
        let s = self.session.as_ref()?;
        let mut order: Vec<(usize, f32, f32)> = s
            .def
            .visible_keys(&s.layout_options)
            .filter(|(_, k)| k.encoder.is_none())
            .map(|(i, k)| (i, k.y, k.x))
            .collect();
        order.sort_by(|a, b| {
            (a.1.round() as i32, a.2)
                .partial_cmp(&(b.1.round() as i32, b.2))
                .unwrap()
        });
        let pos = order.iter().position(|(i, _, _)| *i == idx)?;
        order.get(pos + 1).or(order.first()).map(|(i, _, _)| *i)
    }

    /// Windows or Mac when octo hub controls the switch.
    pub fn mode(&self) -> Option<Mode> {
        let s = self.session.as_ref()?;
        match modes::state_with(s, self.mode_file.as_ref()) {
            State::Active(m) | State::Edited(m) => Some(m),
            _ => None,
        }
    }

    pub fn summary(&self) -> (bool, String) {
        match (&self.session, self.mode()) {
            (None, _) => (false, "Not connected".into()),
            (Some(_), Some(m)) => (true, format!("{} layout", m.name())),
            (Some(s), None) => (true, format!("{} layers", s.layers)),
        }
    }

    pub fn headline(&self) -> Headline {
        let connected = self.session.is_some();
        let (head, unit) = match (&self.session, self.mode()) {
            (Some(_), Some(m)) => (m.name().to_string(), "layout"),
            (Some(s), None) => (s.layers.to_string(), "layers"),
            (None, _) if self.connecting.is_some() => ("Connecting…".into(), ""),
            (None, _) => ("Not connected".into(), ""),
        };
        Headline {
            connected,
            dot: None,
            head,
            unit: unit.into(),
            battery: None,
            conn: match &self.session {
                Some(s) if s.kb.info().path == std::path::Path::new("demo") => {
                    "Connected · demo keyboard".into()
                }
                Some(_) => "Connected · USB".into(),
                None => "Not found. Plug it in over USB".into(),
            },
        }
    }

    /// Top bar: the "Layout Windows" chip, which opens the Device tab.
    pub fn status(&mut self, ui: &mut egui::Ui) {
        if self.session.is_none() {
            return;
        }
        let mode = self.mode().map(|m| m.name()).unwrap_or("Fn+S / Fn+A");
        let g1 = ui
            .painter()
            .layout_no_wrap("Layout".into(), widgets::prop(13.0), theme::TEXT_DIM);
        let g2 = ui
            .painter()
            .layout_no_wrap(mode.into(), widgets::prop(13.0), theme::TEXT);
        let w = g1.size().x + 8.0 + g2.size().x + 24.0;
        ui.add_space(6.0);
        let (r, resp) = ui.allocate_exact_size(vec2(w, 32.0), Sense::click());
        let p = ui.painter();
        p.rect(
            r,
            7,
            if resp.hovered() {
                theme::HOVER
            } else {
                egui::Color32::TRANSPARENT
            },
            Stroke::new(1.0, theme::LINE),
            egui::StrokeKind::Inside,
        );
        let w1 = g1.size().x;
        p.galley(
            egui::pos2(r.min.x + 12.0, r.center().y - g1.size().y / 2.0),
            g1,
            theme::TEXT_DIM,
        );
        p.galley(
            egui::pos2(r.min.x + 20.0 + w1, r.center().y - g2.size().y / 2.0),
            g2,
            theme::TEXT,
        );
        if resp.on_hover_text("Windows / Mac layout").clicked() {
            self.tab = Tab::Device;
        }
    }

    pub fn page(&mut self, ui: &mut egui::Ui) {
        if self.session.is_none() {
            self.missing_page(ui);
            return;
        }
        match self.tab {
            Tab::Keymap => keymap::page(self, ui),
            Tab::Lighting => lighting::page(self, ui),
            Tab::Macros => macros_page::page(self, ui),
            Tab::Tester => tester::page(self, ui),
            Tab::Device => device::page(self, ui),
        }
    }

    fn missing_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(60.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(460.0);
            ui.spacing_mut().item_spacing.y = 14.0;
            let r = icons::allocate(ui, 86.0);
            icons::device(ui.painter(), icons::square(r.center() - vec2(0.0, 7.0), 72.0), Device::Keyboard, theme::TEXT_FAINT, 2.0, false);
            if self.connecting.is_some() {
                ui.label(RichText::new("Reading the keyboard…").size(18.0).color(theme::TEXT));
                return;
            }
            ui.label(RichText::new("GMK70 not found").size(18.0).color(theme::TEXT));
            let e = self.connect_error.clone().unwrap_or_default();
            let text = if e.contains("timeout") {
                "The keyboard stops answering while idle. Press a key on it and try again."
            } else if e.contains("Permission denied") || e.contains("could not open") {
                "Your user can't open the keyboard's hidraw node. Add the udev rule from hidra's README and replug it."
            } else if e.contains("no definition") {
                "The keyboard's VIA definition isn't installed. Load its JSON file."
            } else {
                "Plug in its 2.4 GHz receiver or the USB cable. VIA doesn't work over Bluetooth."
            };
            ui.add(egui::Label::new(RichText::new(text).size(13.5).color(theme::TEXT_DIM)).wrap());
            if !e.is_empty() {
                ui.add(egui::Label::new(RichText::new(e.lines().next().unwrap_or("")).font(widgets::mono(11.5)).color(theme::TEXT_FAINT)).wrap());
            }
            let defs = e.contains("no definition");
            let w = if defs { 100.0 + 8.0 + 170.0 } else { 100.0 };
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - w) / 2.0).max(0.0));
                if widgets::primary(ui, "Try again").clicked() {
                    self.connect();
                }
                if defs && widgets::secondary(ui, "Load VIA definition…").clicked() {
                    self.install_definition();
                }
            });
        });
    }

    fn install_definition(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("VIA definition", &["json"])
            .set_title("Choose the keyboard's VIA definition")
            .pick_file()
        else {
            return;
        };
        match hidra::definition::install(&path) {
            Ok(d) => {
                self.ok(format!("Installed the definition for {}", d.name));
                self.connect();
            }
            Err(e) => self.connect_error = Some(format!("{e:#}")),
        }
    }

    pub fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(c) = self.confirm.clone() {
            device::confirm(self, ctx, c);
        }
    }
}
