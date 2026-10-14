//! The window: top bar with the device switcher and tabs, the home screen,
//! the saved toast, and the dispatch to each device's pages.

use crate::icons::{self, Device};
use crate::keyboard::Keyboard;
use crate::mouse::Mouse;
use crate::pedal::Pedal;
use crate::{Flags, home, shot, theme, widgets};
use anyhow::Result;
use eframe::egui;
use egui::{Align2, Color32, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub fn run(f: Flags) -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("octo hub")
            .with_app_id("octo-hub")
            .with_inner_size([1280.0, 860.0])
            .with_min_inner_size([760.0, 560.0])
            .with_icon(std::sync::Arc::new(window_icon())),
        ..Default::default()
    };
    eframe::run_native(
        "octo-hub",
        options,
        Box::new(move |cc| {
            theme::apply(&cc.egui_ctx);
            let ctx = cc.egui_ctx.clone();
            Ok(Box::new(App::new(f, move || ctx.request_repaint())))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}

/// The logo as window icon, as in `packaging/octo-hub.svg`: an accent disc
/// with two dark eyes, on a 22-unit grid with 4×4 supersampling.
fn window_icon() -> egui::IconData {
    const N: usize = 128;
    let (cx, cy, r) = (11.0f32, 11.0f32, 9.6f32);
    let eyes = [(8.3f32, 10.1f32), (13.7, 10.1)];
    let mut rgba = Vec::with_capacity(N * N * 4);
    for y in 0..N {
        for x in 0..N {
            let (mut cover, mut eye) = (0.0f32, 0.0f32);
            for s in 0..16 {
                let px = (x as f32 + (s % 4) as f32 / 4.0 + 0.125) * 22.0 / N as f32;
                let py = (y as f32 + (s / 4) as f32 / 4.0 + 0.125) * 22.0 / N as f32;
                if (px - cx).hypot(py - cy) <= r {
                    cover += 1.0 / 16.0;
                    if eyes.iter().any(|(ex, ey)| (px - ex).hypot(py - ey) <= 1.8) {
                        eye += 1.0 / 16.0;
                    }
                }
            }
            let t = if cover > 0.0 { eye / cover } else { 0.0 };
            let c = widgets::lerp_color(theme::ACCENT, theme::ACCENT_TEXT, t);
            rgba.extend([c.r(), c.g(), c.b(), (cover * 255.0).round() as u8]);
        }
    }
    egui::IconData {
        rgba,
        width: N as u32,
        height: N as u32,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Home,
    Mouse,
    Keyboard,
    Pedal,
}

/// A message a page wants shown: ok ones float as the saved toast, failures stay longer.
pub type Note = (String, bool);

pub struct App {
    pub screen: Screen,
    pub mouse: Mouse,
    pub kb: Keyboard,
    pub pedal: Pedal,
    toast: Option<(String, bool, Instant)>,
    menu_open: bool,
    shot: Option<PathBuf>,
    frame: u64,
    started: Instant,
}

impl App {
    fn new(f: Flags, wake: impl Fn() + Send + Sync + Clone + 'static) -> Self {
        let screen = match f.screen.as_deref() {
            Some("mouse") => Screen::Mouse,
            Some("keyboard") => Screen::Keyboard,
            Some("pedal") => Screen::Pedal,
            _ => Screen::Home,
        };
        let mut app = App {
            screen: Screen::Home,
            mouse: Mouse::new(f.demo, wake.clone()),
            kb: Keyboard::new(f.demo),
            pedal: Pedal::new(f.demo, wake),
            toast: None,
            menu_open: false,
            shot: f.shot,
            frame: 0,
            started: Instant::now(),
        };
        if let Some(t) = f.tab.as_deref() {
            app.mouse.set_tab(t);
            app.kb.set_tab(t);
            app.pedal.set_tab(t);
        }
        app.go(screen);
        app
    }

    pub fn go(&mut self, s: Screen) {
        self.menu_open = false;
        if s == Screen::Pedal && self.screen != Screen::Pedal {
            self.pedal.open();
        }
        if s != Screen::Pedal && self.screen == Screen::Pedal {
            self.pedal.close();
        }
        self.screen = s;
    }

    fn take_notes(&mut self) {
        let notes: Vec<Note> = self
            .mouse
            .notes
            .drain(..)
            .chain(self.kb.notes.drain(..))
            .chain(self.pedal.notes.drain(..))
            .collect();
        if let Some((text, ok)) = notes.into_iter().last() {
            self.toast = Some((text, ok, Instant::now()));
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            self.switcher(ui);
            self.tabs(ui);
            ui.with_layout(
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| match self.screen {
                    Screen::Mouse => self.mouse.status(ui),
                    Screen::Keyboard => self.kb.status(ui),
                    Screen::Pedal => self.pedal.status(ui),
                    Screen::Home => {}
                },
            );
        });
    }

    fn current(&self) -> Device {
        match self.screen {
            Screen::Keyboard => Device::Keyboard,
            Screen::Pedal => Device::Pedal,
            _ => Device::Mouse,
        }
    }

    /// Icon, device name and ▼; opens the list of all devices.
    fn switcher(&mut self, ui: &mut egui::Ui) {
        let name = home::name(self.current());
        let g = ui
            .painter()
            .layout_no_wrap(name.to_string(), widgets::prop(14.0), theme::TEXT);
        let (r, resp) = ui.allocate_exact_size(
            vec2(6.0 + 24.0 + 8.0 + g.size().x + 8.0 + 10.0 + 10.0, 36.0),
            Sense::click(),
        );
        let p = ui.painter();
        if resp.hovered() || self.menu_open {
            p.rect_filled(r, 8, theme::HOVER);
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let icon = Rect::from_min_size(pos2(r.min.x + 6.0, r.center().y - 12.0), Vec2::splat(24.0));
        icons::device(p, icon, self.current(), theme::TEXT_SOFT, 3.0, false);
        p.galley(
            pos2(icon.max.x + 8.0, r.center().y - g.size().y / 2.0),
            g.clone(),
            theme::TEXT,
        );
        p.text(
            pos2(r.max.x - 14.0, r.center().y),
            Align2::CENTER_CENTER,
            "▼",
            widgets::prop(10.0),
            theme::TEXT_DIM,
        );
        if resp.clicked() {
            self.menu_open = !self.menu_open;
        }
        if !self.menu_open {
            return;
        }
        let area = egui::Area::new(egui::Id::new("device-menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos2(r.min.x, r.max.y + 6.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(theme::CARD_ALT)
                    .stroke(Stroke::new(1.0, theme::LINE))
                    .corner_radius(12)
                    .inner_margin(6)
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 12],
                        blur: 32,
                        spread: 0,
                        color: Color32::from_black_alpha(115),
                    })
                    .show(ui, |ui| {
                        ui.set_width(288.0);
                        ui.spacing_mut().item_spacing.y = 2.0;
                        let mut pick = None;
                        // back to the home screen, the only way there now the logo is gone
                        let (rr, rs) = ui.allocate_exact_size(vec2(288.0, 38.0), Sense::click());
                        if rs.hovered() {
                            ui.painter().rect_filled(rr, 8, theme::HI);
                        }
                        ui.painter().text(
                            pos2(rr.min.x + 10.0, rr.center().y),
                            Align2::LEFT_CENTER,
                            "‹  All devices",
                            widgets::prop(14.0),
                            theme::TEXT_SOFT,
                        );
                        if rs.clicked() {
                            pick = Some(None);
                        }
                        let (sep, _) = ui.allocate_exact_size(vec2(288.0, 5.0), Sense::hover());
                        ui.painter().hline(
                            (sep.min.x + 8.0)..=(sep.max.x - 8.0),
                            sep.center().y,
                            Stroke::new(1.0, theme::LINE),
                        );
                        for d in [Device::Mouse, Device::Keyboard, Device::Pedal] {
                            let (connected, sub) = self.summary(d);
                            let (rr, rs) =
                                ui.allocate_exact_size(vec2(288.0, 52.0), Sense::click());
                            let p = ui.painter();
                            let here = d == self.current();
                            if rs.hovered() || here {
                                p.rect_filled(rr, 8, theme::HI);
                            }
                            let alpha = if connected { 1.0 } else { 0.6 };
                            p.text(
                                pos2(rr.min.x + 10.0, rr.min.y + 17.0),
                                Align2::LEFT_CENTER,
                                home::name(d),
                                widgets::prop(14.0),
                                theme::TEXT.gamma_multiply(alpha),
                            );
                            p.text(
                                pos2(rr.min.x + 10.0, rr.min.y + 36.0),
                                Align2::LEFT_CENTER,
                                sub,
                                widgets::prop(12.0),
                                theme::TEXT_DIM.gamma_multiply(alpha),
                            );
                            p.circle_filled(
                                pos2(rr.max.x - 16.0, rr.center().y),
                                3.5,
                                if connected {
                                    theme::OK
                                } else {
                                    theme::TEXT_EMPTY
                                },
                            );
                            if rs.clicked() {
                                pick = Some(Some(d));
                            }
                        }
                        pick
                    })
                    .inner
            });
        if let Some(d) = area.inner {
            self.go(match d {
                None => Screen::Home,
                Some(Device::Mouse) => Screen::Mouse,
                Some(Device::Keyboard) => Screen::Keyboard,
                Some(Device::Pedal) => Screen::Pedal,
            });
        } else if !resp.clicked() && ui.input(|i| i.pointer.any_click()) && !area.response.hovered()
        {
            self.menu_open = false;
        }
    }

    /// Whether a device answers, and its one-line status for the switcher.
    pub fn summary(&self, d: Device) -> (bool, String) {
        match d {
            Device::Mouse => self.mouse.summary(),
            Device::Keyboard => self.kb.summary(),
            Device::Pedal => self.pedal.summary(),
        }
    }

    fn tabs(&mut self, ui: &mut egui::Ui) {
        let tabs: Vec<&'static str> = match self.screen {
            Screen::Mouse => self.mouse.tab_names(),
            Screen::Keyboard => self.kb.tab_names(),
            Screen::Pedal => self.pedal.tab_names(),
            Screen::Home => vec![],
        };
        if tabs.is_empty() {
            return;
        }
        let current = match self.screen {
            Screen::Mouse => self.mouse.tab_index(),
            Screen::Keyboard => self.kb.tab_index(),
            _ => self.pedal.tab_index(),
        };
        let p = ui.painter().clone();
        let galleys: Vec<_> = tabs
            .iter()
            .map(|t| p.layout_no_wrap(t.to_string(), widgets::prop(13.5), theme::TEXT))
            .collect();
        let w: f32 = galleys.iter().map(|g| g.size().x + 26.0).sum::<f32>()
            + 2.0 * (tabs.len() as f32 - 1.0)
            + 8.0;
        ui.add_space(6.0);
        let (r, _) = ui.allocate_exact_size(vec2(w, 38.0), Sense::hover());
        p.rect(
            r,
            9,
            theme::BG,
            Stroke::new(1.0, theme::EDGE),
            StrokeKind::Inside,
        );
        let mut x = r.min.x + 4.0;
        let mut pick = None;
        for (i, g) in galleys.into_iter().enumerate() {
            let tr = Rect::from_min_size(pos2(x, r.min.y + 4.0), vec2(g.size().x + 26.0, 30.0));
            let resp = ui.interact(tr, ui.id().with(("tab", i)), Sense::click());
            let on = i == current;
            if on {
                p.rect_filled(tr, 7, theme::TAB_ON);
            }
            let color = if on || resp.hovered() {
                theme::TEXT
            } else {
                theme::TEXT_DIM
            };
            p.text(
                tr.center(),
                Align2::CENTER_CENTER,
                tabs[i],
                widgets::prop(13.5),
                color,
            );
            if resp.clicked() {
                pick = Some(i);
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            x += tr.width() + 2.0;
        }
        if let Some(i) = pick {
            match self.screen {
                Screen::Mouse => self.mouse.set_tab_index(i),
                Screen::Keyboard => self.kb.set_tab_index(i),
                _ => self.pedal.set_tab_index(i),
            }
        }
    }

    fn toast(&mut self, ctx: &egui::Context) {
        let Some((text, ok, at)) = &self.toast else {
            return;
        };
        let life = if *ok { 1.8 } else { 5.0 };
        if at.elapsed().as_secs_f32() > life {
            self.toast = None;
            return;
        }
        let (text, ok) = (text.clone(), *ok);
        ctx.request_repaint_after(Duration::from_millis(200));
        egui::Area::new(egui::Id::new("toast"))
            .order(egui::Order::Tooltip)
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -22.0))
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(theme::CARD_ALT)
                    .stroke(Stroke::new(1.0, theme::LINE))
                    .corner_radius(9)
                    .inner_margin(egui::Margin::symmetric(16, 9))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 8],
                        blur: 24,
                        spread: 0,
                        color: Color32::from_black_alpha(100),
                    })
                    .show(ui, |ui| {
                        let mark = if ok { "✓" } else { "✕" };
                        ui.label(
                            egui::RichText::new(format!("{mark} {text}"))
                                .size(13.0)
                                .color(if ok { theme::OK } else { theme::ERR }),
                        );
                    });
            });
    }

    /// With `--shot`, waits for the devices to settle, saves one frame and quits.
    fn screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = self.shot.clone() else {
            return;
        };
        self.frame += 1;
        let ready = self.started.elapsed() > Duration::from_millis(1500) && self.kb.settled();
        if ready && self.frame.is_multiple_of(10) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        let image = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = image {
            if let Err(e) = shot::save_png(&img, &path) {
                eprintln!("screenshot failed: {e}");
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint();
    }
}

