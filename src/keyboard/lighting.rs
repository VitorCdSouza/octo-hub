//! Lighting: the board with an underglow preview, then effect, backlight and
//! colour cards built from the definition's lighting menu (as in hidra).

use super::{Keyboard, board};
use crate::{theme, widgets};
use eframe::egui;
use egui::{Color32, RichText, Sense, Stroke, vec2};
use hidra::keycode;
use hidra::menus::{self, Control, Item};

struct Layout {
    effect: Item,
    ranges: Vec<Item>,
    color: Option<Item>,
}

/// A long dropdown (the effect) turns the menu into the card layout.
fn find_layout(k: &Keyboard) -> Option<Layout> {
    let s = k.session.as_ref()?;
    let items: Vec<Item> = menus::items(&s.menus).into_iter().cloned().collect();
    let effect = items
        .iter()
        .find(|i| matches!(&i.control, Control::Dropdown(o) if o.len() > 12))?
        .clone();
    let mut l = Layout {
        effect: effect.clone(),
        ranges: vec![],
        color: None,
    };
    for i in items {
        match i.control {
            _ if i.id == effect.id => {}
            Control::Range { .. } => l.ranges.push(i),
            Control::Color if l.color.is_none() => l.color = Some(i),
            _ => {}
        }
    }
    Some(l)
}

fn hsv(h: f32, s: f32, v: f32) -> Color32 {
    egui::ecolor::HsvaGamma {
        h: h.rem_euclid(1.0),
        s,
        v,
        a: 1.0,
    }
    .into()
}

fn visible(k: &Keyboard, show_if: &Option<String>) -> bool {
    let (Some(expr), Some(s)) = (show_if, &k.session) else {
        return true;
    };
    menus::eval_show_if(expr, &|id| s.menu_number(id))
}

fn effect_name(k: &Keyboard, l: &Layout) -> String {
    let s = k.session.as_ref().unwrap();
    let fx = s
        .menu_values
        .get(&l.effect.id)
        .and_then(|v| v.first().copied());
    let Control::Dropdown(opts) = &l.effect.control else {
        return String::new();
    };
    opts.iter()
        .find(|(_, v)| Some(*v) == fx)
        .map(|(n, _)| n.clone())
        .unwrap_or_default()
}

pub fn page(k: &mut Keyboard, ui: &mut egui::Ui) {
    let Some(l) = find_layout(k) else {
        widgets::card(ui, |ui| {
            ui.label(widgets::dim("This keyboard's definition has no lighting menu octo hub understands. Use hidra for it.", 13.0));
        });
        return;
    };
    preview(k, ui, &l);
    ui.add_space(16.0);
    widgets::columns(ui, 3, 220.0, 16.0, |ui, i| match i {
        0 => effects(k, ui, &l),
        1 => backlight(k, ui, &l),
        _ => colour(k, ui, &l),
    });
}

fn preview(k: &mut Keyboard, ui: &mut egui::Ui, l: &Layout) {
    let Some(s) = &k.session else { return };
    let def = s.def.clone();
    let opts = s.layout_options.clone();
    let custom = def.custom_keycodes.clone();
    let name = effect_name(k, l).to_lowercase();
    let (hue, sat) = l
        .color
        .as_ref()
        .and_then(|c| s.menu_values.get(&c.id))
        .map(|v| {
            (
                v.first().copied().unwrap_or(0),
                v.get(1).copied().unwrap_or(255),
            )
        })
        .unwrap_or((0, 255));
    let bri = l
        .ranges
        .iter()
        .find(|r| r.id.contains("brightness") || r.label.to_lowercase().contains("bright"))
        .map(|r| s.menu_number(&r.id) as f32)
        .unwrap_or(255.0);
    let alpha = 0.25 + 0.75 * bri.min(255.0) / 255.0;
    let h0 = hue as f32 / 255.0;
    let sat = sat as f32 / 255.0;
    let glow = |idx: usize| -> Option<Color32> {
        let key = &def.keys[idx];
        let c = if name == "all off" || name.is_empty() {
            return None;
        } else if name.contains("alphas mods") {
            hsv(
                if board::is_mod(key) {
                    h0 + 40.0 / 360.0
                } else {
                    h0
                },
                sat,
                1.0,
            )
        } else if name.contains("up down") {
            hsv(h0 + key.y * 18.0 / 360.0, sat, 1.0)
        } else if name.contains("left right") || name.starts_with("gradient") {
            hsv(h0 + key.x * 9.0 / 360.0, sat, 1.0)
        } else if name.contains("band") {
            hsv(
                h0,
                ((30.0 + (8.0 - key.x).abs() * 9.0) / 100.0).min(1.0),
                1.0,
            )
        } else if name.starts_with("cycle")
            || name.starts_with("rainbow")
            || name.contains("pinwheel")
            || name.contains("spiral")
        {
            hsv(
                key.x * 20.0 / 360.0
                    + if name.contains("chevron") {
                        key.y * 25.0 / 360.0
                    } else {
                        0.0
                    },
                1.0,
                1.0,
            )
        } else if name.contains("raindrops")
            || name.contains("pixel")
            || name.contains("heatmap")
            || name.contains("rain")
        {
            hsv(h0 + (idx * 47 % 256) as f32 / 255.0, 1.0, 1.0)
        } else {
            hsv(h0, sat, 1.0)
        };
        Some(c.gamma_multiply(alpha))
    };
    let visual = |idx: usize| {
        let key = &def.keys[idx];
        let code = s
            .keymap
            .first()
            .map(|l| l[s.key_index(key.row, key.col)])
            .unwrap_or(0);
        board::KeyVisual {
            label: keycode::label(code, &custom, key.w),
            colors: None,
            selected: false,
            glow: glow(idx),
            tooltip: None,
        }
    };
    board::card(ui, |ui| board::show(ui, &def, &opts, &visual));
}

