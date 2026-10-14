//! Keymap: layer selector, the board, the keycode picker and the selected key's detail.

use super::{Keyboard, Selection, Slot, board};
use crate::{theme, widgets};
use eframe::egui;
use egui::{Align2, RichText, Sense, vec2};
use hidra::keycode::{self, KeycodeEntry};
use hidra::modes;

/// Names for layers when the Windows/Mac layout is known: Base, Fn, Mac, Mac Fn.
pub fn layer_names(k: &Keyboard) -> Vec<String> {
    let Some(s) = &k.session else { return vec![] };
    let mac = k
        .mode_file
        .as_ref()
        .map(|f| f.mac_layer)
        .or_else(|| modes::plan(s).ok().map(|p| p.0));
    (0..s.layers)
        .map(|l| {
            let name = match mac {
                Some(_) if l == 0 => "Base",
                Some(m) if l == m => "Mac",
                Some(m) if l < m => "Fn",
                Some(m) if l == m + 1 => "Mac Fn",
                _ => "",
            };
            if name.is_empty() {
                l.to_string()
            } else {
                format!("{l} {name}")
            }
        })
        .collect()
}

fn layer_hint(name: &str) -> &'static str {
    match name.split_once(' ').map(|x| x.1) {
        Some("Base") => "What each key does normally.",
        Some("Fn") => "Active while MO(1) or Space is held. ▽ keys fall through to the base layer.",
        Some("Mac") => "The Mac layout.",
        Some("Mac Fn") => "Fn on the Mac layout.",
        _ => "",
    }
}

pub fn page(k: &mut Keyboard, ui: &mut egui::Ui) {
    let names = layer_names(k);
    ui.horizontal(|ui| {
        if let Some(l) = widgets::segmented(&names, Some(k.layer))
            .track(theme::TOPBAR)
            .mono_prefix()
            .show(ui)
        {
            k.layer = l;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(widgets::hint(layer_hint(
                names.get(k.layer).map(String::as_str).unwrap_or(""),
            )));
        });
    });
    ui.add_space(12.0);
    keys(k, ui);
    shortcuts(k, ui);
    ui.add_space(16.0);
    widgets::two(ui, -1.0, 300.0, 16.0, 760.0, |ui, i| {
        if i == 0 { picker(k, ui) } else { detail(k, ui) }
    });
}

fn keys(k: &mut Keyboard, ui: &mut egui::Ui) {
    let Some(s) = &k.session else { return };
    let custom = s.def.custom_keycodes.clone();
    let layer = k.layer;
    let selected = k.selected;
    let def = s.def.clone();
    let opts = s.layout_options.clone();
    let visual = |idx: usize| {
        let key = &def.keys[idx];
        let code = s
            .keymap
            .get(layer)
            .map(|l| l[s.key_index(key.row, key.col)])
            .unwrap_or(0);
        let ghost = layer > 0 && code == keycode::KC_TRNS && key.encoder.is_none();
        board::KeyVisual {
            label: if ghost {
                "▽".into()
            } else {
                keycode::label(code, &custom, key.w)
            },
            colors: ghost.then_some((theme::CAP_GHOST, theme::TEXT_FAINT)),
            selected: selected.is_some_and(|sel| sel.key == idx),
            glow: None,
            tooltip: Some(format!(
                "[{},{}]  {}",
                key.row,
                key.col,
                keycode::describe(code, &custom)
            )),
        }
    };
    let r = board::card(ui, |ui| board::show(ui, &def, &opts, &visual));
    if let Some(idx) = r.clicked {
        k.selected = if k.selected.is_some_and(|s| s.key == idx) {
            None
        } else {
            Some(Selection {
                key: idx,
                slot: Slot::Key,
            })
        };
    }
}

/// Esc stops, Del clears, T sets ▽, as in hidra.
fn shortcuts(k: &mut Keyboard, ui: &mut egui::Ui) {
    if k.confirm.is_some() || ui.ctx().memory(|m| m.focused().is_some()) || k.selected.is_none() {
        return;
    }
    let (del, trns, esc) = ui.input(|i| {
        (
            i.key_pressed(egui::Key::Delete),
            i.key_pressed(egui::Key::T) && i.modifiers.is_none(),
            i.key_pressed(egui::Key::Escape),
        )
    });
    if del {
        k.assign(keycode::KC_NO);
    }
    if trns {
        k.assign(keycode::KC_TRNS);
    }
    if esc {
        k.selected = None;
    }
}