/// Centres the content at `max` width with the design's padding.
pub fn content(ui: &mut egui::Ui, max: f32, pad: Vec2, add: impl FnOnce(&mut egui::Ui)) {
    let avail = ui.available_width();
    let w = (avail - pad.x * 2.0).min(max);
    ui.add_space(pad.y);
    ui.horizontal_top(|ui| {
        ui.add_space(((avail - w) / 2.0).max(0.0));
        ui.vertical(|ui| {
            ui.set_width(w);
            add(ui);
        });
    });
    ui.add_space(pad.y + 26.0);
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.mouse.update();
        self.kb.update();
        self.pedal.update(self.screen == Screen::Pedal);
        self.take_notes();
        self.screenshot(&ctx);
        // the settle timer, sleep badges and the pedal poll need frames without input
        ctx.request_repaint_after(Duration::from_millis(250));
        // the home screen has no top bar; device screens get the switcher and tabs
        if self.screen != Screen::Home {
            egui::Panel::top("top")
                .exact_size(56.0)
                .resizable(false)
                .show_separator_line(false)
                .frame(
                    egui::Frame::new()
                        .fill(theme::TOPBAR)
                        .inner_margin(egui::Margin::symmetric(14, 0)),
                )
                .show(ui, |ui| {
                    let r = ui.max_rect();
                    ui.painter()
                        .with_clip_rect(r.expand2(vec2(14.0, 1.0)))
                        .hline(
                            (r.min.x - 14.0)..=(r.max.x + 14.0),
                            r.max.y - 0.5,
                            Stroke::new(1.0, theme::EDGE),
                        );
                    self.top_bar(ui);
                });
        }
        egui::CentralPanel::default_margins()
            .frame(egui::Frame::new().fill(theme::BG))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| match self.screen {
                        Screen::Home => home::page(self, ui),
                        Screen::Mouse => {
                            content(ui, 1240.0, vec2(24.0, 22.0), |ui| self.mouse.page(ui))
                        }
                        Screen::Keyboard => {
                            content(ui, 1240.0, vec2(24.0, 22.0), |ui| self.kb.page(ui))
                        }
                        Screen::Pedal => {
                            content(ui, 1240.0, vec2(24.0, 22.0), |ui| self.pedal.page(ui))
                        }
                    });
            });
        self.kb.dialogs(&ctx);
        self.pedal.dialogs(&ctx);
        self.toast(&ctx);
    }
}
