//! The Attack Shark X6, through remora's device thread and remembered profile.
//!
//! The mouse cannot report what it holds, so the window edits the profile and
//! every change goes out once the hand settles; what the mouse acknowledges is
//! saved to remora's `~/.config/remora/mouse.json`, shared with the remora app.

mod art;
mod buttons;
mod dpi;
mod lighting;
mod power;

use crate::app::Note;
use crate::home::Headline;
use crate::{theme, widgets};
use eframe::egui;
use egui::{Align2, Rect, Sense, pos2, vec2};
use remora::link::{Event, Link};
use remora::profile::{self, Profile, Saved};
use remora::proto::{self, Status};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dpi,
    Lighting,
    Buttons,
    Power,
}

const TABS: [(Tab, &str, &str); 4] = [
    (Tab::Dpi, "DPI & sensor", "dpi"),
    (Tab::Lighting, "Lighting", "lighting"),
    (Tab::Buttons, "Buttons", "buttons"),
    (Tab::Power, "Power", "power"),
];

const REPORTS: [u8; 4] = [
    proto::SENSOR_REPORT,
    proto::LIGHT_REPORT,
    proto::POLLING_REPORT,
    proto::BUTTONS_REPORT,
];

/// Sliders send once the hand stops for this long.
const SETTLE: Duration = Duration::from_millis(140);

/// The design's swatches, and the LED colour each one sends: the LED is far
/// more saturated than a screen, so the mouse gets pure channels.
pub const SWATCHES: [([u8; 3], [u8; 3]); 10] = [
    ([0xff, 0x3b, 0x30], [0xff, 0x00, 0x00]),
    ([0xff, 0x7a, 0x1a], [0xff, 0x40, 0x00]),
    ([0xff, 0xb0, 0x1a], [0xff, 0xa0, 0x00]),
    ([0xff, 0xe8, 0x1a], [0xff, 0xff, 0x00]),
    ([0x34, 0xe0, 0x5a], [0x00, 0xff, 0x00]),
    ([0x2f, 0xe0, 0xe0], [0x00, 0xff, 0xff]),
    ([0x2f, 0x7b, 0xff], [0x00, 0x60, 0xff]),
    ([0x4a, 0x3a, 0xff], [0x00, 0x00, 0xff]),
    ([0xe0, 0x40, 0xfb], [0xff, 0x00, 0xff]),
    ([0xff, 0xff, 0xff], [0xff, 0xff, 0xff]),
];

/// How an LED colour looks on screen.
pub fn screen_color(led: [u8; 3]) -> egui::Color32 {
    theme::rgb(
        SWATCHES
            .iter()
            .find(|s| s.1 == led)
            .map(|s| s.0)
            .unwrap_or(led),
    )
}

pub struct Mouse {
    link: Link,
    /// what the window shows and edits
    pub profile: Profile,
    /// what the mouse acknowledged; saved to disk
    confirmed: Saved,
    /// None in demo mode, which never touches the real profile
    path: Option<PathBuf>,
    /// report id -> the bytes last handed to the device thread and the profile they came from
    sent: HashMap<u8, (Vec<u8>, Profile)>,
    failed: HashMap<u8, String>,
    last_edit: Instant,
    pub connected: Option<(String, String)>,
    pub connect_error: Option<String>,
    pub battery: Option<u8>,
    pub power: Option<proto::Power>,
    pub last_beat: Option<Instant>,
    pub tab: Tab,
    /// button being edited on the buttons page
    pub edit_button: usize,
    /// action category shown for that button
    pub category: Option<usize>,
    pub recording: bool,
    pub confirm_reset: bool,
    pub notes: Vec<Note>,
    /// acks arrive one per report; one toast per burst of writes
    toast_due: bool,
}

