//! The LED effect list and a preview where only the LED lights up.

use super::{Mouse, Tab, art, screen_color, swatches};
use crate::{theme, widgets};
use eframe::egui;
use egui::{Color32, Sense, pos2};
use remora::proto::LightMode;

pub fn page(m: &mut Mouse, ui: &mut egui::Ui) {
    let list_w = (ui.available_width() * 0.28).clamp(240.0, 300.0);
    widgets::two(ui, list_w, -1.0, 16.0, 640.0, |ui, i| {
        if i == 0 {
            effects(m, ui);
        } else {
            preview(m, ui);
        }
    });
}

fn effects(m: &mut Mouse, ui: &mut egui::Ui) {
    widgets::card_with(ui, egui::Margin::same(10), theme::LINE_SOFT, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for mode in LightMode::ALL {
            let note = match mode {
                LightMode::StageColor | LightMode::StageBreathing => "stage colour",
                LightMode::Steady | LightMode::Breathing => "one colour",
                LightMode::Neon | LightMode::ColorBreathing => "cycles colours",
                LightMode::Off => "",
            };
            if widgets::list_row(
                ui,
                mode.name(),
                note,
                m.profile.light.mode == mode,
                Some(theme::TEXT_DIM),
            )
            .clicked()
                && m.profile.light.mode != mode
            {
                m.profile.light.mode = mode;
                m.touch();
            }
        }
    });
}

/// Seconds per cycle for speed 1–5.
fn period(speed: u8) -> f32 {
    [6.0, 4.5, 3.0, 2.0, 1.2][(speed.clamp(1, 5) - 1) as usize]
}

/// What the LED shows at time `t`, as the design animates it.
fn led_at(m: &Mouse, t: f32) -> Option<Color32> {
    let l = &m.profile.light;
    let stage = screen_color(m.profile.sensor.stages[m.active_stage()].color);
    let one = screen_color(l.color);
    let d = period(l.speed);
    let phase = (t / d).fract();
    // opacity .18 ↔ 1, ease-in-out
    let breath = 0.18 + 0.82 * (0.5 - 0.5 * (std::f32::consts::TAU * phase).cos());
    let hue = |c: Color32| {
        let mut h = egui::ecolor::Hsva::from(c);
        h.h = (h.h + phase).fract();
        Color32::from(h)
    };
    let base = Color32::from_rgb(0xff, 0x3b, 0x30);
    Some(match l.mode {
        LightMode::Off => return None,
        LightMode::StageColor => stage,
        LightMode::StageBreathing => stage.gamma_multiply(breath),
        LightMode::Steady => one,
        LightMode::Breathing => one.gamma_multiply(breath),
        LightMode::Neon => hue(base),
        LightMode::ColorBreathing => hue(base).gamma_multiply(breath),
    })
}

fn preview(m: &mut Mouse, ui: &mut egui::Ui) {
    widgets::card_with(ui, egui::Margin::same(24), theme::LINE_SOFT, |ui| {
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing.y = 22.0;
            let size = art::size(false);
            let (r, _) = ui.allocate_exact_size(size + egui::vec2(0.0, 48.0), Sense::hover());
            let origin = pos2(r.center().x - size.x / 2.0, r.min.y + 24.0);
            let t = ui.input(|i| i.time) as f32;
            art::draw(ui.painter(), origin, false, None, None, led_at(m, t));
            let mode = m.profile.light.mode;
            if !matches!(
                mode,
                LightMode::Off | LightMode::Steady | LightMode::StageColor
            ) {
                ui.ctx().request_repaint();
            }
            if mode.uses_color() {
                ui.vertical_centered(|ui| {
                    ui.spacing_mut().item_spacing.y = 10.0;
                    ui.label(widgets::dim("Colour", 13.0));
                    let w = 10.0 * 22.0 + 9.0 * 9.0 + 8.0;
                    ui.allocate_ui(egui::vec2(w, 30.0), |ui| {
                        if let Some(c) = swatches(ui, m.profile.light.color, 22.0, 9.0) {
                            m.profile.light.color = c;
                            m.touch();
                        }
                    });
                });
            }
            if matches!(mode, LightMode::StageColor | LightMode::StageBreathing) {
                ui.horizontal(|ui| {
                    let text = "Lights up in the colour of the DPI stage in use.";
                    let w = ui
                        .painter()
                        .layout_no_wrap(text.into(), widgets::prop(13.0), theme::TEXT_DIM)
                        .size()
                        .x
                        + ui.painter()
                            .layout_no_wrap(
                                "Change stage colours".into(),
                                widgets::prop(13.0),
                                theme::ACCENT,
                            )
                            .size()
                            .x
                        + 4.0;
                    ui.add_space(((ui.available_width() - w) / 2.0).max(0.0));
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.label(widgets::dim(text, 13.0));
                    if widgets::link(ui, "Change stage colours", 13.0).clicked() {
                        m.tab = Tab::Dpi;
                    }
                });
            }
            if mode.uses_speed() {
                ui.horizontal(|ui| {
                    let labels = widgets::strings(1..=5);
                    let w = 50.0 + 12.0 + 5.0 * 32.0 + 16.0;
                    ui.add_space(((ui.available_width() - w) / 2.0).max(0.0));
                    ui.spacing_mut().item_spacing.x = 12.0;
                    ui.label(widgets::dim("Speed", 13.0));
                    let cur = (m.profile.light.speed.clamp(1, 5) - 1) as usize;
                    if let Some(i) = widgets::segmented(&labels, Some(cur))
                        .pad(12.0, 5.0)
                        .show(ui)
                    {
                        m.profile.light.speed = i as u8 + 1;
                        m.touch();
                    }
                });
            }
        });
    });
}
