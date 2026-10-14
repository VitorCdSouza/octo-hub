//! The keyboard drawn from its VIA definition in the design's flat caps:
//! cream alphas, grey mods, accent caps and a round knob.

use crate::theme;
use eframe::egui;
use egui::{
    Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, epaint::Shadow,
    pos2, vec2,
};
use hidra::definition::{Definition, Key};

pub struct KeyVisual {
    pub label: String,
    /// cap colour and legend colour; None uses the definition's colour
    pub colors: Option<(Color32, Color32)>,
    pub selected: bool,
    pub glow: Option<Color32>,
    pub tooltip: Option<String>,
}

/// Cap and legend colour for a definition colour: alpha, modifier or accent.
pub fn cap_colors(k: &Key) -> (Color32, Color32) {
    if k.encoder.is_some() {
        return (theme::ACCENT, theme::ACCENT_TEXT);
    }
    let hex = k.color.trim_start_matches('#');
    let lum = u32::from_str_radix(hex, 16)
        .map(|v| {
            let (r, g, b) = ((v >> 16) & 0xff, (v >> 8) & 0xff, v & 0xff);
            (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32) / 255.0
        })
        .unwrap_or(0.8);
    if lum > 0.75 {
        (theme::CAP_ALPHA, theme::CAP_LEGEND)
    } else if lum > 0.55 {
        (theme::CAP_MOD, theme::CAP_LEGEND)
    } else {
        (theme::ACCENT, theme::ACCENT_TEXT)
    }
}

pub fn is_mod(k: &Key) -> bool {
    cap_colors(k).0 == theme::CAP_MOD
}

fn bounds(def: &Definition, options: &[usize]) -> Rect {
    let mut r = Rect::NOTHING;
    for (_, k) in def.visible_keys(options) {
        r.extend_with(pos2(k.x, k.y));
        r.extend_with(pos2(k.x + k.w, k.y + k.h));
    }
    if r.is_negative() {
        Rect::from_min_size(Pos2::ZERO, Vec2::splat(1.0))
    } else {
        r
    }
}

/// One flat cap with a darker 4px lip at the bottom, as `inset 0 -4px 0 rgba(0,0,0,.2)`.
pub fn paint_cap(
    p: &egui::Painter,
    r: Rect,
    radius: f32,
    fill: Color32,
    legend: Color32,
    label: &str,
    scale: f32,
    glow: Option<Color32>,
) {
    let cr = CornerRadius::same(radius as u8);
    if let Some(g) = glow {
        // "0 6px 14px -3px g, 0 2px 0 0 g"
        let s = Shadow {
            offset: [0, (6.0 * scale) as i8],
            blur: (14.0 * scale) as u8,
            spread: 0,
            color: g,
        };
        p.add(s.as_shape(r.shrink(3.0), cr));
        p.rect_filled(r.translate(vec2(0.0, 2.0)), cr, g);
    }
    p.rect_filled(r, cr, theme::shade(fill, 0.8));
    p.rect_filled(
        Rect::from_min_max(r.min, pos2(r.max.x, r.max.y - 4.0 * scale.max(0.6))),
        cr,
        fill,
    );
    if label.is_empty() {
        return;
    }
    let lines: Vec<&str> = label.lines().collect();
    let long = lines.iter().any(|l| l.chars().count() > 3);
    let size = if long {
        11.5
    } else if lines.len() > 1 {
        12.5
    } else {
        15.0
    } * scale.clamp(0.7, 1.1);
    let mut s = size;
    let max = r.width() - 6.0;
    let galley = loop {
        let g = p.layout(
            label.to_string(),
            egui::FontId::proportional(s),
            legend,
            max,
        );
        if (g.size().x <= max && g.size().y <= r.height() - 6.0) || s <= 7.0 {
            break g;
        }
        s -= 0.5;
    };
    let c = r.center() - vec2(0.0, 2.0 * scale);
    p.galley(c - galley.size() / 2.0, galley, legend);
}

pub struct BoardResponse {
    pub clicked: Option<usize>,
}

pub fn show(
    ui: &mut egui::Ui,
    def: &Definition,
    options: &[usize],
    visual: &dyn Fn(usize) -> KeyVisual,
) -> BoardResponse {
    let bb = bounds(def, options);
    let avail = ui.available_width();
    let width = avail.min(1100.0);
    let unit = width / bb.width();
    let scale = unit / 62.0;
    let size = vec2(width, bb.height() * unit);
    let (rect, resp) = ui.allocate_exact_size(vec2(avail, size.y), Sense::click());
    let origin = pos2(rect.min.x + (avail - width) / 2.0, rect.min.y);
    let to_screen = |x: f32, y: f32| {
        pos2(
            origin.x + (x - bb.min.x) * unit,
            origin.y + (y - bb.min.y) * unit,
        )
    };
    let p = ui.painter_at(rect.expand(20.0));
    let pointer = resp.hover_pos();
    let mut out = BoardResponse { clicked: None };
    for (idx, k) in def.visible_keys(options) {
        let v = visual(idx);
        let outer =
            Rect::from_min_max(to_screen(k.x, k.y), to_screen(k.x + k.w, k.y + k.h)).shrink(2.5);
        let (fill, legend) = v.colors.unwrap_or_else(|| cap_colors(k));
        let hit = pointer.is_some_and(|q| outer.contains(q));
        if hit {
            if resp.clicked() {
                out.clicked = Some(idx);
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            if let Some(t) = &v.tooltip {
                resp.clone().on_hover_text_at_pointer(t);
            }
        }
        let fill = if hit { theme::shade(fill, 1.05) } else { fill };
        if k.encoder.is_some() {
            let rad = outer.width().min(outer.height()) / 2.0;
            let sq = Rect::from_center_size(outer.center(), Vec2::splat(rad * 2.0));
            paint_cap(&p, sq, rad, fill, legend, &v.label, scale, v.glow);
            if v.selected {
                p.circle_stroke(sq.center(), rad + 1.0, Stroke::new(2.0, theme::PANEL));
                p.circle_stroke(sq.center(), rad + 3.0, Stroke::new(2.0, theme::ACCENT));
            }
        } else {
            paint_cap(&p, outer, 7.0, fill, legend, &v.label, scale, v.glow);
            if v.selected {
                p.rect_stroke(
                    outer,
                    7,
                    Stroke::new(2.0, theme::PANEL),
                    StrokeKind::Outside,
                );
                p.rect_stroke(
                    outer.expand(2.0),
                    9,
                    Stroke::new(2.0, theme::ACCENT),
                    StrokeKind::Outside,
                );
            }
        }
    }
    let _ = Align2::CENTER_CENTER;
    out
}

/// The board inside its card, padded 20×26.
pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    crate::widgets::card_with(ui, egui::Margin::symmetric(26, 20), theme::LINE_SOFT, add)
}
