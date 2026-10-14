//! The sounding preset: slot stepper, save state, bypass, and the signal chain
//! of six blocks with knobs.

use super::preset::{self, BLOCKS, Block, Control, Head};
use super::worker::Command;
use super::{Pedal, Tab};
use crate::{theme, widgets};
use eframe::egui;
use egui::{Align2, Color32, Rect, RichText, Sense, Shape, Stroke, StrokeKind, vec2};

pub fn page(p: &mut Pedal, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing.y = 16.0;
    header(p, ui);
    let bypassed = p.bypassed();
    if bypassed && let Some(s) = p.sounding() {
        ui.label(widgets::dim(
            format!("Every effect is off and the dry signal passes through. Turn bypass off to hear and edit {}.", preset::name(s)),
            13.0,
        ));
    }
    if p.sounding().is_none() && !bypassed {
        ui.label(widgets::dim(
            "The pedal is playing a preset octo hub can't identify. Pick one to edit it.",
            13.0,
        ));
    }
    chain(p, ui);
}

fn header(p: &mut Pedal, ui: &mut egui::Ui) {
    let sounding = p.sounding();
    let dirty = p.dirty();
    let bypassed = p.bypassed();
    widgets::card_with(
        ui,
        egui::Margin::symmetric(18, 14),
        theme::LINE_SOFT,
        |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let step = |ui: &mut egui::Ui, t: &str| {
                    widgets::button(t, widgets::Kind::Secondary)
                        .width(32.0)
                        .show(ui)
                };
                if step(ui, "‹").on_hover_text("Previous preset").clicked() {
                    let s = sounding
                        .map(|s| (s + preset::COUNT - 1) % preset::COUNT)
                        .unwrap_or(0);
                    p.send(Command::Load(s));
                }
                let name = sounding.map(preset::name).unwrap_or_else(|| "—".into());
                let (r, resp) = ui.allocate_exact_size(vec2(72.0, 36.0), Sense::click());
                ui.painter().text(
                    r.center(),
                    Align2::CENTER_CENTER,
                    &name,
                    widgets::mono(30.0),
                    theme::ACCENT,
                );
                if resp.on_hover_text("All presets").clicked() {
                    p.tab = Tab::Presets;
                }
                if step(ui, "›").on_hover_text("Next preset").clicked() {
                    let s = sounding.map(|s| (s + 1) % preset::COUNT).unwrap_or(0);
                    p.send(Command::Load(s));
                }
                ui.add_space(14.0);
                ui.spacing_mut().item_spacing.x = 8.0;
                if let Some(slot) = sounding.filter(|_| !bypassed) {
                    if dirty {
                        ui.label(
                            RichText::new("● Changed, not saved")
                                .size(13.0)
                                .color(theme::ACCENT),
                        );
                        ui.add_space(4.0);
                        if widgets::primary(ui, &format!("Save to {}", preset::name(slot)))
                            .clicked()
                            && let Some(live) = p.live
                        {
                            p.send(Command::Save { slot, preset: live });
                        }
                        let mut options = vec!["Save to…".to_string()];
                        let others: Vec<usize> =
                            (0..preset::COUNT).filter(|s| *s != slot).collect();
                        options.extend(others.iter().map(|s| preset::name(*s)));
                        if let Some(i) = widgets::select(ui, "save-to", "Save to…", &options, 84.0)
                            .filter(|i| *i > 0)
                            && let Some(live) = p.live
                        {
                            p.send(Command::Save {
                                slot: others[i - 1],
                                preset: live,
                            });
                        }
                        if widgets::secondary(ui, "Revert").clicked() {
                            p.send(Command::Load(slot));
                        }
                    } else {
                        ui.label(widgets::dim(
                            "Saved on the pedal. Turn a knob to hear it change.",
                            13.0,
                        ));
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    let enabled = sounding.is_some();
                    ui.add_enabled_ui(enabled, |ui| {
                        if widgets::switch(ui, bypassed, false) {
                            p.send(Command::Bypass(!bypassed));
                        }
                    });
                    ui.label(RichText::new("Bypass").size(13.5));
                });
            });
        },
    );
}

/// Flex rows like the design: (grow, basis, min width) per block.
fn flex(b: &Block) -> (f32, f32, f32) {
    match b.name {
        "Amp" => (2.0, 380.0, 340.0),
        "Mod" => (1.0, 230.0, 230.0),
        _ => (1.0, 170.0, 170.0),
    }
}