impl Mouse {
    pub fn new(demo: bool, wake: impl Fn() + Send + 'static) -> Self {
        let (confirmed, path, load_error) = if demo {
            (
                Saved {
                    written: true,
                    ..Saved::default()
                },
                None,
                None,
            )
        } else {
            let path = profile::default_path();
            match profile::load(&path) {
                Ok(s) => (s, Some(path), None),
                Err(e) => (Saved::default(), Some(path), Some(format!("{e:#}"))),
            }
        };
        let profile = confirmed.profile.clone();
        // the mouse is assumed to hold the saved profile, so nothing is resent at startup
        let sent = REPORTS
            .iter()
            .map(|&id| (id, (encode(&profile, id), profile.clone())))
            .collect();
        let mut m = Mouse {
            link: Link::spawn(demo, wake),
            profile,
            confirmed,
            path,
            sent,
            failed: HashMap::new(),
            last_edit: Instant::now(),
            connected: None,
            connect_error: None,
            battery: None,
            power: None,
            last_beat: None,
            tab: Tab::Dpi,
            edit_button: 0,
            category: None,
            recording: false,
            confirm_reset: false,
            notes: vec![],
            toast_due: false,
        };
        if let Some(e) = load_error {
            m.notes
                .push((format!("{e}; starting from factory settings"), false));
        }
        m
    }

