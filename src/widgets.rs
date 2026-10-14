//! Painted building blocks with the handoff's exact sizes: cards, buttons,
//! segmented controls, switches, sliders, list rows.

use crate::theme;
use eframe::egui;
use egui::{
    Align2, Color32, CornerRadius, FontId, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui,
    Vec2, pos2, vec2,
};

pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

pub fn prop(size: f32) -> FontId {
    FontId::proportional(size)
}

/// Card: PANEL fill, 1px border, radius 12, padding 18×20.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    card_with(ui, egui::Margin::symmetric(20, 18), theme::LINE_SOFT, add)
}

pub fn card_with<R>(
    ui: &mut Ui,
    margin: egui::Margin,
    border: Color32,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    egui::Frame::new()
        .fill(theme::PANEL)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(12)
        .inner_margin(margin)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

pub fn heading(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(15.0).color(theme::TEXT));
}

pub fn hint(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).size(12.5).color(theme::TEXT_DIM)
}

pub fn dim(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text.into()).size(size).color(theme::TEXT_DIM)
}

/// A label that wraps and keeps the given colour.
pub fn para(ui: &mut Ui, text: impl Into<String>, size: f32, color: Color32) {
    ui.add(egui::Label::new(RichText::new(text.into()).size(size).color(color)).wrap());
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Primary,
    Secondary,
    Danger,
    /// red fill, for the last step of a reset
    Destroy,
    /// a "listening" button: accent fill
    Active,
}

pub struct Button<'a> {
    text: &'a str,
    kind: Kind,
    enabled: bool,
    height: f32,
    width: Option<f32>,
    font: f32,
    /// greyed text but still clickable (the design's "dimmed" state)
    muted: bool,
}

pub fn button(text: &str, kind: Kind) -> Button<'_> {
    Button {
        text,
        kind,
        enabled: true,
        height: 32.0,
        width: None,
        font: 13.5,
        muted: false,
    }
}

impl Button<'_> {
    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }
    pub fn compact(mut self) -> Self {
        self.height = 30.0;
        self.font = 13.0;
        self
    }
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
    pub fn muted(mut self, m: bool) -> Self {
        self.muted = m;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let (mut fill, border, mut fg) = match self.kind {
            Kind::Primary => (theme::ACCENT, None, theme::ACCENT_TEXT),
            Kind::Secondary => (theme::HI, Some(theme::LINE), theme::TEXT),
            Kind::Danger => (
                Color32::TRANSPARENT,
                Some(theme::DANGER_LINE),
                theme::DANGER_TEXT,
            ),
            Kind::Destroy => (theme::ERR, None, Color32::from_rgb(0x2a, 0x0a, 0x06)),
            Kind::Active => (theme::ACCENT, Some(theme::LINE), theme::ACCENT_TEXT),
        };
        // a dimmed primary (nothing to save) keeps its place in a LINE fill
        if (!self.enabled || self.muted) && self.kind == Kind::Primary {
            fill = theme::LINE;
        }
        if self.muted || !self.enabled {
            fg = if self.kind == Kind::Primary {
                theme::TEXT_FAINT
            } else {
                theme::TEXT_EMPTY
            };
        }
        let galley = ui
            .painter()
            .layout_no_wrap(self.text.to_string(), prop(self.font), fg);
        let w = self.width.unwrap_or(galley.size().x + 28.0);
        let sense = if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, resp) = ui.allocate_exact_size(vec2(w, self.height), sense);
        let hovered = resp.hovered() && self.enabled;
        let fill = match (hovered, self.kind) {
            (true, Kind::Secondary) => theme::shade(theme::HI, 1.12),
            (true, Kind::Danger) => theme::DANGER_FILL,
            (true, Kind::Primary | Kind::Destroy | Kind::Active) => theme::shade(fill, 1.08),
            _ => fill,
        };
        let stroke = border.map(|c| Stroke::new(1.0, c)).unwrap_or(Stroke::NONE);
        ui.painter().rect(rect, 7, fill, stroke, StrokeKind::Inside);
        ui.painter()
            .galley(rect.center() - galley.size() / 2.0, galley, fg);
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        resp
    }
}

pub fn primary(ui: &mut Ui, text: &str) -> Response {
    button(text, Kind::Primary).show(ui)
}

pub fn secondary(ui: &mut Ui, text: &str) -> Response {
    button(text, Kind::Secondary).show(ui)
}

/// Accent text that acts as a link.
pub fn link(ui: &mut Ui, text: &str, size: f32) -> Response {
    let r = ui.add(
        egui::Label::new(RichText::new(text).size(size).color(theme::ACCENT)).sense(Sense::click()),
    );
    if r.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    r
}