fn picker(k: &mut Keyboard, ui: &mut egui::Ui) {
    let Some(s) = &k.session else { return };
    let categories = keycode::db().categories(
        s.macro_count,
        &s.def.custom_keycodes,
        s.def.has_lighting_keycodes(),
    );
    let has_sel = k.selected.is_some();
    widgets::card_with(
        ui,
        egui::Margin::symmetric(18, 16),
        theme::LINE_SOFT,
        |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let searching = !k.search.trim().is_empty();
            ui.horizontal_wrapped(|ui| {
                let labels: Vec<String> = categories.iter().map(|c| c.label.to_string()).collect();
                let cur = (!searching).then_some(k.category);
                if let Some(i) = widgets::segmented(&labels, cur).pad(12.0, 5.0).show(ui) {
                    k.category = i;
                    k.search.clear();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    widgets::text_field(ui, &mut k.search, "Search keycodes", 200.0, false);
                });
            });
            ui.add_space(14.0);
            let list: Vec<KeycodeEntry> = if searching {
                let q = k.search.to_lowercase();
                let mut seen = std::collections::HashSet::new();
                categories
                    .iter()
                    .flat_map(|c| c.keys.iter())
                    .filter(|e| {
                        e.code.to_lowercase().contains(&q)
                            || e.name.to_lowercase().contains(&q)
                            || e.title
                                .as_deref()
                                .is_some_and(|t| t.to_lowercase().contains(&q))
                    })
                    .filter(|e| seen.insert(e.code.clone()))
                    .cloned()
                    .collect()
            } else {
                categories
                    .get(k.category)
                    .map(|c| c.keys.clone())
                    .unwrap_or_default()
            };
            let mut chosen = None;
            egui::ScrollArea::vertical()
                .max_height(250.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if !has_sel {
                        ui.multiply_opacity(0.55);
                    }
                    if list.is_empty() {
                        ui.label(widgets::dim(
                            "Nothing matches. Type the keycode into the field below.",
                            13.0,
                        ));
                    }
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(5.0, 5.0);
                        for e in &list {
                            let Some(code) = keycode::parse(&e.code) else {
                                continue;
                            };
                            let label = if e.name.is_empty() {
                                "✕".to_string()
                            } else {
                                e.name.replace('\n', " ")
                            };
                            let g = ui.painter().layout_no_wrap(
                                label.clone(),
                                widgets::prop(12.5),
                                theme::CAP_LEGEND,
                            );
                            let w = (g.size().x + 16.0).max(44.0);
                            let (r, resp) = ui.allocate_exact_size(vec2(w, 40.0), Sense::click());
                            let alpha = e.code.starts_with("KC_") && label.chars().count() <= 1;
                            let fill = if alpha {
                                theme::CAP_ALPHA
                            } else {
                                theme::CAP_MOD
                            };
                            board::paint_cap(
                                ui.painter(),
                                r,
                                6.0,
                                if resp.hovered() {
                                    theme::shade(fill, 1.08)
                                } else {
                                    fill
                                },
                                theme::CAP_LEGEND,
                                "",
                                0.75,
                                None,
                            );
                            ui.painter().text(
                                r.center() - vec2(0.0, 1.5),
                                Align2::CENTER_CENTER,
                                &label,
                                widgets::prop(12.5),
                                theme::CAP_LEGEND,
                            );
                            let hover = match &e.title {
                                Some(t) => format!("{t}\n{}", e.code),
                                None => e.code.clone(),
                            };
                            if resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if resp.on_hover_text(hover).clicked() {
                                chosen = Some(code);
                            }
                        }
                    });
                });
            ui.add_space(14.0);
            widgets::divider(ui);
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                let w = (ui.available_width() - 90.0).min(360.0);
                let edit = widgets::text_field(
                    ui,
                    &mut k.any_input,
                    "Any QMK keycode, e.g. LT(1,KC_SPC)",
                    w,
                    true,
                );
                let parsed = keycode::parse(&k.any_input);
                let submit = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if widgets::secondary(ui, "Apply").clicked() || submit {
                    match parsed {
                        Some(v) => chosen = Some(v),
                        None if !k.any_input.trim().is_empty() => k
                            .notes
                            .push((format!("{} is not a keycode", k.any_input.trim()), false)),
                        None => {}
                    }
                }
            });
            if let Some(code) = chosen {
                k.assign(code);
            }
        },
    );
}