fn effects(k: &mut Keyboard, ui: &mut egui::Ui, l: &Layout) {
    let Control::Dropdown(opts) = &l.effect.control else {
        return;
    };
    let cur = k
        .session
        .as_ref()
        .and_then(|s| s.menu_values.get(&l.effect.id))
        .and_then(|v| v.first().copied());
    widgets::card_with(ui, egui::Margin::same(12), theme::LINE_SOFT, |ui| {
        ui.spacing_mut().item_spacing.y = 8.0;
        widgets::text_field(
            ui,
            &mut k.fx_filter,
            "Filter effects",
            ui.available_width(),
            false,
        );
        let q = k.fx_filter.trim().to_lowercase();
        let mut pick = None;
        egui::ScrollArea::vertical()
            .max_height(300.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                for (name, v) in opts
                    .iter()
                    .filter(|(n, _)| q.is_empty() || n.to_lowercase().contains(&q))
                {
                    if widgets::list_row_sized(ui, name, "", cur == Some(*v), None, 13.5).clicked()
                    {
                        pick = Some(*v);
                    }
                }
            });
        if let Some(v) = pick
            && k.with_session(|s| s.set_menu_value(&l.effect.id, &[v]))
                .is_some()
        {
            k.ok("Written to the keyboard");
        }
    });
}

fn range(k: &mut Keyboard, ui: &mut egui::Ui, item: &Item) {
    let Control::Range { min, max } = item.control else {
        return;
    };
    let Some(s) = &k.session else { return };
    let raw = s.menu_values.get(&item.id).cloned().unwrap_or_default();
    let v = menus::numeric(item, &raw) as f64;
    ui.horizontal(|ui| {
        ui.label(RichText::new(&item.label).size(13.5));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let pct = ((v - min as f64) / (max as f64 - min as f64).max(1.0) * 100.0).round();
            ui.label(
                RichText::new(format!("{pct}%"))
                    .font(widgets::mono(13.0))
                    .color(theme::TEXT_DIM),
            );
        });
    });
    let mut nv = v;
    let r = widgets::slider(
        ui,
        &mut nv,
        min as f64,
        max as f64,
        false,
        ui.available_width(),
    );
    let n = nv.round() as u16;
    let bytes = if max > 255 {
        n.to_be_bytes().to_vec()
    } else {
        vec![n as u8]
    };
    if r.changed && n as f64 != v {
        // preview while dragging, store once the hand lets go
        k.with_session(|s| s.preview_menu_value(&item.id, &bytes));
        k.menu_drag = Some(item.id.clone());
    }
    if r.released && k.menu_drag.as_deref() == Some(&item.id) {
        k.menu_drag = None;
        if k.with_session(|s| s.save_menu_channel(&item.id)).is_some() {
            k.ok("Written to the keyboard");
        }
    }
}

fn backlight(k: &mut Keyboard, ui: &mut egui::Ui, l: &Layout) {
    widgets::card(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 8.0;
        let mut first = true;
        for r in &l.ranges {
            if !visible(k, &r.show_if) {
                continue;
            }
            if !first {
                ui.add_space(10.0);
            }
            first = false;
            range(k, ui, r);
        }
        if effect_name(k, l).eq_ignore_ascii_case("all off") {
            ui.add_space(6.0);
            ui.label(widgets::dim(
                "The lights are off. Pick an effect to turn them on.",
                13.0,
            ));
        }
    });
}