pub struct Segmented<'a> {
    labels: &'a [String],
    selected: Option<usize>,
    pad: Vec2,
    font: f32,
    /// item width: None sizes each item to its text; Some fills the given width evenly
    fill_width: Option<f32>,
    track: Color32,
    mono_prefix: bool,
}

pub fn segmented(labels: &[String], selected: Option<usize>) -> Segmented<'_> {
    Segmented {
        labels,
        selected,
        pad: vec2(13.0, 5.0),
        font: 13.0,
        fill_width: None,
        track: theme::BG,
        mono_prefix: false,
    }
}

impl Segmented<'_> {
    pub fn pad(mut self, x: f32, y: f32) -> Self {
        self.pad = vec2(x, y);
        self
    }
    pub fn font(mut self, f: f32) -> Self {
        self.font = f;
        self
    }
    pub fn fill(mut self, width: f32) -> Self {
        self.fill_width = Some(width);
        self
    }
    pub fn track(mut self, c: Color32) -> Self {
        self.track = c;
        self
    }
    /// "0 Base": the part before the first space is drawn in bold mono
    pub fn mono_prefix(mut self) -> Self {
        self.mono_prefix = true;
        self
    }

    /// Returns the index clicked (also when it was already selected).
    pub fn show(self, ui: &mut Ui) -> Option<usize> {
        let painter = ui.painter().clone();
        let job = |text: &str, color: Color32| {
            let mut j = egui::text::LayoutJob::default();
            match text.split_once(' ').filter(|_| self.mono_prefix) {
                Some((n, rest)) => {
                    j.append(n, 0.0, egui::TextFormat::simple(mono(self.font), color));
                    j.append(rest, 6.0, egui::TextFormat::simple(prop(self.font), color));
                }
                None => j.append(text, 0.0, egui::TextFormat::simple(prop(self.font), color)),
            }
            j
        };
        let sizes: Vec<Vec2> = self
            .labels
            .iter()
            .map(|l| painter.layout_job(job(l, theme::TEXT)).size())
            .collect();
        let gap = 2.0;
        let inner_h = self.font + 4.0 + self.pad.y * 2.0;
        let n = self.labels.len().max(1) as f32;
        let widths: Vec<f32> = match self.fill_width {
            Some(total) => vec![((total - 8.0 - gap * (n - 1.0)) / n).max(10.0); self.labels.len()],
            None => sizes.iter().map(|s| s.x + self.pad.x * 2.0).collect(),
        };
        let total_w = widths.iter().sum::<f32>() + gap * (n - 1.0) + 8.0;
        let (rect, _) = ui.allocate_exact_size(vec2(total_w, inner_h + 8.0), Sense::hover());
        let p = ui.painter();
        p.rect(
            rect,
            8,
            self.track,
            Stroke::new(1.0, theme::LINE),
            StrokeKind::Inside,
        );
        let mut x = rect.min.x + 4.0;
        let mut out = None;
        for (i, label) in self.labels.iter().enumerate() {
            let r = Rect::from_min_size(pos2(x, rect.min.y + 4.0), vec2(widths[i], inner_h));
            let resp = ui.interact(r, ui.id().with(("seg", i, label)), Sense::click());
            let on = self.selected == Some(i);
            let fg = if on {
                theme::ACCENT_TEXT
            } else if resp.hovered() {
                theme::TEXT
            } else {
                theme::TEXT_SOFT
            };
            if on {
                p.rect_filled(r, 6, theme::ACCENT);
            }
            let g = p.layout_job(job(label, fg));
            p.galley(r.center() - g.size() / 2.0, g, fg);
            if resp.clicked() {
                out = Some(i);
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            x += widths[i] + gap;
        }
        out
    }
}

pub fn strings<T: ToString>(items: impl IntoIterator<Item = T>) -> Vec<String> {
    items.into_iter().map(|s| s.to_string()).collect()
}

