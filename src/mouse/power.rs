//! Battery, rewriting the mouse, factory reset and the sleep timers.

use super::Mouse;
use crate::{theme, widgets};
use eframe::egui;
use egui::{Rect, RichText, Sense, Stroke, vec2};
use remora::proto;

pub fn page(m: &mut Mouse, ui: &mut egui::Ui) {
    let left_w = (ui.available_width() * 0.3).clamp(260.0, 320.0);
    widgets::two(ui, left_w, -1.0, 16.0, 660.0, |ui, i| {
        if i == 0 {
            ui.spacing_mut().item_spacing.y = 16.0;
            battery(m, ui);
            on_mouse(m, ui);
        } else {
            sleep(m, ui);
        }
    });
}

fn battery(m: &mut Mouse, ui: &mut egui::Ui) {
    widgets::card(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.label(widgets::dim("Battery", 13.0));
        ui.add_space(4.0);
        let awake = m.awake();
        match m.battery {
            Some(b) => {
                ui.label(RichText::new(format!("{b}%")).size(40.0).color(theme::TEXT));
                ui.add_space(10.0);
                let (r, _) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 6.0), Sense::hover());
                ui.painter().rect_filled(r, 3, theme::LINE);
                let f = Rect::from_min_size(
                    r.min,
                    vec2(r.width() * b.min(100) as f32 / 100.0, r.height()),
                );
                ui.painter()
                    .rect_filled(f, 3, if awake { theme::OK } else { theme::TEXT_DIM });
                ui.add_space(10.0);
                let charging = m.power.is_some_and(|p| p.charging());
                let text = match (awake, charging) {
                    (true, true) => "Charging. The receiver reports in steps of 10%.",
                    (true, false) => "Awake. The receiver reports in steps of 10%.",
                    (false, _) => "Asleep. Move the mouse to wake it.",
                };
                ui.add(egui::Label::new(widgets::hint(text)).wrap());
            }
            None => {
                ui.label(RichText::new("—").size(40.0).color(theme::TEXT_DIM));
                ui.add_space(10.0);
                let text = if m.connected.is_some() {
                    "Waiting for the mouse. Move it if it is asleep."
                } else {
                    "Plug in the 2.4 GHz receiver."
                };
                ui.add(egui::Label::new(widgets::hint(text)).wrap());
            }
        }
    });
}

fn on_mouse(m: &mut Mouse, ui: &mut egui::Ui) {
    widgets::card(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.label(RichText::new("Settings on the mouse").size(14.0));
        ui.add_space(6.0);
        widgets::para(
            ui,
            "The X6 can't report what it holds, so octo hub keeps what it last wrote. If another app changed the mouse, write everything again.",
            12.5,
            theme::TEXT_DIM,
        );
        ui.add_space(14.0);
        if !m.confirm_reset {
            ui.horizontal_wrapped(|ui| {
                if widgets::button("Write everything", widgets::Kind::Primary)
                    .enabled(m.connected.is_some())
                    .show(ui)
                    .clicked()
                {
                    m.write_all();
                }
                if widgets::button("Factory reset…", widgets::Kind::Danger)
                    .show(ui)
                    .clicked()
                {
                    m.confirm_reset = true;
                }
            });
            return;
        }
        egui::Frame::new()
            .fill(theme::DANGER_FILL)
            .stroke(Stroke::new(1.0, theme::DANGER_LINE))
            .corner_radius(9)
            .inner_margin(12)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                widgets::para(
                    ui,
                    "Put every setting back to how the mouse shipped?",
                    13.0,
                    theme::TEXT,
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if widgets::button("Reset", widgets::Kind::Destroy)
                        .compact()
                        .enabled(m.connected.is_some())
                        .show(ui)
                        .clicked()
                    {
                        m.confirm_reset = false;
                        m.factory_reset();
                    }
                    if widgets::button("Cancel", widgets::Kind::Secondary)
                        .compact()
                        .show(ui)
                        .clicked()
                    {
                        m.confirm_reset = false;
                    }
                });
            });
    });
}

fn minutes(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{v:.0} min")
    } else {
        format!("{v:.1} min")
    }
}

fn sleep(m: &mut Mouse, ui: &mut egui::Ui) {
    widgets::card_with(ui, egui::Margin::symmetric(20, 6), theme::LINE_SOFT, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let mut changed = false;
        let l = &mut m.profile.light;
        widgets::setting_row(
            ui,
            "Sleep after",
            "Idle time before the lights and sensor rest.",
            false,
            |ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                let mins = l.sleep_half_minutes as f64 / 2.0;
                ui.add_sized(
                    vec2(62.0, 22.0),
                    egui::Label::new(RichText::new(minutes(mins)).font(widgets::mono(13.0))),
                );
                let mut v = mins;
                if widgets::slider(
                    ui,
                    &mut v,
                    0.5,
                    proto::SLEEP_HALF_MINUTES_MAX as f64 / 2.0,
                    false,
                    220.0,
                )
                .changed
                {
                    let half = (v * 2.0)
                        .round()
                        .clamp(1.0, proto::SLEEP_HALF_MINUTES_MAX as f64)
                        as u8;
                    if half != l.sleep_half_minutes {
                        l.sleep_half_minutes = half;
                        changed = true;
                    }
                }
            },
        );
        widgets::setting_row(
            ui,
            "Deep sleep after",
            "The radio sleeps too. Waking takes a click.",
            true,
            |ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                let mins = l.deep_sleep_minutes;
                ui.add_sized(
                    vec2(62.0, 22.0),
                    egui::Label::new(
                        RichText::new(format!("{mins} min")).font(widgets::mono(13.0)),
                    ),
                );
                let mut v = mins as f64;
                if widgets::slider(ui, &mut v, 1.0, proto::DEEP_SLEEP_MAX as f64, false, 220.0)
                    .changed
                {
                    let n = v.round() as u8;
                    if n != l.deep_sleep_minutes {
                        l.deep_sleep_minutes = n;
                        changed = true;
                    }
                }
            },
        );
        if changed {
            m.touch();
        }
    });
}
