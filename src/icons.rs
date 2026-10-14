//! Line icons for the three devices and the logo, drawn with the painter.

use eframe::egui;
use egui::{Color32, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2, vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Device {
    Mouse,
    Keyboard,
    Pedal,
}

/// Draws a device icon designed on a 48×48 grid into `rect` (a square),
/// with strokes of `stroke` grid units. `bare` drops the pedal's knobs, as the
/// "not found" state shows it.
pub fn device(p: &Painter, rect: Rect, d: Device, color: Color32, stroke: f32, bare: bool) {
    let s = rect.width() / 48.0;
    let at = |x: f32, y: f32| rect.min + vec2(x * s, y * s);
    let r = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(at(x, y), vec2(w * s, h * s));
    let st = Stroke::new(stroke * s, color);
    match d {
        Device::Mouse => {
            p.rect_stroke(
                r(10.0, 4.0, 28.0, 40.0),
                (14.0 * s) as u8,
                st,
                StrokeKind::Inside,
            );
            p.rect_filled(r(23.0, 10.0, 2.0, 9.0), (1.0 * s) as u8, color);
        }
        Device::Keyboard => {
            p.rect_stroke(
                r(1.0, 11.0, 46.0, 27.0),
                (5.0 * s) as u8,
                st,
                StrokeKind::Inside,
            );
            let soft = color.gamma_multiply(0.8);
            p.rect_filled(r(8.0, 17.0, 32.0, 3.0), (1.5 * s) as u8, soft);
            p.rect_filled(r(8.0, 23.0, 32.0, 3.0), (1.5 * s) as u8, soft);
            p.rect_filled(r(15.0, 30.0, 18.0, 3.0), (1.5 * s) as u8, color);
        }
        Device::Pedal => {
            p.rect_stroke(
                r(3.0, 8.0, 42.0, 32.0),
                (6.0 * s) as u8,
                st,
                StrokeKind::Inside,
            );
            if !bare {
                for x in [11.0, 21.5, 32.0] {
                    p.circle_filled(at(x + 2.5, 16.5), 2.5 * s, color);
                }
            }
            for x in [10.0, 28.0] {
                p.circle_stroke(at(x + 5.0, 30.0), (5.0 - stroke / 2.0) * s, st);
            }
        }
    }
}

pub fn allocate(ui: &mut egui::Ui, size: f32) -> Rect {
    ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover())
        .0
}

pub fn square(center: Pos2, size: f32) -> Rect {
    Rect::from_center_size(center, Vec2::splat(size))
}
