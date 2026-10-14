//! Macros: list on the left, the text editor and a preview of the steps.

use super::Keyboard;
use crate::{theme, widgets};
use eframe::egui;
use egui::{Align2, RichText, Sense, Stroke, vec2};
use hidra::keycode;
use hidra::macros::{self, Action};

const TOKENS: [(&str, &str); 5] = [
    ("{KC_A}", "Tap a key"),
    ("{KC_LCTL,KC_C}", "Keys together"),
    ("{+KC_LSFT}", "Hold"),
    ("{-KC_LSFT}", "Release"),
    ("{250}", "Wait"),
];

pub fn page(k: &mut Keyboard, ui: &mut egui::Ui) {
    let Some(s) = &k.session else { return };
    if s.macro_count == 0 {
        widgets::card(ui, |ui| {
            ui.label(widgets::dim("This keyboard has no macro slots.", 13.0))
        });
        return;
    }
    if !k.macro_loaded || k.macro_edit.len() != s.macros.len() {
        k.macro_edit = s.macros.iter().map(|m| macros::to_expression(m)).collect();
        k.macro_loaded = true;
    }
    let saved: Vec<String> = s.macros.iter().map(|m| macros::to_expression(m)).collect();
    k.selected_macro = k.selected_macro.min(k.macro_edit.len() - 1);
    let list_w = (ui.available_width() * 0.25).clamp(220.0, 280.0);
    widgets::two(ui, list_w, -1.0, 16.0, 640.0, |ui, i| {
        if i == 0 {
            list(k, ui);
        } else {
            ui.spacing_mut().item_spacing.y = 16.0;
            editor(k, ui);
            preview(k, ui, &saved);
        }
    });
}

fn list(k: &mut Keyboard, ui: &mut egui::Ui) {
    widgets::card_with(ui, egui::Margin::same(10), theme::LINE_SOFT, |ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        for i in 0..k.macro_edit.len() {
            let on = k.selected_macro == i;
            let (r, resp) =
                ui.allocate_exact_size(vec2(ui.available_width(), 31.0), Sense::click());
            let p = ui.painter();
            if on {
                p.rect_filled(r, 7, theme::HI);
            } else if resp.hovered() {
                p.rect_filled(r, 7, theme::HOVER);
            }
            p.text(
                r.left_center() + vec2(10.0, 0.0),
                Align2::LEFT_CENTER,
                format!("M{i}"),
                widgets::mono(12.5),
                if on { theme::ACCENT } else { theme::TEXT_DIM },
            );
            let text = &k.macro_edit[i];
            let (shown, color) = if text.is_empty() {
                ("empty".to_string(), theme::TEXT_EMPTY)
            } else {
                (text.replace('\n', " "), theme::TEXT_SOFT)
            };
            let clip = egui::Rect::from_min_max(r.min + vec2(52.0, 0.0), r.max - vec2(8.0, 0.0));
            p.with_clip_rect(clip).text(
                clip.left_center(),
                Align2::LEFT_CENTER,
                shown,
                widgets::mono(12.5),
                color,
            );
            if resp.clicked() {
                k.selected_macro = i;
            }
        }
    });
}

fn editor(k: &mut Keyboard, ui: &mut egui::Ui) {
    let i = k.selected_macro;
    let size = k.session.as_ref().map(|s| s.macro_buffer_size).unwrap_or(0);
    let parsed: Vec<Vec<Action>> = k
        .macro_edit
        .iter()
        .map(|e| macros::from_expression(e).unwrap_or_default())
        .collect();
    let used = macros::to_bytes(&parsed).len();
    widgets::card(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("M{i}"))
                    .font(widgets::mono(17.0))
                    .strong()
                    .color(theme::ACCENT),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                ui.label(
                    RichText::new(format!("{used} / {size} bytes"))
                        .font(widgets::mono(12.0))
                        .color(if used > size {
                            theme::ERR
                        } else {
                            theme::TEXT_DIM
                        }),
                );
                let (bar, _) = ui.allocate_exact_size(vec2(120.0, 4.0), Sense::hover());
                ui.painter().rect_filled(bar, 2, theme::LINE);
                let mut f = bar;
                f.set_width(bar.width() * (used as f32 / size.max(1) as f32).min(1.0));
                ui.painter().rect_filled(
                    f,
                    2,
                    if used > size {
                        theme::ERR
                    } else {
                        theme::ACCENT
                    },
                );
            });
        });
        ui.add_space(14.0);
        let err = macros::from_expression(&k.macro_edit[i])
            .err()
            .map(|e| e.to_string());
        egui::Frame::new()
            .fill(theme::BG)
            .stroke(Stroke::new(
                1.0,
                if err.is_some() {
                    theme::ERR
                } else {
                    theme::LINE
                },
            ))
            .corner_radius(9)
            .inner_margin(egui::Margin::symmetric(12, 11))
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut k.macro_edit[i])
                        .desired_rows(3)
                        .desired_width(f32::INFINITY)
                        .frame(egui::Frame::NONE)
                        .font(widgets::mono(14.0))
                        .hint_text(
                            RichText::new("Type text, or add keys below").color(theme::TEXT_FAINT),
                        ),
                );
            });
        if let Some(e) = err {
            ui.add_space(6.0);
            ui.label(RichText::new(e).size(12.5).color(theme::ERR));
        }
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            ui.label(widgets::dim("Add", 12.5));
            ui.add_space(4.0);
            for (t, d) in TOKENS {
                let g1 = ui
                    .painter()
                    .layout_no_wrap(d.into(), widgets::prop(12.5), theme::TEXT);
                let g2 =
                    ui.painter()
                        .layout_no_wrap(t.into(), widgets::mono(11.5), theme::TEXT_DIM);
                let (r, resp) = ui.allocate_exact_size(
                    vec2(g1.size().x + 7.0 + g2.size().x + 20.0, 28.0),
                    Sense::click(),
                );
                let p = ui.painter();
                p.rect(
                    r,
                    6,
                    theme::HI,
                    Stroke::new(
                        1.0,
                        if resp.hovered() {
                            theme::ACCENT_DIM
                        } else {
                            theme::LINE
                        },
                    ),
                    egui::StrokeKind::Inside,
                );
                let w1 = g1.size().x;
                p.galley(
                    egui::pos2(r.min.x + 10.0, r.center().y - g1.size().y / 2.0),
                    g1,
                    theme::TEXT,
                );
                p.galley(
                    egui::pos2(r.min.x + 17.0 + w1, r.center().y - g2.size().y / 2.0 + 1.0),
                    g2,
                    theme::TEXT_DIM,
                );
                if resp.on_hover_text(t).clicked() {
                    k.macro_edit[i].push_str(t);
                }
            }
        });
    });
}