fn detail(k: &mut Keyboard, ui: &mut egui::Ui) {
    widgets::card_with(
        ui,
        egui::Margin::symmetric(18, 16),
        theme::LINE_SOFT,
        |ui| {
            ui.set_min_height(168.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            let Some(sel) = k.selected else {
                ui.add_space(56.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("Click a key or the knob\nto change what it does.")
                            .size(13.5)
                            .color(theme::TEXT_DIM),
                    );
                });
                return;
            };
            let Some(s) = &k.session else { return };
            let key = s.def.keys[sel.key].clone();
            let custom = s.def.custom_keycodes.clone();
            let code = k.current_code(sel).unwrap_or(0);
            let is_knob = key.encoder.is_some();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 14.0;
                let (r, _) = ui.allocate_exact_size(vec2(60.0, 60.0), Sense::hover());
                let radius = if is_knob { 30.0 } else { 9.0 };
                board::paint_cap(
                    ui.painter(),
                    r,
                    radius,
                    theme::ACCENT,
                    theme::ACCENT_TEXT,
                    &keycode::label(code, &custom, 1.0).replace('\n', " "),
                    0.95,
                    None,
                );
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    let name = keycode::to_string(code);
                    ui.add(
                        egui::Label::new(
                            RichText::new(&name)
                                .font(widgets::mono(15.0))
                                .strong()
                                .color(theme::ACCENT),
                        )
                        .wrap(),
                    );
                    let desc = keycode::describe(code, &custom);
                    let desc = desc.trim_end_matches(&format!(" ({name})")).to_string();
                    let desc = if desc == name {
                        if is_knob {
                            "Rotary knob".into()
                        } else {
                            "Custom keycode".into()
                        }
                    } else {
                        desc
                    };
                    ui.add(egui::Label::new(widgets::dim(desc, 13.0)).wrap());
                });
            });
            if let Some(e) = key.encoder {
                ui.add_space(14.0);
                let (ccw, cw) = s
                    .encoders
                    .get(k.layer)
                    .and_then(|l| l.get(e as usize))
                    .copied()
                    .unwrap_or((0, 0));
                let press = s
                    .keymap
                    .get(k.layer)
                    .map(|l| l[s.key_index(key.row, key.col)])
                    .unwrap_or(0);
                ui.spacing_mut().item_spacing.y = 4.0;
                for (slot, name, v) in [
                    (Slot::Key, "Press", press),
                    (Slot::EncoderCcw, "Turn left ↺", ccw),
                    (Slot::EncoderCw, "Turn right ↻", cw),
                ] {
                    let on = sel.slot == slot;
                    let (r, resp) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
                    let p = ui.painter();
                    p.rect(
                        r,
                        7,
                        if on {
                            theme::ACCENT_TINT
                        } else {
                            egui::Color32::TRANSPARENT
                        },
                        egui::Stroke::new(1.0, if on { theme::ACCENT } else { theme::LINE }),
                        egui::StrokeKind::Inside,
                    );
                    p.text(
                        r.left_center() + vec2(11.0, 0.0),
                        Align2::LEFT_CENTER,
                        name,
                        widgets::prop(13.5),
                        theme::TEXT,
                    );
                    p.text(
                        r.right_center() - vec2(11.0, 0.0),
                        Align2::RIGHT_CENTER,
                        keycode::to_string(v),
                        widgets::mono(12.0),
                        theme::TEXT_DIM,
                    );
                    if resp.clicked() {
                        k.selected = Some(Selection { slot, ..sel });
                    }
                }
            }
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let w = (ui.available_width() - 6.0) / 2.0;
                if widgets::button("▽ Transparent", widgets::Kind::Secondary)
                    .width(w)
                    .show(ui)
                    .clicked()
                {
                    k.assign(keycode::KC_TRNS);
                }
                if widgets::button("Clear", widgets::Kind::Secondary)
                    .width(w)
                    .show(ui)
                    .clicked()
                {
                    k.assign(keycode::KC_NO);
                }
            });
            ui.add_space(12.0);
            ui.label(widgets::dim(
                "Picking a keycode moves on to the next key. Esc stops.",
                12.0,
            ));
        },
    );
}