    pub fn tab_names(&self) -> Vec<&'static str> {
        TABS.iter().map(|t| t.1).collect()
    }
    pub fn tab_index(&self) -> usize {
        TABS.iter().position(|t| t.0 == self.tab).unwrap_or(0)
    }
    pub fn set_tab_index(&mut self, i: usize) {
        self.tab = TABS[i].0;
        self.recording = false;
    }
    pub fn set_tab(&mut self, key: &str) {
        if let Some(t) = TABS.iter().find(|t| t.2 == key) {
            self.tab = t.0;
        }
    }

    /// Call after changing `profile`; the change goes out once input settles.
    pub fn touch(&mut self) {
        self.last_edit = Instant::now();
        self.toast_due = true;
    }

    pub fn awake(&self) -> bool {
        self.last_beat
            .is_some_and(|t| t.elapsed() < Duration::from_secs(5))
    }

    pub fn active_stage(&self) -> usize {
        (self
            .profile
            .sensor
            .active
            .clamp(1, self.profile.sensor.stage_count)
            - 1) as usize
    }

    /// Sends every report, so the mouse matches the window whatever it held before.
    pub fn write_all(&mut self) {
        for id in REPORTS {
            self.send(id);
        }
        self.confirmed.written = true;
        self.notes
            .push(("Wrote every setting to the mouse".into(), true));
    }

    pub fn factory_reset(&mut self) {
        self.profile = Profile::default();
        for id in REPORTS {
            self.send(id);
        }
        self.confirmed.written = true;
        self.notes
            .push(("Mouse reset to factory settings".into(), true));
    }

    fn send(&mut self, id: u8) {
        let bytes = encode(&self.profile, id);
        self.link.send(bytes.clone());
        let colors = |p: &Profile| p.sensor.stages.map(|s| s.color);
        let recolored = id == proto::SENSOR_REPORT
            && self
                .sent
                .get(&id)
                .is_some_and(|(_, p)| colors(p) != colors(&self.profile));
        self.sent.insert(id, (bytes, self.profile.clone()));
        self.failed.remove(&id);
        // the LED keeps the old stage colour until the next stage change or lighting write
        if recolored {
            self.send(proto::LIGHT_REPORT);
        }
    }

    /// Sends whatever changed since the last send, once the input settles.
    fn sync(&mut self) {
        if self.connected.is_none() || self.last_edit.elapsed() < SETTLE {
            return;
        }
        for id in REPORTS {
            let now = encode(&self.profile, id);
            if self.sent.get(&id).map(|s| &s.0) != Some(&now) {
                self.send(id);
            }
        }
    }

    fn save(&mut self) {
        let Some(path) = &self.path else { return };
        if let Err(e) = profile::save(path, &self.confirmed) {
            self.notes.push((format!("{e:#}"), false));
        }
    }

    pub fn update(&mut self) {
        for e in self.link.poll() {
            match e {
                Event::Connected { path, phys } => {
                    self.connected = Some((path, phys));
                    self.connect_error = None;
                    // anything that failed while the receiver was away goes again
                    for id in std::mem::take(&mut self.failed).into_keys() {
                        self.sent.remove(&id);
                    }
                }
                Event::Disconnected(why) => {
                    self.connected = None;
                    self.battery = None;
                    self.last_beat = None;
                    self.connect_error = Some(why);
                }
                Event::Status(Status::Heartbeat { battery, power }) => {
                    self.battery = Some(battery);
                    self.power = Some(power);
                    self.last_beat = Some(Instant::now());
                }
                Event::Status(Status::Stage(n)) => {
                    let n = n.clamp(1, self.profile.sensor.stage_count);
                    // the mouse already switched; record it without writing it back
                    self.profile.sensor.active = n;
                    self.confirmed.profile.sensor.active = n;
                    if let Some((bytes, p)) = self.sent.get_mut(&proto::SENSOR_REPORT) {
                        p.sensor.active = n;
                        *bytes = p.sensor.encode().to_vec();
                    }
                    self.save();
                }
                Event::Status(_) => {}
                Event::Applied(id) => {
                    if let Some((_, p)) = self.sent.get(&id) {
                        copy_section(&mut self.confirmed.profile, p, id);
                        self.save();
                    }
                    if self.toast_due && !REPORTS.iter().any(|&r| self.pending(r)) {
                        self.toast_due = false;
                        self.notes.push(("Saved to the mouse".into(), true));
                    }
                }
                Event::Failed(id, why) => {
                    self.failed.insert(id, why.clone());
                    self.toast_due = false;
                    self.notes.push((format!("Not saved: {why}"), false));
                }
            }
        }
        self.sync();
    }

    fn pending(&self, id: u8) -> bool {
        encode(&self.profile, id) != encode(&self.confirmed.profile, id)
    }

    pub fn summary(&self) -> (bool, String) {
        match (&self.connected, self.battery) {
            (None, _) => (false, "Not connected".into()),
            (Some(_), Some(b)) => (
                true,
                format!("{} DPI · {b}% battery", self.profile.sensor.active_dpi()),
            ),
            (Some(_), None) => (true, format!("{} DPI", self.profile.sensor.active_dpi())),
        }
    }

    pub fn headline(&self) -> Headline {
        let stage = self.profile.sensor.stages[self.active_stage()];
        let connected = self.connected.is_some();
        Headline {
            connected,
            dot: Some(screen_color(stage.color)),
            head: if connected {
                stage.dpi.to_string()
            } else {
                "Not connected".into()
            },
            unit: if connected {
                "DPI".into()
            } else {
                String::new()
            },
            battery: self.battery.map(|b| (b, self.awake())),
            conn: match &self.connected {
                Some((path, _)) if path == "demo" => "Connected · demo mouse".into(),
                Some(_) => "Connected · 2.4 GHz receiver".into(),
                None => "Not found. Plug in the 2.4 GHz receiver".into(),
            },
        }
    }

    /// Top bar: stage dot and DPI (opens DPI), battery (opens Power).
    pub fn status(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.spacing_mut().item_spacing.x = 14.0;
        if let Some(b) = self.battery {
            let color = if self.awake() {
                theme::OK
            } else {
                theme::TEXT_DIM
            };
            let text = format!("{b}%");
            let g = ui
                .painter()
                .layout_no_wrap(text, widgets::prop(13.0), color);
            let (r, resp) =
                ui.allocate_exact_size(vec2(20.0 + 7.0 + g.size().x, 24.0), Sense::click());
            widgets::paint_battery(
                ui.painter(),
                Rect::from_min_size(pos2(r.min.x, r.center().y - 5.5), vec2(20.0, 11.0)),
                b,
                color,
            );
            ui.painter().galley(
                pos2(r.min.x + 27.0, r.center().y - g.size().y / 2.0),
                g,
                color,
            );
            if resp
                .on_hover_text(if self.awake() {
                    "Battery"
                } else {
                    "Battery (mouse asleep)"
                })
                .clicked()
            {
                self.tab = Tab::Power;
            }
            widgets::vline(ui, 18.0, theme::LINE);
        } else if self.connected.is_none() {
            ui.label(widgets::dim("Not connected", 13.0));
            return;
        }
        let stage = self.profile.sensor.stages[self.active_stage()];
        let dpi = stage.dpi.to_string();
        let dg = ui
            .painter()
            .layout_no_wrap(dpi, widgets::mono(13.0), theme::TEXT);
        let (r, resp) = ui.allocate_exact_size(
            vec2(8.0 + 7.0 + dg.size().x + 7.0 + 24.0, 24.0),
            Sense::click(),
        );
        let p = ui.painter();
        p.circle_filled(
            pos2(r.min.x + 4.0, r.center().y),
            4.0,
            screen_color(stage.color),
        );
        let dw = dg.size().x;
        p.galley(
            pos2(r.min.x + 15.0, r.center().y - dg.size().y / 2.0),
            dg,
            theme::TEXT,
        );
        p.text(
            pos2(r.min.x + 22.0 + dw, r.center().y),
            Align2::LEFT_CENTER,
            "DPI",
            widgets::prop(13.0),
            theme::TEXT_DIM,
        );
        if resp.on_hover_text("DPI stage in use").clicked() {
            self.tab = Tab::Dpi;
        }
    }

    pub fn page(&mut self, ui: &mut egui::Ui) {
        match self.tab {
            Tab::Dpi => dpi::page(self, ui),
            Tab::Lighting => lighting::page(self, ui),
            Tab::Buttons => buttons::page(self, ui),
            Tab::Power => power::page(self, ui),
        }
    }
}

