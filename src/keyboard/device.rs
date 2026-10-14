//! Device: Windows / Mac takeover, layout files, copy a layer, info and the
//! dangerous buttons, plus the confirmations they open.

use super::{Confirm, Keyboard, keymap};
use crate::{theme, widgets};
use eframe::egui;
use egui::{RichText, vec2};
use hidra::modes::{self, Mode};
use hidra::session::Backup;

pub fn page(k: &mut Keyboard, ui: &mut egui::Ui) {
    widgets::grid(ui, 4, 340.0, 16.0, |ui, i| match i {
        0 => mode(k, ui),
        1 => layout_file(k, ui),
        2 => copy_layer(k, ui),
        _ => info(k, ui),
    });
}

fn mode(k: &mut Keyboard, ui: &mut egui::Ui) {
    widgets::card(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        widgets::heading(ui, "Windows / Mac layout");
        if k.mode_file.is_some() {
            ui.add_space(14.0);
            let cur = k.mode();
            let labels = widgets::strings(["Windows", "Mac"]);
            let sel = cur.map(|m| if m == Mode::Mac { 1 } else { 0 });
            if let Some(i) = widgets::segmented(&labels, sel)
                .pad(18.0, 7.0)
                .font(13.5)
                .show(ui)
            {
                let m = if i == 1 { Mode::Mac } else { Mode::Windows };
                if Some(m) != cur {
                    if k.with_session(|s| modes::switch(s, m)).is_some() {
                        k.ok(format!("Switched to the {} layout", m.name()));
                    }
                    k.mode_file = k.session.as_ref().and_then(modes::load);
                }
            }
            ui.add_space(12.0);
            if cur.is_none() {
                widgets::para(
                    ui,
                    "A switch stopped halfway. Pick a layout to finish it.",
                    12.5,
                    theme::ERR,
                );
            } else {
                widgets::para(
                    ui,
                    "Switching writes the chosen layout into layer 0, so the mode is always what you see here.",
                    12.5,
                    theme::TEXT_DIM,
                );
            }
            ui.add_space(10.0);
            if widgets::link(ui, "Give the switch back to the keyboard", 12.5).clicked() {
                k.confirm = Some(Confirm::ModeOff);
            }
            return;
        }
        ui.add_space(8.0);
        let plan = k
            .session
            .as_ref()
            .map(|s| modes::plan(s).is_ok())
            .unwrap_or(false);
        if !plan {
            widgets::para(
                ui,
                "No Windows / Mac switch keys were found in this keymap.",
                13.0,
                theme::TEXT_DIM,
            );
            return;
        }
        widgets::para(
            ui,
            "The keyboard flips to its Mac layout with Fn+S and back with Fn+A, and it can't tell anyone which one it's in.",
            13.0,
            theme::TEXT_DIM,
        );
        ui.add_space(14.0);
        if widgets::primary(ui, "Let octo hub switch it…").clicked() {
            k.confirm = Some(Confirm::ModeSetup);
        }
    });
}

fn layout_file(k: &mut Keyboard, ui: &mut egui::Ui) {
    widgets::card(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        widgets::heading(ui, "Layout file");
        ui.add_space(6.0);
        widgets::para(
            ui,
            "Same format as VIA, so files move freely between the two.",
            12.5,
            theme::TEXT_DIM,
        );
        ui.add_space(14.0);
        ui.horizontal_wrapped(|ui| {
            if widgets::secondary(ui, "Save layout file…").clicked() {
                let name = k
                    .session
                    .as_ref()
                    .map(|s| s.def.name.clone())
                    .unwrap_or_default();
                let suggested = format!(
                    "{}.layout.json",
                    name.chars()
                        .map(|c| if c.is_ascii_alphanumeric() {
                            c.to_ascii_lowercase()
                        } else {
                            '_'
                        })
                        .collect::<String>()
                );
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name(suggested)
                    .add_filter("layout", &["json"])
                    .save_file()
                    && let Some(s) = &k.session
                {
                    match s.to_backup().save(&path) {
                        Ok(()) => k.ok(format!(
                            "Saved {}",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        )),
                        Err(e) => k.notes.push((format!("{e:#}"), false)),
                    }
                }
            }
            if widgets::secondary(ui, "Load layout file…").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("layout", &["json"])
                    .pick_file()
            {
                k.confirm = Some(Confirm::Restore(path));
            }
        });
    });
}

fn copy_layer(k: &mut Keyboard, ui: &mut egui::Ui) {
    let names = keymap::layer_names(k);
    widgets::card(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        widgets::heading(ui, "Copy a layer");
        ui.add_space(14.0);
        ui.horizontal_wrapped(|ui| {
            let (mut from, mut to) = k.layer_copy;
            let label = |i: usize| names.get(i).cloned().unwrap_or_else(|| i.to_string());
            if let Some(i) = widgets::select(ui, "copy-from", &label(from), &names, 90.0) {
                from = i;
            }
            ui.label(widgets::dim("over", 13.5));
            if let Some(i) = widgets::select(ui, "copy-to", &label(to), &names, 90.0) {
                to = i;
            }
            k.layer_copy = (from, to);
            if widgets::button("Copy", widgets::Kind::Secondary)
                .enabled(from != to)
                .show(ui)
                .clicked()
            {
                k.confirm = Some(Confirm::CopyLayer(from, to));
            }
        });
    });
}

