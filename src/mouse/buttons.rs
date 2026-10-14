//! Button bindings: pick a part on the drawing or in the list, then what it does.

use super::{Mouse, art};
use crate::{theme, widgets};
use eframe::egui;
use egui::{RichText, Sense, pos2, vec2};
use remora::keys;
use remora::proto::{self, ACTIONS, Action, Category};

const CATEGORIES: [&str; 5] = ["Mouse", "DPI", "Media", "Keys", "Off"];

/// The left button only takes clicks, so it can't be lost.
const LEFT_ONLY: [u8; 4] = [0x02, 0x03, 0x07, 0x08];

/// Ready-made shortcuts in the Keys category, as (modifiers, key name).
const SHORTCUTS: [(u8, &str); 4] = [
    (proto::MOD_CTRL, "C"),
    (proto::MOD_CTRL, "V"),
    (proto::MOD_CTRL, "Z"),
    (proto::MOD_ALT, "Tab"),
];

fn category_of(a: Action) -> usize {
    match a.0 {
        proto::SHORTCUT => 3,
        proto::EASY_AIM => 1,
        0x01 => 4,
        _ => match a.category() {
            Category::Mouse => 0,
            Category::Dpi => 1,
            Category::Media | Category::Launch => 2,
            Category::Special => 3,
        },
    }
}

/// The actions a category lists for `button`, as (action, label).
fn actions(cat: usize, button: usize, current: Action) -> Vec<(Action, String)> {
    let simple = |pred: &dyn Fn(u8, Category) -> bool| -> Vec<(Action, String)> {
        ACTIONS
            .iter()
            .filter(|(id, _, c)| pred(*id, *c))
            .map(|(id, name, _)| (Action::simple(*id), name.to_string()))
            .collect()
    };
    if button == 0 {
        return simple(&|id, _| LEFT_ONLY.contains(&id));
    }
    match cat {
        0 => simple(&|id, c| c == Category::Mouse && id != 0x01),
        1 => {
            let mut v = simple(&|_, c| c == Category::Dpi);
            v.push((Action::easy_aim(3), "Easy aim (low DPI while held)".into()));
            v
        }
        2 => simple(&|_, c| c == Category::Media || c == Category::Launch),
        3 => {
            let mut v: Vec<(Action, String)> = SHORTCUTS
                .iter()
                .filter_map(|(m, k)| keys::from_name(k).map(|u| Action::shortcut(*m, u)))
                .map(|a| (a, a.label()))
                .collect();
            if current.0 == proto::SHORTCUT && !v.iter().any(|(a, _)| *a == current) {
                v.push((current, current.label()));
            }
            v
        }
        _ => simple(&|id, _| id == 0x01),
    }
}

pub fn page(m: &mut Mouse, ui: &mut egui::Ui) {
    let left_w = (ui.available_width() * 0.3).clamp(260.0, 320.0);
    widgets::two(ui, left_w, -1.0, 16.0, 660.0, |ui, i| {
        if i == 0 {
            figure(m, ui);
        } else {
            ui.spacing_mut().item_spacing.y = 16.0;
            picker(m, ui);
            debounce(m, ui);
        }
    });
}

fn select(m: &mut Mouse, b: usize) {
    m.edit_button = b;
    m.category = None;
    m.recording = false;
}

fn figure(m: &mut Mouse, ui: &mut egui::Ui) {
    widgets::card_with(
        ui,
        egui::Margin {
            left: 14,
            right: 14,
            top: 24,
            bottom: 10,
        },
        theme::LINE_SOFT,
        |ui| {
            ui.spacing_mut().item_spacing.y = 18.0;
            let size = art::size(true);
            let (r, resp) =
                ui.allocate_exact_size(vec2(ui.available_width(), size.y + 12.0), Sense::click());
            let origin = pos2(r.center().x - size.x / 2.0 + 5.0, r.min.y + 6.0);
            let hover = resp.hover_pos().and_then(|p| art::hit(origin, true, p));
            art::draw(
                ui.painter(),
                origin,
                true,
                Some(m.edit_button),
                hover.filter(|h| *h != m.edit_button),
                None,
            );
            if let Some(h) = hover {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                if resp.clicked() {
                    select(m, h);
                }
            }
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for (i, name) in proto::BUTTONS.iter().enumerate() {
                    let a = m.profile.buttons[i];
                    let on = m.edit_button == i;
                    let color = if on {
                        theme::ACCENT
                    } else if a == proto::DEFAULT_BUTTONS[i] {
                        theme::TEXT_DIM
                    } else {
                        theme::TEXT_SOFT
                    };
                    if row(ui, name, &a.label(), on, color).clicked() {
                        select(m, i);
                    }
                }
            });
        },
    );
}

