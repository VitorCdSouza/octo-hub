//! The X6 as a simple line drawing seen from above: a pill body, two click
//! buttons, the wheel, the DPI housing with its two buttons and the LED, and
//! on the Buttons page the two side buttons.

use crate::theme;
use eframe::egui;
use egui::{Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, pos2, vec2};

/// Sizes from the design, for the Lighting page (small) and Buttons page (large).
struct Geo {
    w: f32,
    h: f32,
    split: f32,
    wheel: (f32, f32, f32),
    housing: (f32, f32, f32),
    dpi: (f32, f32, f32, f32),
    led: f32,
}

const SMALL: Geo = Geo {
    w: 124.0,
    h: 208.0,
    split: 81.0,
    wheel: (20.0, 34.0, 14.0),
    housing: (24.0, 80.0, 44.0),
    dpi: (14.0, 18.0, 62.0, 84.0),
    led: 110.0,
};
const LARGE: Geo = Geo {
    w: 136.0,
    h: 228.0,
    split: 87.0,
    wheel: (22.0, 38.0, 16.0),
    housing: (26.0, 84.0, 50.0),
    dpi: (16.0, 20.0, 66.0, 90.0),
    led: 118.0,
};

pub fn size(large: bool) -> egui::Vec2 {
    let g = if large { &LARGE } else { &SMALL };
    vec2(g.w, g.h)
}

/// Areas of the drawing, in `remora::proto::BUTTONS` order:
/// left, right, wheel, forward, back, DPI +, DPI −.
pub struct Parts {
    pub zones: [Vec<Pos2>; 7],
    pub rects: [Rect; 7],
    pub led: Pos2,
}

fn quarter(c: Pos2, r: f32, from: f32, to: f32) -> Vec<Pos2> {
    (0..=16)
        .map(|i| {
            let a = from + (to - from) * i as f32 / 16.0;
            c + vec2(a.cos(), a.sin()) * r
        })
        .collect()
}

pub fn parts(origin: Pos2, large: bool) -> Parts {
    let g = if large { &LARGE } else { &SMALL };
    let o = origin;
    let cx = o.x + g.w / 2.0;
    let r = g.w / 2.0;
    let top = pos2(cx, o.y + r);
    use std::f32::consts::PI;
    let mut left = quarter(top, r, PI, 1.5 * PI);
    left.extend([pos2(cx, o.y + g.split), pos2(o.x, o.y + g.split)]);
    let mut right = vec![pos2(cx, o.y)];
    right.extend(quarter(top, r, 1.5 * PI, 2.0 * PI));
    right.extend([pos2(o.x + g.w, o.y + g.split), pos2(cx, o.y + g.split)]);
    let rect =
        |w: f32, h: f32, y: f32| Rect::from_min_size(pos2(cx - w / 2.0, o.y + y), vec2(w, h));
    let wheel = rect(g.wheel.0, g.wheel.1, g.wheel.2);
    let up = rect(g.dpi.0, g.dpi.1, g.dpi.2);
    let down = rect(g.dpi.0, g.dpi.1, g.dpi.3);
    let forward = Rect::from_min_size(pos2(o.x - 9.0, o.y + 118.0), vec2(9.0, 32.0));
    let back = Rect::from_min_size(pos2(o.x - 9.0, o.y + 156.0), vec2(9.0, 32.0));
    let poly = |r: Rect| {
        vec![
            r.left_top(),
            r.right_top(),
            r.right_bottom(),
            r.left_bottom(),
        ]
    };
    let bounds = |pts: &[Pos2]| Rect::from_points(pts);
    Parts {
        rects: [
            bounds(&left),
            bounds(&right),
            wheel,
            forward,
            back,
            up,
            down,
        ],
        zones: [
            left,
            right,
            poly(wheel),
            poly(forward),
            poly(back),
            poly(up),
            poly(down),
        ],
        led: pos2(cx, o.y + g.led + 4.0),
    }
}