const SWATCHES: [(u8, [u8; 3]); 8] = [
    (0, [0xff, 0x2a, 0x2a]),
    (20, [0xff, 0x8a, 0x1f]),
    (36, [0xff, 0xd2, 0x1f]),
    (85, [0x2b, 0xff, 0x4a]),
    (128, [0x24, 0xf0, 0xf0]),
    (170, [0x2a, 0x4b, 0xff]),
    (200, [0xa2, 0x3b, 0xff]),
    (230, [0xff, 0x3b, 0xb0]),
];

fn colour(k: &mut Keyboard, ui: &mut egui::Ui, l: &Layout) {
    let Some(item) = l.color.clone() else {
        widgets::card(ui, |ui| {
            ui.label(widgets::dim("This keyboard has no colour setting.", 13.0))
        });
        return;
    };
    let uses = visible(k, &item.show_if);
    let off = effect_name(k, l).eq_ignore_ascii_case("all off");
    let raw = k
        .session
        .as_ref()
        .and_then(|s| s.menu_values.get(&item.id))
        .cloned()
        .unwrap_or_default();
    let (h, s) = (
        raw.first().copied().unwrap_or(0),
        raw.get(1).copied().unwrap_or(255),
    );
    widgets::card(ui, |ui| {
        if !uses {
            ui.multiply_opacity(0.45);
        }
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Colour").size(13.5));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (r, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
                ui.painter()
                    .rect_filled(r, 6, hsv(h as f32 / 255.0, s as f32 / 255.0, 1.0));
            });
        });
        ui.add_space(12.0);
        // rainbow strip: hue across, saturation kept
        let (r, resp) = ui.allocate_exact_size(
            vec2(ui.available_width(), 26.0),
            if uses {
                Sense::click_and_drag()
            } else {
                Sense::hover()
            },
        );
        let mut mesh = egui::Mesh::default();
        let n = 48;
        for i in 0..n {
            let x0 = r.min.x + r.width() * i as f32 / n as f32;
            let x1 = r.min.x + r.width() * (i + 1) as f32 / n as f32;
            let c0 = hsv(i as f32 / n as f32, 1.0, 1.0);
            let c1 = hsv((i + 1) as f32 / n as f32, 1.0, 1.0);
            let b = mesh.vertices.len() as u32;
            mesh.colored_vertex(egui::pos2(x0, r.min.y), c0);
            mesh.colored_vertex(egui::pos2(x1, r.min.y), c1);
            mesh.colored_vertex(egui::pos2(x1, r.max.y), c1);
            mesh.colored_vertex(egui::pos2(x0, r.max.y), c0);
            mesh.add_triangle(b, b + 1, b + 2);
            mesh.add_triangle(b, b + 2, b + 3);
        }
        ui.painter().add(egui::Shape::mesh(mesh));
        let mx = r.min.x + r.width() * h as f32 / 255.0;
        let marker = egui::Rect::from_center_size(egui::pos2(mx, r.center().y), vec2(12.0, 32.0));
        ui.painter().rect_stroke(
            marker,
            4,
            Stroke::new(2.0, theme::TEXT),
            egui::StrokeKind::Inside,
        );
        if let Some(p) = resp.interact_pointer_pos()
            && (resp.dragged() || resp.clicked())
        {
            let nh = (((p.x - r.min.x) / r.width()).clamp(0.0, 1.0) * 255.0).round() as u8;
            if nh != h {
                k.with_session(|x| x.preview_menu_value(&item.id, &[nh, s]));
            }
        }
        if (resp.drag_stopped() || resp.clicked())
            && k.with_session(|x| x.save_menu_channel(&item.id)).is_some()
        {
            k.ok("Written to the keyboard");
        }
        ui.add_space(16.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            for (sh, c) in SWATCHES {
                let (r, resp) = ui.allocate_exact_size(
                    vec2(26.0, 26.0),
                    if uses { Sense::click() } else { Sense::hover() },
                );
                ui.painter().rect_filled(r, 7, theme::rgb(c));
                if (h as i32 - sh as i32).abs() < 3 {
                    widgets::swatch_ring(ui.painter(), r, 7.0, theme::PANEL);
                }
                if resp.clicked()
                    && k.with_session(|x| x.set_menu_value(&item.id, &[sh, 255]))
                        .is_some()
                {
                    k.ok("Written to the keyboard");
                }
            }
        });
        if !uses && !off {
            ui.add_space(14.0);
            ui.label(widgets::hint("This effect picks its own colours."));
        }
    });
}
