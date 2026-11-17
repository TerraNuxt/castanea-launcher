//! Small controls: the switch, the segmented control, chips, the search field and section titles.

use eframe::egui::{self, Color32, Response, RichText, Role, Sense, WidgetInfo};

use super::button::{focus_ring, hover_press};
use crate::icons;
use crate::theme::{self, anim, color, font, radius, size, space};

/// An on/off switch with an animated knob and its label on the right.
pub fn switch(ui: &mut egui::Ui, on: &mut bool, label: &str) -> Response {
    switch_inner(ui, on, label, true)
}

/// A switch without visible text; `name` is for screen readers and the tooltip.
pub fn switch_named(ui: &mut egui::Ui, on: &mut bool, name: &str) -> Response {
    switch_inner(ui, on, name, false).on_hover_text(name)
}

fn switch_inner(ui: &mut egui::Ui, on: &mut bool, label: &str, visible: bool) -> Response {
    let track = egui::vec2(40.0, 22.0);
    let text = (visible && !label.is_empty()).then(|| {
        ui.painter().layout(label.to_owned(), font::regular(size::BODY), color::TEXT, ui.available_width() - 60.0)
    });
    let width = track.x + text.as_ref().map_or(0.0, |g| space::S + g.size().x);
    let height = track.y.max(text.as_ref().map_or(0.0, |g| g.size().y));
    let (rect, mut response) = ui.allocate_exact_size(egui::vec2(width, height), Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let t = ui.ctx().animate_bool_with_time(response.id, *on, anim::HOVER);
    let (hover, _) = hover_press(ui, &response);
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let track_rect = egui::Rect::from_min_size(egui::pos2(rect.left(), rect.center().y - track.y / 2.0), track);
        let fill = theme::mix(color::SURFACE_ACTIVE, color::ACCENT, t);
        painter.rect_filled(track_rect, theme::radius(radius::PILL), theme::lighten(fill, hover * 0.08));
        let knob_x = egui::lerp(track_rect.left() + 11.0..=track_rect.right() - 11.0, t);
        painter.circle_filled(egui::pos2(knob_x, track_rect.center().y), 8.0, color::TEXT);
        if let Some(galley) = text {
            let pos = egui::pos2(track_rect.right() + space::S, rect.center().y - galley.size().y / 2.0);
            painter.galley(pos, galley, color::TEXT);
        }
        focus_ring(ui, &response, track_rect, radius::PILL);
    }
    let (state, name) = (*on, label.to_owned());
    response.widget_info(|| WidgetInfo::selected(Role::CheckBox, true, state, &name));
    response
}

/// One pill per choice; returns true when the choice changed.
pub fn segmented(ui: &mut egui::Ui, current: &mut usize, choices: &[&str]) -> bool {
    // Measured first and allocated as one block, so it lays out the same in any parent direction.
    let (pad, gap, height) = (3.0, 2.0, 28.0);
    let galleys: Vec<_> = choices
        .iter()
        .map(|c| ui.painter().layout_no_wrap((*c).to_owned(), font::semibold(size::SMALL), color::TEXT))
        .collect();
    let widths: Vec<f32> = galleys.iter().map(|g| g.size().x + space::L * 2.0).collect();
    let total = widths.iter().sum::<f32>() + gap * (widths.len().saturating_sub(1)) as f32 + pad * 2.0;
    let (outer, block) = ui.allocate_exact_size(egui::vec2(total, height + pad * 2.0), Sense::hover());
    ui.painter().rect_filled(outer, theme::radius(radius::M), color::SUNKEN);
    let mut changed = false;
    let mut x = outer.left() + pad;
    for (n, (galley, width)) in galleys.into_iter().zip(widths).enumerate() {
        let rect = egui::Rect::from_min_size(egui::pos2(x, outer.top() + pad), egui::vec2(width, height));
        x += width + gap;
        let response = ui.interact(rect, block.id.with(n), Sense::click());
        let selected = *current == n;
        let (hover, _) = hover_press(ui, &response);
        let t = ui.ctx().animate_bool_with_time(response.id.with("sel"), selected, anim::HOVER);
        let fill = theme::mix(theme::mix(Color32::TRANSPARENT, color::SURFACE_HOVER, hover), color::SURFACE_ACTIVE, t);
        ui.painter().rect_filled(rect, theme::radius(radius::S + 1), fill);
        let tint = theme::mix(color::TEXT_MUTED, color::TEXT, t.max(hover));
        ui.painter().galley(rect.center() - galley.size() / 2.0, galley, tint);
        focus_ring(ui, &response, rect, radius::S);
        if response.clicked() && !selected {
            *current = n;
            changed = true;
        }
        let label = choices[n].to_owned();
        response.widget_info(|| WidgetInfo::selected(Role::RadioButton, true, selected, &label));
    }
    changed
}

/// A small rounded label tinted with `tint`.
pub fn chip(ui: &mut egui::Ui, text: &str, tint: Color32) -> Response {
    egui::Frame::new()
        .fill(tint.gamma_multiply(0.16))
        .corner_radius(theme::radius(radius::PILL))
        .inner_margin(egui::Margin::symmetric(8, 2))
        .show(ui, |ui| ui.label(RichText::new(text).size(size::CAPTION).color(tint)))
        .response
}

/// A text field with a search icon inside a sunken pill.
pub fn search_field(ui: &mut egui::Ui, text: &mut String, hint: &str, width: f32) -> Response {
    egui::Frame::new()
        .fill(color::SUNKEN)
        .stroke(egui::Stroke::new(1.0, color::BORDER))
        .corner_radius(theme::radius(radius::M))
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(super::icon(icons::SEARCH, 16.0, color::TEXT_MUTED));
                let edit = egui::TextEdit::singleline(text)
                    .hint_text(hint)
                    .frame(egui::Frame::NONE)
                    .desired_width(width - 40.0)
                    .font(font::regular(size::BODY));
                ui.add(edit)
            })
            .inner
        })
        .inner
}

/// A section title inside a page or card.
pub fn section_title(ui: &mut egui::Ui, glyph: &str, title: &str) {
    ui.horizontal(|ui| {
        ui.label(super::icon(glyph, 18.0, color::TEXT_MUTED));
        ui.label(RichText::new(title).font(font::semibold(size::TITLE)).color(color::TEXT));
    });
}
