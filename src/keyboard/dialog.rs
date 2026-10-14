//! The design's modal: 500px card over a dark overlay, Cancel and one action.

use crate::{theme, widgets};
use eframe::egui;
use egui::{Color32, RichText, Stroke};

/// Some(true) when the action was pressed, Some(false) on Cancel or Esc.
pub fn modal(
    ctx: &egui::Context,
    id: &str,
    title: &str,
    paragraphs: &[&str],
    verb: &str,
    destructive: bool,
) -> Option<bool> {
    let mut out = None;
    let m = egui::Modal::new(egui::Id::new(id))
        .backdrop_color(Color32::from_rgba_unmultiplied(10, 10, 12, 158))
        .frame(
            egui::Frame::new()
                .fill(theme::CARD_ALT)
                .stroke(Stroke::new(1.0, theme::LINE))
                .corner_radius(14)
                .inner_margin(egui::Margin::symmetric(24, 22))
                .shadow(egui::epaint::Shadow {
                    offset: [0, 24],
                    blur: 60,
                    spread: 0,
                    color: Color32::from_black_alpha(128),
                }),
        )
        .show(ctx, |ui| {
            ui.set_width(452.0);
            ui.spacing_mut().item_spacing.y = 10.0;
            ui.label(RichText::new(title).size(17.0).color(theme::TEXT));
            ui.add_space(2.0);
            for p in paragraphs {
                widgets::para(ui, *p, 13.5, theme::TEXT_SOFT);
            }
            ui.add_space(10.0);
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 34.0),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    let kind = if destructive {
                        widgets::Kind::Destroy
                    } else {
                        widgets::Kind::Primary
                    };
                    if widgets::button(verb, kind).show(ui).clicked() {
                        out = Some(true);
                    }
                    if widgets::secondary(ui, "Cancel").clicked() {
                        out = Some(false);
                    }
                },
            );
        });
    if out.is_none() && m.should_close() {
        out = Some(false);
    }
    out
}