/// 40×22 switch (34×20 small); returns true when clicked.
pub fn switch(ui: &mut Ui, on: bool, small: bool) -> bool {
    let size = if small {
        vec2(34.0, 20.0)
    } else {
        vec2(40.0, 22.0)
    };
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let t = ui.ctx().animate_bool_with_time(resp.id, on, 0.15);
    let p = ui.painter();
    let fill = lerp_color(theme::SWITCH_OFF, theme::ACCENT, t);
    p.rect_filled(rect, CornerRadius::same((size.y / 2.0) as u8), fill);
    let knob = if small { 14.0 } else { 16.0 };
    let x = rect.min.x + 3.0 + (size.x - 6.0 - knob) * t + knob / 2.0;
    p.circle_filled(pos2(x, rect.center().y), knob / 2.0, theme::TEXT);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

#[derive(Default)]
pub struct SliderResult {
    pub changed: bool,
    /// the hand let go (or a click without drag)
    pub released: bool,
}

/// 4px LINE track, ACCENT_DIM trailing fill and an 18px thumb. `log` maps the
/// position logarithmically, as the DPI slider needs.
pub fn slider(
    ui: &mut Ui,
    value: &mut f64,
    min: f64,
    max: f64,
    log: bool,
    width: f32,
) -> SliderResult {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 22.0), Sense::click_and_drag());
    let to_p = |v: f64| {
        if log {
            (v / min).ln() / (max / min).ln()
        } else {
            (v - min) / (max - min)
        }
    };
    let from_p = |p: f64| {
        if log {
            min * (max / min).powf(p)
        } else {
            min + p * (max - min)
        }
    };
    let mut out = SliderResult::default();
    if let Some(pos) = resp.interact_pointer_pos()
        && (resp.dragged() || resp.clicked() || resp.drag_started())
    {
        let p = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0) as f64;
        let v = from_p(p);
        if (v - *value).abs() > f64::EPSILON {
            *value = v;
            out.changed = true;
        }
    }
    if resp.drag_stopped() || resp.clicked() {
        out.released = true;
    }
    let t = to_p(*value).clamp(0.0, 1.0) as f32;
    let p = ui.painter();
    let track = Rect::from_min_max(
        pos2(rect.min.x, rect.center().y - 2.0),
        pos2(rect.max.x, rect.center().y + 2.0),
    );
    p.rect_filled(track, 2, theme::LINE);
    let mut fill = track;
    fill.set_width(track.width() * t);
    p.rect_filled(fill, 2, theme::ACCENT_DIM);
    let c = pos2(rect.min.x + rect.width() * t, rect.center().y);
    p.circle_filled(c + vec2(0.0, 1.0), 9.5, Color32::from_black_alpha(90));
    p.circle_filled(c, 9.0, theme::TEXT);
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    out
}

/// List row: padding 9×12, radius 8; selected = accent border, tint and accent text.
pub fn list_row(
    ui: &mut Ui,
    left: &str,
    right: &str,
    selected: bool,
    right_color: Option<Color32>,
) -> Response {
    list_row_sized(ui, left, right, selected, right_color, 14.0)
}

pub fn list_row_sized(
    ui: &mut Ui,
    left: &str,
    right: &str,
    selected: bool,
    right_color: Option<Color32>,
    size: f32,
) -> Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, size + 4.0 + 18.0), Sense::click());
    let p = ui.painter();
    let (fill, stroke) = if selected {
        (theme::ACCENT_TINT, Stroke::new(1.0, theme::ACCENT))
    } else if resp.hovered() {
        (theme::HOVER, Stroke::NONE)
    } else {
        (Color32::TRANSPARENT, Stroke::NONE)
    };
    p.rect(rect, 8, fill, stroke, StrokeKind::Inside);
    let fg = if selected { theme::ACCENT } else { theme::TEXT };
    p.text(
        rect.left_center() + vec2(12.0, 0.0),
        Align2::LEFT_CENTER,
        left,
        prop(size),
        fg,
    );
    if !right.is_empty() {
        p.text(
            rect.right_center() - vec2(12.0, 0.0),
            Align2::RIGHT_CENTER,
            right,
            prop(12.0),
            right_color.unwrap_or(theme::TEXT_DIM),
        );
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

pub fn paint_battery(p: &egui::Painter, r: Rect, percent: u8, color: Color32) {
    p.rect_stroke(r, 3, Stroke::new(1.5, color), StrokeKind::Inside);
    let inner = r.shrink(2.5);
    let w = inner.width() * percent.min(100) as f32 / 100.0;
    p.rect_filled(
        Rect::from_min_size(inner.min, vec2(w, inner.height())),
        1,
        color,
    );
}

pub fn divider(ui: &mut Ui) {
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
    ui.painter()
        .hline(r.x_range(), r.center().y, Stroke::new(1.0, theme::DIVIDER));
}

pub fn vline(ui: &mut Ui, h: f32, color: Color32) {
    let (r, _) = ui.allocate_exact_size(vec2(1.0, h), Sense::hover());
    ui.painter()
        .vline(r.center().x, r.y_range(), Stroke::new(1.0, color));
}

/// A settings row: title and one-line hint on the left, the control on the
/// right, padded 14 top and bottom with a divider under it unless `last`.
pub fn setting_row(ui: &mut Ui, title: &str, sub: &str, last: bool, control: impl FnOnce(&mut Ui)) {
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        let w = ui.available_width();
        ui.vertical(|ui| {
            ui.set_width((w - 260.0).max(160.0));
            ui.spacing_mut().item_spacing.y = 3.0;
            ui.label(RichText::new(title).size(14.0));
            if !sub.is_empty() {
                ui.add(egui::Label::new(hint(sub)).wrap());
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), control);
    });
    ui.add_space(14.0);
    if !last {
        divider(ui);
    }
}

