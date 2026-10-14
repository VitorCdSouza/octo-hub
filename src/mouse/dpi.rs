//! DPI stages, and the sensor settings card.

use super::{Mouse, SWATCHES, screen_color, swatches};
use crate::{theme, widgets};
use eframe::egui;
use egui::{Align2, Rect, RichText, Sense, Stroke, StrokeKind, pos2, vec2};
use remora::proto::{self, Polling};

const PRESETS: [u32; 8] = [400, 800, 1200, 1600, 2400, 3200, 6400, 12800];

pub fn page(m: &mut Mouse, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing.y = 16.0;
    widgets::card(ui, |ui| stages(m, ui));
    widgets::card_with(ui, egui::Margin::symmetric(20, 6), theme::LINE_SOFT, |ui| {
        settings(m, ui)
    });
}

fn stages(m: &mut Mouse, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing.y = 0.0;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width((ui.available_width() - 150.0).max(100.0));
            ui.spacing_mut().item_spacing.y = 3.0;
            widgets::heading(ui, "DPI stages");
            ui.add(egui::Label::new(widgets::hint("The DPI button on the mouse steps through these. Click one to use and edit it.")).wrap());
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let count = m.profile.sensor.stage_count;
            if widgets::button("+", widgets::Kind::Secondary).compact().width(30.0).enabled(count < proto::STAGES as u8).show(ui).on_hover_text("Add a stage").clicked() {
                let s = &mut m.profile.sensor;
                let i = s.stage_count as usize;
                s.stages[i].dpi = proto::clamp_dpi(s.stages[i - 1].dpi + 800);
                s.stages[i].color = SWATCHES[i % SWATCHES.len()].1;
                s.stage_count += 1;
                m.touch();
            }
            ui.add_sized(vec2(66.0, 30.0), egui::Label::new(RichText::new(format!("{count} stages")).size(13.0).color(theme::TEXT_SOFT)));
            if widgets::button("−", widgets::Kind::Secondary).compact().width(30.0).enabled(count > 1).show(ui).on_hover_text("Remove the last stage").clicked() {
                let s = &mut m.profile.sensor;
                s.stage_count -= 1;
                s.active = s.active.min(s.stage_count);
                m.touch();
            }
        });
    });
    ui.add_space(14.0);
    tiles(m, ui);
    ui.add_space(18.0);
    widgets::divider(ui);
    ui.add_space(16.0);
    editor(m, ui);
}

fn tiles(m: &mut Mouse, ui: &mut egui::Ui) {
    let count = m.profile.sensor.stage_count as usize;
    let active = m.active_stage();
    widgets::grid(ui, count, 112.0, 10.0, |ui, i| {
        let st = m.profile.sensor.stages[i];
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 72.0), Sense::click());
        let p = ui.painter();
        let on = i == active;
        let border = if on {
            theme::ACCENT
        } else if resp.hovered() {
            theme::HOVER_BORDER
        } else {
            theme::LINE
        };
        p.rect(
            r,
            9,
            if on {
                theme::ACCENT_TINT
            } else {
                theme::CARD_ALT
            },
            Stroke::new(1.5, border),
            StrokeKind::Inside,
        );
        let bar = Rect::from_min_max(
            pos2(r.min.x + 12.0, r.min.y + 10.0),
            pos2(r.max.x - 12.0, r.min.y + 14.0),
        );
        p.rect_filled(bar, 2, screen_color(st.color));
        p.text(
            pos2(r.min.x + 12.0, r.min.y + 31.0),
            Align2::LEFT_CENTER,
            (i + 1).to_string(),
            widgets::prop(12.0),
            theme::TEXT_DIM,
        );
        if on {
            p.text(
                pos2(r.max.x - 12.0, r.min.y + 31.0),
                Align2::RIGHT_CENTER,
                "in use",
                widgets::prop(12.0),
                theme::ACCENT,
            );
        }
        p.text(
            pos2(r.min.x + 12.0, r.min.y + 52.0),
            Align2::LEFT_CENTER,
            st.dpi.to_string(),
            widgets::mono(20.0),
            theme::TEXT,
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if resp.clicked() && !on {
            m.profile.sensor.active = i as u8 + 1;
            m.touch();
        }
    });
}

/// Label column of 90px, controls beside it.
fn labelled(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 16.0;
        let (r, _) = ui.allocate_exact_size(vec2(90.0, 24.0), Sense::hover());
        ui.painter().text(
            r.left_center(),
            Align2::LEFT_CENTER,
            label,
            widgets::prop(13.0),
            theme::TEXT_DIM,
        );
        ui.horizontal_wrapped(add);
    });
}