fn chain(p: &mut Pedal, ui: &mut egui::Ui) {
    let editable = p.can_edit();
    let avail = ui.available_width();
    let gap = 10.0;
    // greedy rows by basis, then the leftover width is shared by grow
    let mut rows: Vec<Vec<usize>> = vec![];
    let mut used = 0.0;
    for (i, b) in BLOCKS.iter().enumerate() {
        let basis = flex(b).1;
        match rows.last_mut() {
            Some(row) if used + gap + basis <= avail => {
                row.push(i);
                used += gap + basis;
            }
            _ => {
                rows.push(vec![i]);
                used = basis;
            }
        }
    }
    let opacity = if p.bypassed() || p.live.is_none() {
        0.35
    } else {
        1.0
    };
    ui.scope(|ui| {
        ui.multiply_opacity(opacity);
        if !editable {
            ui.disable();
        }
        ui.spacing_mut().item_spacing.y = gap;
        for (ri, row) in rows.iter().enumerate() {
            let bases: f32 = row.iter().map(|i| flex(&BLOCKS[*i]).1).sum::<f32>()
                + gap * (row.len() as f32 - 1.0);
            let grow: f32 = row.iter().map(|i| flex(&BLOCKS[*i]).0).sum();
            let extra = (avail - bases).max(0.0);
            let height_id = ui.id().with(("chain-row", ri));
            let height: f32 = ui.data(|d| d.get_temp(height_id)).unwrap_or(0.0);
            let mut tallest: f32 = 0.0;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for &i in row {
                    let (g, basis, min) = flex(&BLOCKS[i]);
                    let w = (basis + extra * g / grow).max(min.min(avail));
                    ui.vertical(|ui| {
                        ui.set_width(w);
                        let h = block(p, ui, i, height);
                        tallest = tallest.max(h);
                    });
                }
            });
            if (tallest - height).abs() > 0.5 {
                ui.data_mut(|d| d.insert_temp(height_id, tallest));
                ui.ctx().request_repaint();
            }
        }
    });
}

/// One block card; returns its natural height so the row can match them.
fn block(p: &mut Pedal, ui: &mut egui::Ui, bi: usize, min_height: f32) -> f32 {
    let b = &BLOCKS[bi];
    let live = p.live.unwrap_or([0; preset::SIZE]);
    let on = b.is_on(&live);
    let border = if on {
        Color32::from_rgb(0x33, 0x35, 0x3a)
    } else {
        theme::LINE_SOFT
    };
    let r = egui::Frame::new()
        .fill(theme::PANEL)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(12)
        .inner_margin(egui::Margin {
            left: 16,
            right: 16,
            top: 14,
            bottom: 16,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let top = ui.cursor().min.y;
            ui.spacing_mut().item_spacing.y = 14.0;
            ui.horizontal(|ui| {
                ui.label(RichText::new(b.name).size(14.0).color(if on {
                    theme::TEXT
                } else {
                    theme::TEXT_DIM
                }));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::switch(ui, on, true) {
                        toggle(p, bi, !on);
                    }
                });
            });
            ui.scope(|ui| {
                if !on {
                    ui.multiply_opacity(0.4);
                }
                ui.spacing_mut().item_spacing.y = 14.0;
                match b.head {
                    Head::Stepper(prefix) => stepper(p, ui, &b.controls[0], prefix, &live),
                    Head::Segments(names) => {
                        let c = b.controls[0];
                        let cur =
                            (c.shown(&live) - c.min).clamp(0, names.len() as i32 - 1) as usize;
                        let labels = widgets::strings(names.iter());
                        if let Some(i) = widgets::segmented(&labels, Some(cur))
                            .font(12.0)
                            .pad(4.0, 5.0)
                            .fill(ui.available_width())
                            .show(ui)
                        {
                            p.set_byte(c.offset, c.stored(c.min + i as i32));
                        }
                    }
                    Head::None => {}
                }
                let knobs = b.knobs();
                if !knobs.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        let n = knobs.len() as f32;
                        let space = ((ui.available_width() - n * 62.0) / (n + 1.0)).max(4.0);
                        ui.spacing_mut().item_spacing = vec2(0.0, 8.0);
                        for (ki, c) in knobs.iter().enumerate() {
                            ui.add_space(space);
                            knob(p, ui, bi, ki, c, &live, on);
                        }
                    });
                }
            });
            // stretch to the row's tallest card, measured without the stretch
            let natural = ui.min_rect().max.y - top;
            if min_height > natural {
                ui.add_space(min_height - natural);
            }
            natural
        });
    r.inner
}

/// Gate and Cab are off at 0 and come back with the value they had; the others have a switch byte.
fn toggle(p: &mut Pedal, bi: usize, on: bool) {
    let b = &BLOCKS[bi];
    if let Some(o) = b.switch {
        p.set_byte(o, on as u8);
        return;
    }
    let c = b.controls[0];
    let Some(live) = p.live else { return };
    if on {
        let v = p
            .remembered
            .get(&c.offset)
            .copied()
            .or(preset::on_default(c.offset))
            .unwrap_or(1);
        p.set_byte(c.offset, v);
    } else {
        if live[c.offset] != 0 {
            p.remembered.insert(c.offset, live[c.offset]);
        }
        p.set_byte(c.offset, 0);
    }
}