fn info(k: &mut Keyboard, ui: &mut egui::Ui) {
    let Some(s) = &k.session else { return };
    let rows = [
        (
            "Definition",
            s.def
                .path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
        ),
        ("VIA protocol", s.kb.protocol.to_string()),
        ("Matrix", format!("{} × {}", s.def.rows, s.def.cols)),
        ("Layers", s.layers.to_string()),
        ("Encoders", s.def.encoder_count().to_string()),
        (
            "Macros",
            format!("{} slots, {} bytes", s.macro_count, s.macro_buffer_size),
        ),
    ];
    widgets::card_with(ui, egui::Margin::symmetric(20, 6), theme::LINE_SOFT, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (key, v) in rows {
            ui.add_space(9.0);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                let (r, _) = ui.allocate_exact_size(vec2(110.0, 16.0), egui::Sense::hover());
                ui.painter().text(
                    r.left_center(),
                    egui::Align2::LEFT_CENTER,
                    key,
                    widgets::prop(13.0),
                    theme::TEXT_DIM,
                );
                ui.add(egui::Label::new(RichText::new(v).font(widgets::mono(12.5))).wrap());
            });
            ui.add_space(9.0);
            let (r, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), 1.0), egui::Sense::hover());
            ui.painter().hline(
                r.x_range(),
                r.center().y,
                egui::Stroke::new(1.0, theme::HOVER),
            );
        }
        ui.add_space(14.0);
        ui.horizontal_wrapped(|ui| {
            if widgets::button("Blink lights", widgets::Kind::Secondary)
                .compact()
                .show(ui)
                .clicked()
                && k.with_session(|s| s.kb.indicate()).is_some()
            {
                k.ok("Blinking the lights");
            }
            if widgets::button("Reset keymap…", widgets::Kind::Danger)
                .compact()
                .show(ui)
                .clicked()
            {
                k.confirm = Some(Confirm::ResetKeymap);
            }
            if widgets::button("Bootloader…", widgets::Kind::Danger)
                .compact()
                .show(ui)
                .clicked()
            {
                k.confirm = Some(Confirm::Bootloader);
            }
        });
        ui.add_space(14.0);
    });
}

pub fn confirm(k: &mut Keyboard, ctx: &egui::Context, c: Confirm) {
    let restore_text;
    let (title, text, verb, destructive): (&str, Vec<&str>, &str, bool) = match &c {
        Confirm::ModeSetup => (
            "Let octo hub switch Windows / Mac?",
            vec![
                "The keyboard's own switch keys (Fn+S and Fn+A) are turned off so the mode can't change by accident. Both layouts are kept here, and switching writes the one you pick into layer 0.",
                "If it's in Mac mode right now, press Fn+A first.",
            ],
            "Take over",
            false,
        ),
        Confirm::ModeOff => (
            "Give the switch back?",
            vec!["Layer 0 gets the Windows layout and Fn+S / Fn+A work again."],
            "Give back",
            false,
        ),
        Confirm::Restore(p) => {
            restore_text = format!(
                "Overwrite the keymap, knob, macros and lighting on the keyboard with {}?",
                p.display()
            );
            (
                "Load layout file?",
                vec![restore_text.as_str()],
                "Overwrite",
                false,
            )
        }
        Confirm::ResetKeymap => (
            "Reset the keymap?",
            vec![
                "Every layer goes back to the firmware's default. Your changes are lost unless you saved a layout file.",
            ],
            "Reset keymap",
            true,
        ),
        Confirm::Bootloader => (
            "Jump to the bootloader?",
            vec!["The keyboard stops typing until it is flashed or plugged in again."],
            "Reboot",
            true,
        ),
        Confirm::CopyLayer(a, b) => (
            "Copy a layer?",
            vec!["The target layer is replaced, knob included."],
            if a == b { "Copy" } else { "Copy" },
            false,
        ),
    };
    match super::dialog::modal(ctx, "kb-confirm", title, &text, verb, destructive) {
        Some(true) => {
            k.confirm = None;
            perform(k, c);
        }
        Some(false) => k.confirm = None,
        None => {}
    }
}

fn perform(k: &mut Keyboard, c: Confirm) {
    match c {
        Confirm::ModeSetup => {
            if k.with_session(modes::setup).is_some() {
                k.ok("octo hub now switches the layout");
            }
            k.mode_file = k.session.as_ref().and_then(modes::load);
        }
        Confirm::ModeOff => {
            if k.with_session(modes::teardown).is_some() {
                k.ok("Switch keys restored on the keyboard");
            }
            k.mode_file = k.session.as_ref().and_then(modes::load);
        }
        Confirm::Restore(path) => match Backup::load(&path) {
            Ok(b) => {
                if k.with_session(|s| s.restore(&b)).is_some() {
                    k.macro_loaded = false;
                    k.ok("Loaded the layout file");
                }
            }
            Err(e) => k.notes.push((format!("{e:#}"), false)),
        },
        Confirm::ResetKeymap => {
            if k.with_session(|s| {
                s.kb.reset_keymap()?;
                s.reload()
            })
            .is_some()
            {
                k.ok("Keymap reset to the default");
            }
        }
        Confirm::Bootloader => {
            if k.with_session(|s| s.kb.jump_to_bootloader()).is_some() {
                k.session = None;
                k.ok("The keyboard is in bootloader mode");
            }
        }
        Confirm::CopyLayer(a, b) => {
            if k.with_session(|s| {
                let mut km = s.keymap.clone();
                km[b] = km[a].clone();
                s.kb.write_keymap(&km)?;
                s.keymap = km;
                for e in 0..s.def.encoder_count() {
                    let (ccw, cw) = s.encoders[a][e];
                    s.set_encoder(b, e, false, ccw)?;
                    s.set_encoder(b, e, true, cw)?;
                }
                Ok(())
            })
            .is_some()
            {
                k.ok(format!("Copied layer {a} over layer {b}"));
            }
        }
    }
}