fn kc(k: u8) -> String {
    keycode::to_string(k as u16)
}

fn preview(k: &mut Keyboard, ui: &mut egui::Ui, saved: &[String]) {
    let i = k.selected_macro;
    let dirty = k.macro_edit != saved;
    let all: Vec<anyhow::Result<Vec<Action>>> = k
        .macro_edit
        .iter()
        .map(|e| macros::from_expression(e))
        .collect();
    let all_ok = all.iter().all(|p| p.is_ok());
    let size = k.session.as_ref().map(|s| s.macro_buffer_size).unwrap_or(0);
    let fits = macros::to_bytes(
        &all.iter()
            .map(|p| p.as_ref().ok().cloned().unwrap_or_default())
            .collect::<Vec<_>>(),
    )
    .len()
        <= size;
    widgets::card(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.label(
            RichText::new(format!("When you press M{i}, the keyboard will"))
                .size(13.5)
                .color(theme::TEXT_SOFT),
        );
        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            match &all[i] {
                Ok(seq) if !seq.is_empty() => {
                    for a in seq {
                        let (kind, value) = match a {
                            Action::Text(t) => ("type", t.clone()),
                            Action::Tap(c) => ("tap", kc(*c)),
                            Action::Chord(cs) => (
                                "press together",
                                cs.iter().map(|c| kc(*c)).collect::<Vec<_>>().join(" + "),
                            ),
                            Action::Down(c) => ("hold", kc(*c)),
                            Action::Up(c) => ("release", kc(*c)),
                            Action::Delay(ms) => ("wait", format!("{ms} ms")),
                        };
                        step(ui, kind, &value);
                    }
                }
                Ok(_) => {
                    ui.label(widgets::dim("do nothing yet.", 13.0));
                }
                Err(_) => {
                    ui.label(widgets::dim("Fix the macro text to see the steps.", 13.0));
                }
            }
        });
        ui.add_space(16.0);
        widgets::divider(ui);
        ui.add_space(14.0);
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), 32.0),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                let can_save = dirty && all_ok && fits;
                if widgets::button("Save to keyboard", widgets::Kind::Primary)
                    .muted(!can_save)
                    .show(ui)
                    .clicked()
                    && can_save
                {
                    let new: Vec<Vec<Action>> =
                        all.iter().map(|p| p.as_ref().unwrap().clone()).collect();
                    if k.with_session(|s| s.write_macros(new)).is_some() {
                        k.macro_loaded = false;
                        k.ok("Macros saved to the keyboard");
                    }
                }
                if widgets::button("Revert", widgets::Kind::Secondary)
                    .muted(!dirty)
                    .show(ui)
                    .clicked()
                    && dirty
                {
                    k.macro_loaded = false;
                }
                if !fits {
                    ui.label(
                        RichText::new("Too long for the keyboard's macro memory")
                            .size(12.5)
                            .color(theme::ERR),
                    );
                }
            },
        );
    });
}

fn step(ui: &mut egui::Ui, kind: &str, value: &str) {
    egui::Frame::new()
        .fill(theme::STEP_FILL)
        .corner_radius(7)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 7.0;
                ui.label(RichText::new(kind).size(11.5).color(theme::STEP_KIND));
                ui.label(
                    RichText::new(value)
                        .font(widgets::mono(13.0))
                        .color(theme::STEP_TEXT),
                );
            });
        });
}