/// The value a zero-means-off control shows while off: the one it gets back.
fn shown(p: &Pedal, c: &Control, live: &preset::Preset) -> i32 {
    let v = c.shown(live);
    if v == 0 && c.min > 0 {
        p.remembered
            .get(&c.offset)
            .copied()
            .or(preset::on_default(c.offset))
            .unwrap_or(1) as i32
    } else {
        v
    }
}

fn stepper(p: &mut Pedal, ui: &mut egui::Ui, c: &Control, prefix: &str, live: &preset::Preset) {
    let v = shown(p, c, live);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let small = |ui: &mut egui::Ui, t: &str| {
            widgets::button(t, widgets::Kind::Secondary)
                .width(28.0)
                .show(ui)
        };
        let minus = small(ui, "−").clicked();
        let w = ui.available_width() - 34.0;
        ui.add_sized(
            vec2(w, 28.0),
            egui::Label::new(RichText::new(format!("{prefix}{v}")).size(13.5)),
        );
        let plus = small(ui, "+").clicked();
        let next = if minus {
            v - 1
        } else if plus {
            v + 1
        } else {
            v
        };
        if next != v && (c.min..=c.max).contains(&next) {
            p.set_byte(c.offset, c.stored(next));
        }
    });
}

/// A 50px dial: drag up or down (160px = full range), double-click restores the stored value.
fn knob(
    p: &mut Pedal,
    ui: &mut egui::Ui,
    bi: usize,
    ki: usize,
    c: &Control,
    live: &preset::Preset,
    on: bool,
) {
    let v = shown(p, c, live);
    ui.vertical(|ui| {
        ui.set_width(62.0);
        ui.spacing_mut().item_spacing.y = 5.0;
        let (slot, resp) = ui.allocate_exact_size(vec2(62.0, 50.0), Sense::click_and_drag());
        let rect = Rect::from_center_size(slot.center(), egui::Vec2::splat(50.0));
        let pt = ui.painter();
        let t = ((v - c.min) as f32 / (c.max - c.min).max(1) as f32).clamp(0.0, 1.0);
        let arc = |from: f32, to: f32| -> Vec<egui::Pos2> {
            let n = 40;
            (0..=n)
                .map(|i| {
                    // 0° is straight up; the arc runs from −135° to +135° clockwise
                    let a = (from + (to - from) * i as f32 / n as f32 - 90.0).to_radians();
                    rect.center() + vec2(a.cos(), a.sin()) * 23.0
                })
                .collect()
        };
        pt.add(Shape::line(
            arc(-135.0, 135.0),
            Stroke::new(4.0, theme::LINE),
        ));
        if t > 0.0 {
            pt.add(Shape::line(
                arc(-135.0, -135.0 + 270.0 * t),
                Stroke::new(4.0, if on { theme::ACCENT } else { theme::TEXT_EMPTY }),
            ));
        }
        pt.circle_filled(rect.center(), 21.0, theme::HI);
        let a = (-135.0 + 270.0 * t - 90.0f32).to_radians();
        let dir = vec2(a.cos(), a.sin());
        pt.line_segment(
            [rect.center() + dir * 6.0, rect.center() + dir * 16.0],
            Stroke::new(2.0, theme::TEXT),
        );
        if resp.hovered() || resp.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
        if resp.drag_started()
            && let Some(pos) = resp.interact_pointer_pos()
        {
            p.drag = Some((bi, ki, v, pos.y));
        }
        if resp.dragged()
            && let (Some((db, dk, v0, y0)), Some(pos)) = (p.drag, resp.interact_pointer_pos())
            && (db, dk) == (bi, ki)
        {
            let nv = (v0 as f32 + (y0 - pos.y) / 160.0 * (c.max - c.min) as f32).round() as i32;
            let nv = nv.clamp(c.min, c.max);
            if nv != v {
                p.set_byte(c.offset, c.stored(nv));
            }
        }
        if resp.drag_stopped() {
            p.drag = None;
        }
        if resp.double_clicked()
            && let Some(stored) = p.sounding().and_then(|s| p.stored(s))
        {
            p.set_byte(c.offset, stored[c.offset]);
        }
        let resp = resp.on_hover_text("Drag up or down");
        let _ = resp;
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(RichText::new(v.to_string()).font(widgets::mono(13.0)));
            ui.label(widgets::dim(c.label, 12.0));
        });
    });
    let _ = StrokeKind::Inside;
}
