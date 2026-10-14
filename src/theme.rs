//! The design tokens from the octo hub handoff: one dark theme for every
//! device, warm greys and a single orange accent.

use eframe::egui;
use egui::{Color32, FontData, FontDefinitions, FontFamily, Stroke, Visuals};
use std::sync::Arc;

const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

pub const BG: Color32 = hex(0x131416);
pub const TOPBAR: Color32 = hex(0x17181b);
pub const ICON_BG: Color32 = hex(0x16171a);
pub const PANEL: Color32 = hex(0x1a1b1e);
pub const CARD_ALT: Color32 = hex(0x1e2023);
pub const HI: Color32 = hex(0x26282c);
pub const HOVER: Color32 = hex(0x222428);
pub const TAB_ON: Color32 = hex(0x2a2c30);
pub const LINE: Color32 = hex(0x2e3035);
/// card border
pub const LINE_SOFT: Color32 = hex(0x26282c);
/// row divider inside a card
pub const DIVIDER: Color32 = hex(0x25272b);
/// top bar border, tab track border
pub const EDGE: Color32 = hex(0x232529);
pub const HOVER_BORDER: Color32 = hex(0x4a4c52);

pub const TEXT: Color32 = hex(0xe8e6e1);
pub const TEXT_SOFT: Color32 = hex(0xc4c2bc);
pub const TEXT_DIM: Color32 = hex(0x8e8f93);
pub const TEXT_FAINT: Color32 = hex(0x6b6d72);
pub const TEXT_EMPTY: Color32 = hex(0x55575c);

pub const ACCENT: Color32 = hex(0xf0914e);
pub const ACCENT_DIM: Color32 = hex(0x8c5128);
pub const ACCENT_TINT: Color32 = hex(0x2a201a);
pub const ACCENT_TEXT: Color32 = hex(0x2a1406);
pub const OK: Color32 = hex(0x8cc79b);
pub const DANGER_TEXT: Color32 = hex(0xf0a596);
pub const DANGER_LINE: Color32 = hex(0x6e3a32);
pub const DANGER_FILL: Color32 = hex(0x241a18);
pub const ERR: Color32 = hex(0xe8685a);
pub const SWITCH_OFF: Color32 = hex(0x3a3c42);

pub const CAP_ALPHA: Color32 = hex(0xe6e2da);
pub const CAP_MOD: Color32 = hex(0xa8a49c);
pub const CAP_LEGEND: Color32 = hex(0x2a2825);
pub const CAP_GHOST: Color32 = hex(0x2a2c30);

/// mouse line drawing
pub const DRAW_LINE: Color32 = hex(0x55585e);
pub const DRAW_DIVIDER: Color32 = hex(0x3a3d42);

/// macro step chips
pub const STEP_FILL: Color32 = hex(0x352519);
pub const STEP_KIND: Color32 = hex(0xd7a27a);
pub const STEP_TEXT: Color32 = hex(0xf3d2b8);

pub fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

pub fn shade(c: Color32, f: f32) -> Color32 {
    let s = |v: u8| ((v as f32 * f).round().clamp(0.0, 255.0)) as u8;
    Color32::from_rgb(s(c.r()), s(c.g()), s(c.b()))
}

pub fn apply(ctx: &egui::Context) {
    let mut v = Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = CARD_ALT;
    v.extreme_bg_color = BG;
    v.faint_bg_color = HI;
    v.override_text_color = Some(TEXT);
    v.hyperlink_color = ACCENT;
    v.selection.bg_fill = ACCENT_DIM;
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.inactive.bg_fill = HI;
    v.widgets.inactive.weak_bg_fill = HI;
    v.widgets.hovered.bg_fill = HOVER;
    v.widgets.hovered.weak_bg_fill = HOVER;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, HOVER_BORDER);
    v.widgets.active.bg_fill = ACCENT_DIM;
    v.widgets.active.weak_bg_fill = ACCENT_DIM;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE_SOFT);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = egui::CornerRadius::same(7);
        w.expansion = 0.0;
    }
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    v.text_edit_bg_color = Some(BG);
    v.window_corner_radius = egui::CornerRadius::same(12);
    v.window_stroke = Stroke::new(1.0, LINE);
    v.window_shadow = egui::epaint::Shadow {
        offset: [0, 12],
        blur: 32,
        spread: 0,
        color: Color32::from_black_alpha(115),
    };
    v.popup_shadow = v.window_shadow;
    ctx.set_theme(egui::Theme::Dark);
    ctx.set_visuals_of(egui::Theme::Dark, v);
    ctx.all_styles_mut(|s| {
        use egui::{FontId, TextStyle};
        s.text_styles
            .insert(TextStyle::Body, FontId::proportional(14.0));
        s.text_styles
            .insert(TextStyle::Button, FontId::proportional(13.5));
        s.text_styles
            .insert(TextStyle::Small, FontId::proportional(12.0));
        s.text_styles
            .insert(TextStyle::Monospace, FontId::monospace(13.0));
        s.text_styles
            .insert(TextStyle::Heading, FontId::proportional(24.0));
        s.spacing.interact_size.y = 32.0;
        s.spacing.button_padding = egui::vec2(14.0, 6.0);
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
    });
    ctx.set_fonts(fonts());
}

/// Ubuntu and JetBrains Mono when installed (egui ships Ubuntu Light and Hack),
/// plus fallbacks so ▽ ↺ ↻ − render.
fn fonts() -> FontDefinitions {
    let mut f = FontDefinitions::default();
    let first = [
        (
            "ubuntu",
            FontFamily::Proportional,
            &[
                "/usr/share/fonts/ubuntu/Ubuntu-R.ttf",
                "/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf",
            ][..],
        ),
        (
            "jetbrains",
            FontFamily::Monospace,
            &[
                "/usr/share/fonts/jetbrains-mono-fonts/JetBrainsMono-Regular.otf",
                "/usr/share/fonts/truetype/jetbrains-mono/JetBrainsMono-Regular.ttf",
            ][..],
        ),
    ];
    for (name, fam, paths) in first {
        if let Some(bytes) = paths.iter().find_map(|p| std::fs::read(p).ok()) {
            f.font_data
                .insert(name.to_string(), Arc::new(FontData::from_owned(bytes)));
            f.families
                .entry(fam)
                .or_default()
                .insert(0, name.to_string());
        }
    }
    let fallbacks = [
        (
            "dejavu",
            "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
        ),
        ("symbola", "/usr/share/fonts/gdouros-symbola/Symbola.ttf"),
    ];
    for (name, path) in fallbacks {
        if let Ok(bytes) = std::fs::read(path) {
            f.font_data
                .insert(name.to_string(), Arc::new(FontData::from_owned(bytes)));
            for fam in [FontFamily::Proportional, FontFamily::Monospace] {
                f.families.entry(fam).or_default().push(name.to_string());
            }
        }
    }
    f
}
