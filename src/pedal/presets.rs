//! All 36 presets in a 9 × 4 grid, and backup / restore.

use super::preset::{self, BLOCKS};
use super::worker::Command;
use super::{Confirm, Pedal};
use crate::{theme, widgets};
use eframe::egui;
use egui::{Align2, Sense, Stroke, StrokeKind, pos2, vec2};

pub fn page(p: &mut Pedal, ui: &mut egui::Ui) {
    widgets::two(ui, -1.0, 280.0, 16.0, 720.0, |ui, i| {
        if i == 0 {
            grid(p, ui);
        } else {
            backup(p, ui);
        }
    });
}

fn grid(p: &mut Pedal, ui: &mut egui::Ui) {
    let sounding = p.sounding();
    widgets::card(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.label(widgets::dim(
            "Click a preset to play it. The footswitches step through the same grid.",
            13.0,
        ));
        ui.add_space(8.0);
        let w = ui.available_width();
        let cell_w = (w - 30.0 - 6.0 * 9.0) / 9.0;
        let (head, _) = ui.allocate_exact_size(vec2(w, 16.0), Sense::hover());
        for bank in 0..9 {
            let x = head.min.x + 36.0 + bank as f32 * (cell_w + 6.0) + cell_w / 2.0;
            ui.painter().text(
                pos2(x, head.center().y),
                Align2::CENTER_CENTER,
                (bank + 1).to_string(),
                widgets::prop(12.0),
                theme::TEXT_DIM,
            );
        }
        for letter in 0..4 {
            let (row, _) = ui.allocate_exact_size(vec2(w, 58.0), Sense::hover());
            ui.painter().text(
                pos2(row.min.x, row.center().y),
                Align2::LEFT_CENTER,
                ["A", "B", "C", "D"][letter],
                widgets::prop(12.0),
                theme::TEXT_DIM,
            );
            for bank in 0..9 {
                let slot = bank * 4 + letter;
                let r = egui::Rect::from_min_size(
                    pos2(row.min.x + 36.0 + bank as f32 * (cell_w + 6.0), row.min.y),
                    vec2(cell_w, 58.0),
                );
                let resp = ui.interact(r, ui.id().with(("preset", slot)), Sense::click());
                let here = sounding == Some(slot);
                let pre = if here {
                    p.live.or_else(|| p.stored(slot))
                } else {
                    p.stored(slot)
                };
                let pt = ui.painter();
                let border = if here {
                    theme::ACCENT
                } else if resp.hovered() {
                    theme::HOVER_BORDER
                } else {
                    theme::LINE
                };
                pt.rect(
                    r,
                    8,
                    if here {
                        theme::ACCENT_TINT
                    } else {
                        theme::CARD_ALT
                    },
                    Stroke::new(1.0, border),
                    StrokeKind::Inside,
                );
                pt.text(
                    pos2(r.min.x + 8.0, r.min.y + 15.0),
                    Align2::LEFT_CENTER,
                    preset::name(slot),
                    widgets::mono(13.0),
                    if here { theme::ACCENT } else { theme::TEXT },
                );
                if let Some(pre) = pre {
                    for (k, b) in BLOCKS.iter().enumerate() {
                        let c = if !b.is_on(&pre) {
                            theme::SWITCH_OFF
                        } else if here {
                            theme::ACCENT
                        } else {
                            theme::TEXT_SOFT
                        };
                        pt.circle_filled(
                            pos2(r.min.x + 10.5 + k as f32 * 8.0, r.max.y - 11.0),
                            2.5,
                            c,
                        );
                    }
                }
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    p.send(Command::Load(slot));
                }
            }
        }
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            ui.label(widgets::dim("Dots show which blocks are on:", 12.0));
            ui.label(widgets::dim(
                "gate · amp · mod · delay · reverb · cab",
                12.0,
            ));
        });
    });
}

fn backup(p: &mut Pedal, ui: &mut egui::Ui) {
    widgets::card(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        widgets::heading(ui, "Backup");
        ui.add_space(6.0);
        widgets::para(
            ui,
            "All 36 presets in one file. A copy is also kept before the first save of every session.",
            12.5,
            theme::TEXT_DIM,
        );
        ui.add_space(14.0);
        let w = ui.available_width();
        let dir = Pedal::backup_dir();
        if widgets::button("Back up all presets…", widgets::Kind::Secondary)
            .width(w)
            .show(ui)
            .clicked()
        {
            let _ = std::fs::create_dir_all(&dir);
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Back up the Tank-G presets")
                .set_directory(&dir)
                .set_file_name(format!("tank-g-presets-{}.bin", Pedal::stamp()))
                .add_filter("Tank-G backup", &["bin"])
                .save_file()
            {
                p.send(Command::BackupTo(path));
            }
        }
        ui.add_space(8.0);
        if widgets::button("Restore from a backup…", widgets::Kind::Secondary)
            .width(w)
            .show(ui)
            .clicked()
            && let Some(path) = rfd::FileDialog::new()
                .set_title("Restore Tank-G presets")
                .set_directory(&dir)
                .add_filter("Tank-G backup", &["bin"])
                .pick_file()
        {
            restore(p, &path);
        }
    });
}

fn restore(p: &mut Pedal, path: &std::path::Path) {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let table = match std::fs::read(path) {
        Ok(t) if preset::table_ok(&t) => t,
        Ok(_) => {
            p.notes.push((
                format!(
                    "{name} is not a Tank-G backup ({} bytes starting with A5 5A)",
                    preset::TABLE_SIZE
                ),
                false,
            ));
            return;
        }
        Err(e) => {
            p.notes.push((format!("{name}: {e}"), false));
            return;
        }
    };
    let slots: Vec<usize> = (0..preset::COUNT)
        .filter(|s| Some(preset::from_table(&table, *s)) != p.stored(*s))
        .collect();
    if slots.is_empty() {
        p.notes
            .push((format!("{name} matches what is on the pedal"), true));
        return;
    }
    p.confirm = Some(Confirm::Restore { name, table, slots });
}