/// `selected` is the part drawn in accent, `hovered` one drawn a little lighter.
pub fn draw(
    p: &Painter,
    origin: Pos2,
    large: bool,
    selected: Option<usize>,
    hovered: Option<usize>,
    led: Option<Color32>,
) {
    let g = if large { &LARGE } else { &SMALL };
    let parts = parts(origin, large);
    let line = Stroke::new(2.0, theme::DRAW_LINE);
    let body = Rect::from_min_size(origin, vec2(g.w, g.h));
    let cx = body.center().x;
    // click buttons: fill under the outline when picked or hovered
    for i in [0usize, 1] {
        if selected == Some(i) {
            p.add(Shape::convex_polygon(
                parts.zones[i].clone(),
                Color32::from_rgba_unmultiplied(240, 145, 78, 46),
                Stroke::NONE,
            ));
        } else if hovered == Some(i) {
            p.add(Shape::convex_polygon(
                parts.zones[i].clone(),
                Color32::from_white_alpha(8),
                Stroke::NONE,
            ));
        }
    }
    p.rect_stroke(
        body,
        CornerRadius::same((g.w / 2.0) as u8),
        line,
        StrokeKind::Inside,
    );
    let divider = Stroke::new(2.0, theme::DRAW_DIVIDER);
    p.hline(
        (body.min.x + 2.0)..=(body.max.x - 2.0),
        origin.y + g.split,
        divider,
    );
    p.vline(cx, (origin.y + 2.0)..=(origin.y + g.split), divider);
    for i in [0usize, 1] {
        if selected == Some(i) {
            p.add(Shape::closed_line(
                parts.zones[i].clone(),
                Stroke::new(1.5, theme::ACCENT),
            ));
        }
    }
    let housing = Rect::from_min_size(
        pos2(cx - g.housing.0 / 2.0, origin.y + g.housing.2),
        vec2(g.housing.0, g.housing.1),
    );
    p.rect(housing, 6, theme::PANEL, line, StrokeKind::Inside);
    // small parts: solid accent when picked
    let small = |i: usize, radius: u8| {
        let r = parts.rects[i];
        let (fill, stroke) = if selected == Some(i) {
            (theme::ACCENT, theme::ACCENT)
        } else if hovered == Some(i) {
            (theme::HOVER, theme::TEXT_DIM)
        } else {
            (
                if large { theme::BG } else { theme::PANEL },
                theme::DRAW_LINE,
            )
        };
        p.rect(
            r,
            radius,
            fill,
            Stroke::new(2.0, stroke),
            StrokeKind::Inside,
        );
    };
    small(2, (g.wheel.0 / 2.0) as u8);
    small(5, 4);
    small(6, 4);
    if large {
        for i in [3usize, 4] {
            let r = parts.rects[i];
            let (fill, stroke) = if selected == Some(i) {
                (theme::ACCENT, theme::ACCENT)
            } else if hovered == Some(i) {
                (theme::HOVER, theme::TEXT_DIM)
            } else {
                (theme::BG, theme::DRAW_LINE)
            };
            let cr = CornerRadius {
                nw: 5,
                sw: 5,
                ne: 0,
                se: 0,
            };
            p.rect(r, cr, fill, Stroke::new(2.0, stroke), StrokeKind::Inside);
        }
    }
    // the LED below the DPI buttons
    p.circle_filled(parts.led, 7.0, theme::PANEL);
    match led {
        Some(c) => {
            for (rad, a) in [(14.0, 0.10), (10.0, 0.18), (7.0, 0.35)] {
                p.circle_filled(parts.led, rad, c.gamma_multiply(a));
            }
            p.circle_filled(parts.led, 4.0, c);
        }
        None => {
            p.circle_filled(
                parts.led,
                4.0,
                if large {
                    theme::TEXT_FAINT
                } else {
                    theme::LINE
                },
            );
        }
    }
}

pub fn hit(origin: Pos2, large: bool, at: Pos2) -> Option<usize> {
    let parts = parts(origin, large);
    // small parts first: they sit on top of the click buttons
    [2usize, 5, 6, 3, 4, 0, 1].into_iter().find(|&i| {
        if i <= 1 {
            contains(&parts.zones[i], at)
        } else {
            parts.rects[i].expand(1.0).contains(at)
        }
    })
}

fn contains(poly: &[Pos2], p: Pos2) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}