fn editor(m: &mut Mouse, ui: &mut egui::Ui) {
    let i = m.active_stage();
    let mut changed = false;
    let mut st = m.profile.sensor.stages[i];
    labelled(ui, "DPI", |ui| {
        ui.spacing_mut().item_spacing.x = 14.0;
        let mut v = st.dpi as f64;
        let w = ui.available_width().min(300.0);
        if widgets::slider(
            ui,
            &mut v,
            proto::DPI_MIN as f64,
            proto::DPI_MAX as f64,
            true,
            w,
        )
        .changed
        {
            st.dpi = proto::clamp_dpi(v.round() as u32);
            changed = true;
        }
        // the value box takes typing too
        egui::Frame::new()
            .fill(theme::BG)
            .stroke(Stroke::new(1.0, theme::LINE))
            .corner_radius(7)
            .inner_margin(egui::Margin::symmetric(10, 5))
            .show(ui, |ui| {
                ui.set_min_width(72.0);
                let v = ui.visuals_mut();
                for w in [
                    &mut v.widgets.inactive,
                    &mut v.widgets.hovered,
                    &mut v.widgets.active,
                ] {
                    w.weak_bg_fill = egui::Color32::TRANSPARENT;
                    w.bg_fill = egui::Color32::TRANSPARENT;
                    w.bg_stroke = Stroke::NONE;
                }
                ui.spacing_mut().interact_size.y = 20.0;
                ui.spacing_mut().button_padding = egui::vec2(0.0, 0.0);
                let mut dpi = st.dpi;
                let r = ui.add(
                    egui::DragValue::new(&mut dpi)
                        .range(proto::DPI_MIN..=proto::DPI_MAX)
                        .speed(10.0)
                        .suffix(" DPI")
                        .custom_formatter(|v, _| format!("{v:.0}")),
                );
                if r.changed() {
                    st.dpi = proto::clamp_dpi(dpi);
                    changed = true;
                }
            });
    });
    ui.add_space(14.0);
    labelled(ui, "", |ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
        for d in PRESETS {
            let on = st.dpi == d;
            let g = ui
                .painter()
                .layout_no_wrap(d.to_string(), widgets::mono(12.5), theme::TEXT);
            let (r, resp) = ui.allocate_exact_size(vec2(g.size().x + 18.0, 26.0), Sense::click());
            let fg = if on || resp.hovered() {
                theme::TEXT
            } else {
                theme::TEXT_SOFT
            };
            ui.painter()
                .rect_filled(r, 6, if on { theme::ACCENT_DIM } else { theme::HI });
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                d.to_string(),
                widgets::mono(12.5),
                fg,
            );
            if resp.clicked() {
                st.dpi = d;
                changed = true;
            }
        }
    });
    ui.add_space(14.0);
    labelled(ui, "Colour", |ui| {
        if let Some(c) = swatches(ui, st.color, 22.0, 9.0) {
            st.color = c;
            changed = true;
        }
    });
    if changed {
        m.profile.sensor.stages[i] = st;
        m.touch();
    }
}

fn settings(m: &mut Mouse, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing.y = 0.0;
    let mut changed = false;
    let cur = Polling::ALL.iter().position(|p| *p == m.profile.polling);
    widgets::setting_row(
        ui,
        "Polling rate",
        "How often the mouse reports. Lower saves battery.",
        false,
        |ui| {
            let names: Vec<String> = Polling::ALL
                .iter()
                .map(|p| format!("{} Hz", p.hz()))
                .collect();
            if let Some(i) = widgets::segmented(&names, cur).show(ui) {
                m.profile.polling = Polling::ALL[i];
                changed = true;
            }
        },
    );
    let s = &mut m.profile.sensor;
    widgets::setting_row(
        ui,
        "Lift-off distance",
        "Height at which the sensor stops tracking.",
        false,
        |ui| {
            if let Some(i) = widgets::segmented(
                &widgets::strings(["1 mm", "2 mm"]),
                Some(s.lift_2mm as usize),
            )
            .show(ui)
            {
                s.lift_2mm = i == 1;
                changed = true;
            }
        },
    );
    widgets::setting_row(
        ui,
        "Angle snapping",
        "Straightens near-horizontal and near-vertical movement.",
        false,
        |ui| {
            if widgets::switch(ui, s.angle_snap, false) {
                s.angle_snap = !s.angle_snap;
                changed = true;
            }
        },
    );
    widgets::setting_row(
        ui,
        "Ripple control",
        "Smooths jitter at high DPI, at a little latency.",
        true,
        |ui| {
            if widgets::switch(ui, s.ripple, false) {
                s.ripple = !s.ripple;
                changed = true;
            }
        },
    );
    if changed {
        m.touch();
    }
}
