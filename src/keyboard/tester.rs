//! Key tester: reads the raw switch matrix, so a dead switch shows even when
//! the keymap hides it.

use super::{Keyboard, board};
use crate::{theme, widgets};
use eframe::egui;
use egui::{RichText, Sense, vec2};
use hidra::keycode;
use std::collections::VecDeque;
use std::time::Duration;

pub struct Tester {
    pub pressed: Vec<bool>,
    pub seen: Vec<bool>,
    /// newest first: (down, row, col, key name)
    pub log: VecDeque<(bool, usize, usize, String)>,
}

impl Tester {
    pub fn new() -> Self {
        Tester {
            pressed: vec![],
            seen: vec![],
            log: VecDeque::new(),
        }
    }

    pub fn reset(&mut self, n: usize) {
        self.pressed = vec![false; n];
        self.seen = vec![false; n];
        self.log.clear();
    }
}

fn poll(k: &mut Keyboard) {
    let Some(s) = &mut k.session else { return };
    let (rows, cols) = (s.def.rows as usize, s.def.cols as usize);
    match s.kb.matrix_state(rows, cols) {
        Ok(now) => {
            let t = &mut k.tester;
            for i in 0..now.len().min(t.pressed.len()) {
                if now[i] != t.pressed[i] {
                    let code = s.keymap.first().map(|l| l[i]).unwrap_or(0);
                    t.log.push_front((
                        now[i],
                        i / cols,
                        i % cols,
                        keycode::label(code, &[], 2.0).replace('\n', " "),
                    ));
                    t.log.truncate(9);
                    if now[i] {
                        t.seen[i] = true;
                    }
                }
            }
            t.pressed = now;
        }
        Err(e) => k.notes.push((format!("Matrix read failed: {e:#}"), false)),
    }
}

pub fn page(k: &mut Keyboard, ui: &mut egui::Ui) {
    poll(k);
    ui.ctx().request_repaint_after(Duration::from_millis(16));
    let Some(s) = &k.session else { return };
    let def = s.def.clone();
    let custom = def.custom_keycodes.clone();
    let opts = s.layout_options.clone();
    let t = &k.tester;
    let visual = |idx: usize| {
        let key = &def.keys[idx];
        let i = s.key_index(key.row, key.col);
        let code = s.keymap.first().map(|l| l[i]).unwrap_or(0);
        let fill = if t.pressed.get(i).copied().unwrap_or(false) {
            theme::ACCENT
        } else if t.seen.get(i).copied().unwrap_or(false) {
            theme::OK
        } else {
            theme::CAP_ALPHA
        };
        board::KeyVisual {
            label: keycode::label(code, &custom, key.w),
            colors: Some((fill, theme::CAP_LEGEND)),
            selected: false,
            glow: None,
            tooltip: Some(format!("[{},{}]", key.row, key.col)),
        }
    };
    board::card(ui, |ui| board::show(ui, &def, &opts, &visual));
    ui.add_space(16.0);
    let positions: Vec<usize> = def
        .visible_keys(&opts)
        .map(|(_, key)| s.key_index(key.row, key.col))
        .collect();
    let total = positions.len();
    let seen = positions
        .iter()
        .filter(|i| k.tester.seen.get(**i).copied().unwrap_or(false))
        .count();
    let mut reset = false;
    widgets::columns(ui, 2, 320.0, 16.0, |ui, i| {
        if i == 0 {
            widgets::card(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.horizontal(|ui| {
                    ui.label(RichText::new(seen.to_string()).size(34.0));
                    ui.label(
                        RichText::new(format!("of {total} keys work"))
                            .size(13.5)
                            .color(theme::TEXT_DIM),
                    );
                });
                ui.add_space(12.0);
                let (bar, _) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 6.0), Sense::hover());
                ui.painter().rect_filled(bar, 3, theme::LINE);
                let mut f = bar;
                f.set_width(
                    bar.width()
                        * if total > 0 {
                            seen as f32 / total as f32
                        } else {
                            0.0
                        },
                );
                ui.painter().rect_filled(f, 3, theme::OK);
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for (c, text) in [
                        (theme::ACCENT, "held"),
                        (theme::OK, "worked"),
                        (theme::CAP_ALPHA, "not yet"),
                    ] {
                        let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                        ui.painter().rect_filled(r, 3, c);
                        ui.label(widgets::dim(text, 12.5));
                        ui.add_space(10.0);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::button("Start over", widgets::Kind::Secondary)
                            .compact()
                            .show(ui)
                            .clicked()
                        {
                            reset = true;
                        }
                    });
                });
            });
        } else {
            widgets::card(ui, |ui| {
                ui.set_min_height(96.0);
                ui.spacing_mut().item_spacing.y = 5.0;
                if k.tester.log.is_empty() {
                    widgets::para(
                        ui,
                        "Press keys on the keyboard. Each switch is read straight from the matrix, so a dead one shows even if the keymap hides it.",
                        13.5,
                        theme::TEXT_DIM,
                    );
                    return;
                }
                for (down, r, c, name) in &k.tester.log {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 14.0;
                        ui.add_sized(
                            vec2(52.0, 16.0),
                            egui::Label::new(
                                RichText::new(format!("[{r},{c}]"))
                                    .font(widgets::mono(12.5))
                                    .color(theme::TEXT_DIM),
                            ),
                        );
                        ui.add_sized(
                            vec2(110.0, 16.0),
                            egui::Label::new(RichText::new(name).font(widgets::mono(12.5))),
                        );
                        ui.label(
                            RichText::new(if *down { "down" } else { "up" })
                                .font(widgets::mono(12.5))
                                .color(if *down {
                                    theme::ACCENT
                                } else {
                                    theme::TEXT_DIM
                                }),
                        );
                    });
                }
            });
        }
    });
    if reset {
        let n = k.tester.pressed.len();
        k.tester.reset(n);
    }
}