pub fn encode(p: &Profile, id: u8) -> Vec<u8> {
    match id {
        proto::SENSOR_REPORT => p.sensor.encode().to_vec(),
        proto::LIGHT_REPORT => p.light.encode().to_vec(),
        proto::POLLING_REPORT => p.polling.encode().to_vec(),
        _ => proto::encode_buttons(&p.buttons).to_vec(),
    }
}

fn copy_section(to: &mut Profile, from: &Profile, id: u8) {
    match id {
        proto::SENSOR_REPORT => to.sensor = from.sensor.clone(),
        proto::LIGHT_REPORT => to.light = from.light.clone(),
        proto::POLLING_REPORT => to.polling = from.polling,
        _ => to.buttons = from.buttons,
    }
}

/// A row of the design's colour swatches; returns the LED colour clicked.
pub fn swatches(ui: &mut egui::Ui, current: [u8; 3], size: f32, gap: f32) -> Option<[u8; 3]> {
    let mut pick = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(gap, gap);
        for (screen, led) in SWATCHES {
            let (r, resp) = ui.allocate_exact_size(egui::Vec2::splat(size), Sense::click());
            let p = ui.painter();
            p.circle_filled(r.center(), size / 2.0, theme::rgb(screen));
            if current == led {
                p.circle_stroke(
                    r.center(),
                    size / 2.0 + 1.0,
                    egui::Stroke::new(2.0, theme::PANEL),
                );
                p.circle_stroke(
                    r.center(),
                    size / 2.0 + 3.0,
                    egui::Stroke::new(2.0, theme::TEXT),
                );
            } else {
                p.circle_stroke(
                    r.center(),
                    size / 2.0 - 0.5,
                    egui::Stroke::new(1.0, egui::Color32::from_black_alpha(50)),
                );
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                pick = Some(led);
            }
        }
    });
    pick
}