/// A list row whose right text is 13px, as the button list has it.
fn row(
    ui: &mut egui::Ui,
    left: &str,
    right: &str,
    on: bool,
    right_color: egui::Color32,
) -> egui::Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 36.0), Sense::click());
    let p = ui.painter();
    let (fill, stroke) = if on {
        (theme::ACCENT_TINT, egui::Stroke::new(1.0, theme::ACCENT))
    } else if resp.hovered() {
        (theme::HOVER, egui::Stroke::NONE)
    } else {
        (egui::Color32::TRANSPARENT, egui::Stroke::NONE)
    };
    p.rect(rect, 8, fill, stroke, egui::StrokeKind::Inside);
    p.text(
        rect.left_center() + vec2(12.0, 0.0),
        egui::Align2::LEFT_CENTER,
        left,
        widgets::prop(14.0),
        theme::TEXT,
    );
    p.text(
        rect.right_center() - vec2(12.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        right,
        widgets::prop(13.0),
        right_color,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

fn picker(m: &mut Mouse, ui: &mut egui::Ui) {
    let b = m.edit_button;
    let current = m.profile.buttons[b];
    let default = proto::DEFAULT_BUTTONS[b];
    let cat = if b == 0 {
        0
    } else {
        m.category.unwrap_or_else(|| category_of(current))
    };
    widgets::card(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            ui.label(RichText::new(proto::BUTTONS[b]).size(17.0));
            ui.label(widgets::dim("does", 13.0));
            ui.label(
                RichText::new(current.label())
                    .size(15.0)
                    .color(theme::ACCENT),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::button("Default", widgets::Kind::Secondary)
                    .compact()
                    .muted(current == default)
                    .show(ui)
                    .clicked()
                    && current != default
                {
                    m.profile.buttons[b] = default;
                    m.category = None;
                    m.touch();
                }
            });
        });
        ui.add_space(16.0);
        let names: Vec<String> = if b == 0 {
            vec!["Mouse".into()]
        } else {
            widgets::strings(CATEGORIES)
        };
        if let Some(i) = widgets::segmented(&names, Some(cat)).show(ui) {
            m.category = Some(i);
            m.recording = false;
        }
        if b == 0 {
            ui.add_space(10.0);
            ui.label(widgets::hint(
                "The left button only takes clicks, so you can't lose it.",
            ));
        }
        ui.add_space(12.0);
        ui.spacing_mut().item_spacing.y = 2.0;
        for (a, label) in actions(cat, b, current) {
            let tag = if a == default { "default" } else { "" };
            if widgets::list_row(ui, &label, tag, a == current, None).clicked() && a != current {
                m.profile.buttons[b] = a;
                m.touch();
            }
        }
        if cat == 3 && b != 0 {
            ui.add_space(12.0);
            widgets::divider(ui);
            ui.add_space(12.0);
            record(m, ui);
        }
    });
}

fn record(m: &mut Mouse, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        let (label, kind) = if m.recording {
            ("● Listening…", widgets::Kind::Active)
        } else {
            ("Record a shortcut", widgets::Kind::Secondary)
        };
        if widgets::button(label, kind).show(ui).clicked() {
            m.recording = !m.recording;
        }
        ui.label(widgets::hint(if m.recording {
            "Press the keys together now. Esc cancels."
        } else {
            "Or press the keys yourself."
        }));
    });
    if !m.recording {
        return;
    }
    let pressed = ui.input(|i| {
        i.events.iter().find_map(|e| match e {
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => Some((*key, *modifiers)),
            _ => None,
        })
    });
    let Some((k, mods)) = pressed else { return };
    m.recording = false;
    if k == egui::Key::Escape {
        return;
    }
    match usage(k) {
        Some(u) => {
            let bits = if mods.ctrl { proto::MOD_CTRL } else { 0 }
                | if mods.shift { proto::MOD_SHIFT } else { 0 }
                | if mods.alt { proto::MOD_ALT } else { 0 };
            m.profile.buttons[m.edit_button] = Action::shortcut(bits, u);
            m.touch();
        }
        None => m
            .notes
            .push((format!("The mouse can't send {}", k.name()), false)),
    }
}

fn debounce(m: &mut Mouse, ui: &mut egui::Ui) {
    widgets::card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width((ui.available_width() - 290.0).max(140.0));
                ui.spacing_mut().item_spacing.y = 3.0;
                ui.label(RichText::new("Click response").size(14.0));
                ui.add(
                    egui::Label::new(widgets::hint(
                        "Debounce for every button. Raise it if a worn switch double-clicks.",
                    ))
                    .wrap(),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                let ms = m.profile.light.debounce_ms;
                ui.add_sized(
                    vec2(48.0, 22.0),
                    egui::Label::new(RichText::new(format!("{ms} ms")).font(widgets::mono(13.0))),
                );
                let mut v = ms as f64;
                if widgets::slider(
                    ui,
                    &mut v,
                    proto::DEBOUNCE_MIN as f64,
                    proto::DEBOUNCE_MAX as f64,
                    false,
                    200.0,
                )
                .changed
                {
                    let n = v.round() as u8;
                    if n != ms {
                        m.profile.light.debounce_ms = n;
                        m.touch();
                    }
                }
            });
        });
    });
}

/// HID usage for a key egui reports, through remora's key names.
fn usage(k: egui::Key) -> Option<u8> {
    use egui::Key::*;
    let name = match k {
        Enter => "Enter",
        Tab => "Tab",
        Space => "Space",
        Backspace => "Backspace",
        Delete => "Delete",
        Insert => "Insert",
        Home => "Home",
        End => "End",
        PageUp => "Page Up",
        PageDown => "Page Down",
        ArrowLeft => "Left",
        ArrowRight => "Right",
        ArrowUp => "Up",
        ArrowDown => "Down",
        Minus => "-",
        Equals => "=",
        OpenBracket => "[",
        CloseBracket => "]",
        Backslash => "\\",
        Semicolon => ";",
        Quote => "'",
        Backtick => "`",
        Comma => ",",
        Period => ".",
        Slash => "/",
        Num0 => "0",
        Num1 => "1",
        Num2 => "2",
        Num3 => "3",
        Num4 => "4",
        Num5 => "5",
        Num6 => "6",
        Num7 => "7",
        Num8 => "8",
        Num9 => "9",
        other => return keys::from_name(other.name()),
    };
    keys::from_name(name)
}
