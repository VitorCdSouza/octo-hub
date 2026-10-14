//! The home screen: the three devices side by side; click one to open it.

use crate::app::{App, Screen};
use crate::icons::{self, Device};
use crate::{theme, widgets};
use eframe::egui;
use egui::{Align2, Color32, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

/// What a home card shows under the device name.
pub struct Headline {
    pub connected: bool,
    /// colour dot before the value (the mouse's DPI stage)
    pub dot: Option<Color32>,
    pub head: String,
    pub unit: String,
    pub battery: Option<(u8, bool)>,
    /// footer line: "Connected · USB" or why it is not
    pub conn: String,
}

pub fn name(d: Device) -> &'static str {
    match d {
        Device::Mouse => "Attack Shark X6",
        Device::Keyboard => "ZUOYA GMK70",
        Device::Pedal => "M-Vave Tank-G",
    }
}

fn kind(d: Device) -> &'static str {
    match d {
        Device::Mouse => "Mouse",
        Device::Keyboard => "Keyboard",
        Device::Pedal => "Guitar pedal",
    }
}

pub fn page(app: &mut App, ui: &mut egui::Ui) {
    let mut open = None;
    crate::app::content(ui, 1060.0, vec2(28.0, 48.0), |ui| {
        ui.label(egui::RichText::new("Devices").size(24.0).color(theme::TEXT));
        ui.add_space(14.0);
        let devices = [Device::Mouse, Device::Keyboard, Device::Pedal];
        let lines = [
            app.mouse.headline(),
            app.kb.headline(),
            app.pedal.headline(),
        ];
        widgets::grid(ui, 3, 290.0, 18.0, |ui, i| {
            if card(ui, devices[i], &lines[i]) {
                open = Some(devices[i]);
            }
        });
    });
    if let Some(d) = open {
        app.go(match d {
            Device::Mouse => Screen::Mouse,
            Device::Keyboard => Screen::Keyboard,
            Device::Pedal => Screen::Pedal,
        });
    }
}

/// One device card; true when clicked.
fn card(ui: &mut egui::Ui, d: Device, h: &Headline) -> bool {
    let w = ui.available_width();
    let height = 168.0 + 16.0 + 40.0 + 14.0 + 30.0 + 14.0 + 13.0 + 16.0 + 18.0;
    let (slot, resp) = ui.allocate_exact_size(vec2(w, height + 2.0), Sense::click());
    let lift = ui
        .ctx()
        .animate_bool_with_time(resp.id, resp.hovered(), 0.15)
        * 2.0;
    let r = Rect::from_min_size(slot.min + vec2(0.0, 2.0 - lift), vec2(w, height));
    let mut p = ui.painter().clone();
    if !h.connected {
        p.set_opacity(0.6);
    }
    let border = widgets::lerp_color(theme::LINE_SOFT, theme::HOVER_BORDER, lift / 2.0);
    p.rect(r, 14, theme::PANEL, Stroke::NONE, StrokeKind::Inside);
    // icon area
    let icon_area = Rect::from_min_size(r.min, vec2(w, 168.0));
    p.rect_filled(
        icon_area,
        egui::CornerRadius {
            nw: 14,
            ne: 14,
            sw: 0,
            se: 0,
        },
        theme::ICON_BG,
    );
    p.hline(
        icon_area.x_range(),
        icon_area.max.y,
        Stroke::new(1.0, theme::HOVER),
    );
    icons::device(
        &p,
        icons::square(icon_area.center(), 48.0 * 1.7),
        d,
        theme::TEXT_SOFT,
        2.0,
        false,
    );
    p.rect_stroke(r, 14, Stroke::new(1.0, border), StrokeKind::Inside);

    let x = r.min.x + 18.0;
    let mut y = icon_area.max.y + 16.0;
    p.text(
        pos2(x, y),
        Align2::LEFT_TOP,
        name(d),
        widgets::prop(16.0),
        theme::TEXT,
    );
    p.text(
        pos2(x, y + 22.0),
        Align2::LEFT_TOP,
        kind(d),
        widgets::prop(13.0),
        theme::TEXT_DIM,
    );
    y += 40.0 + 14.0;
    // headline stat
    let mid = y + 15.0;
    let mut hx = x;
    if let Some(c) = h.dot.filter(|_| h.connected) {
        p.circle_filled(pos2(hx + 4.5, mid), 4.5, c);
        hx += 16.0;
    }
    let head_font = if h.connected {
        widgets::mono(22.0)
    } else {
        widgets::prop(22.0)
    };
    let g = p.layout_no_wrap(h.head.clone(), head_font, theme::TEXT);
    let gw = g.size().x;
    p.galley(pos2(hx, mid - g.size().y / 2.0), g, theme::TEXT);
    if !h.unit.is_empty() {
        p.text(
            pos2(hx + gw + 7.0, mid + 3.0),
            Align2::LEFT_CENTER,
            &h.unit,
            widgets::prop(13.0),
            theme::TEXT_DIM,
        );
    }
    if let Some((b, awake)) = h.battery.filter(|_| h.connected) {
        let color = if awake { theme::OK } else { theme::TEXT_DIM };
        let text = format!("{b}%");
        let tg = p.layout_no_wrap(text, widgets::prop(13.0), color);
        let tx = r.max.x - 18.0 - tg.size().x;
        p.galley(pos2(tx, mid - tg.size().y / 2.0), tg, color);
        let br = Rect::from_min_size(pos2(tx - 7.0 - 22.0, mid - 6.0), vec2(22.0, 12.0));
        widgets::paint_battery(&p, br, b, color);
    }
    y += 30.0 + 14.0;
    // footer
    p.hline(
        (r.min.x + 18.0)..=(r.max.x - 18.0),
        y,
        Stroke::new(1.0, theme::EDGE),
    );
    y += 12.0 + 7.0;
    p.circle_filled(
        pos2(x + 3.5, y),
        3.5,
        if h.connected {
            theme::OK
        } else {
            theme::TEXT_EMPTY
        },
    );
    p.text(
        pos2(x + 15.0, y),
        Align2::LEFT_CENTER,
        &h.conn,
        widgets::prop(12.5),
        theme::TEXT_DIM,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}