/// Text field: BG fill, LINE border, radius 7, height 32.
pub fn text_field(
    ui: &mut Ui,
    text: &mut String,
    hint_text: &str,
    width: f32,
    monospace: bool,
) -> Response {
    let font = if monospace { mono(13.0) } else { prop(13.0) };
    egui::Frame::new()
        .fill(theme::BG)
        .stroke(Stroke::new(1.0, theme::LINE))
        .corner_radius(7)
        .inner_margin(egui::Margin::symmetric(11, 0))
        .show(ui, |ui| {
            ui.set_height(30.0);
            ui.add_sized(
                vec2(width - 24.0, 30.0),
                egui::TextEdit::singleline(text)
                    .hint_text(RichText::new(hint_text).color(theme::TEXT_FAINT))
                    .frame(egui::Frame::NONE)
                    .font(font)
                    .vertical_align(egui::Align::Center),
            )
        })
        .inner
}

/// Lays out `n` blocks in a grid like CSS `repeat(auto-fill, minmax(min, 1fr))`.
pub fn grid(ui: &mut Ui, n: usize, min_width: f32, gap: f32, add: impl FnMut(&mut Ui, usize)) {
    grid_with(ui, n, min_width, gap, usize::MAX, add);
}

/// Like `grid`, but never more than `n` columns: the blocks share the whole width.
pub fn columns(ui: &mut Ui, n: usize, min_width: f32, gap: f32, add: impl FnMut(&mut Ui, usize)) {
    grid_with(ui, n, min_width, gap, n, add);
}

fn grid_with(
    ui: &mut Ui,
    n: usize,
    min_width: f32,
    gap: f32,
    max_cols: usize,
    mut add: impl FnMut(&mut Ui, usize),
) {
    let avail = ui.available_width();
    let per_row = (((avail + gap) / (min_width + gap)).floor() as usize).clamp(1, max_cols.max(1));
    let width = (avail - gap * (per_row as f32 - 1.0)) / per_row as f32;
    let mut i = 0;
    while i < n {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for _ in 0..per_row {
                if i >= n {
                    break;
                }
                ui.vertical(|ui| {
                    ui.set_width(width);
                    add(ui, i);
                });
                i += 1;
            }
        });
        if i < n {
            ui.add_space(gap);
        }
    }
}

/// Two columns with fixed widths (a negative width takes what is left); stacked when narrow.
pub fn two(
    ui: &mut Ui,
    left: f32,
    right: f32,
    gap: f32,
    stack_below: f32,
    mut add: impl FnMut(&mut Ui, usize),
) {
    let avail = ui.available_width();
    if avail < stack_below {
        add(ui, 0);
        ui.add_space(gap);
        add(ui, 1);
        return;
    }
    let (l, r) = match (left < 0.0, right < 0.0) {
        (true, _) => (avail - right - gap, right),
        (_, true) => (left, avail - left - gap),
        _ => (left, right),
    };
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        ui.vertical(|ui| {
            ui.set_width(l);
            add(ui, 0);
        });
        ui.vertical(|ui| {
            ui.set_width(r);
            add(ui, 1);
        });
    });
}

/// A dropdown in the secondary-button look.
pub fn select(
    ui: &mut Ui,
    id: &str,
    current: &str,
    options: &[String],
    width: f32,
) -> Option<usize> {
    let mut pick = None;
    ui.scope(|ui| {
        let v = ui.visuals_mut();
        v.widgets.inactive.weak_bg_fill = theme::HI;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, theme::LINE);
        v.widgets.hovered.weak_bg_fill = theme::shade(theme::HI, 1.12);
        egui::ComboBox::from_id_salt(id)
            .selected_text(RichText::new(current).size(13.0))
            .width(width)
            .height(320.0)
            .show_ui(ui, |ui| {
                for (i, o) in options.iter().enumerate() {
                    if ui.selectable_label(o == current, o).clicked() {
                        pick = Some(i);
                    }
                }
            });
    });
    pick
}

/// Accent ring for a selected swatch: a 2px gap in `gap_color`, then a 2px TEXT ring.
pub fn swatch_ring(p: &egui::Painter, rect: Rect, radius: f32, gap_color: Color32) {
    let r = CornerRadius::same(radius as u8);
    p.rect_stroke(rect, r, Stroke::new(2.0, gap_color), StrokeKind::Outside);
    p.rect_stroke(
        rect.expand(2.0),
        CornerRadius::same((radius + 2.0) as u8),
        Stroke::new(2.0, theme::TEXT),
        StrokeKind::Outside,
    );
}
