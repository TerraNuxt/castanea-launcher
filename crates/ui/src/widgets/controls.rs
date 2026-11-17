//! Small controls: the switch, the segmented control, text fields and tags.

use eframe::egui::{self, Color32, Response, RichText, Role, Sense, StrokeKind, WidgetInfo};

use super::button::{focus_ring, hover_press};
use crate::icons;
use crate::theme::{self, anim, color, font, layout, line, radius, size, space};

/// An on/off switch with its label on the right.
pub fn switch(ui: &mut egui::Ui, on: &mut bool, label: &str) -> Response {
    switch_inner(ui, on, label, true)
}

/// A switch without visible text; `name` is for screen readers and the tooltip.
pub fn switch_named(ui: &mut egui::Ui, on: &mut bool, name: &str) -> Response {
    switch_inner(ui, on, name, false).on_hover_text(name)
}

fn switch_inner(ui: &mut egui::Ui, on: &mut bool, label: &str, visible: bool) -> Response {
    let track = egui::vec2(layout::SWITCH[0], layout::SWITCH[1]);
    let text = (visible && !label.is_empty()).then(|| {
        let wrap = (ui.available_width() - track.x - space::S).max(layout::FIELD / 4.0);
        ui.painter().layout(label.to_owned(), font::regular(size::BODY), color::TEXT, wrap)
    });
    let width = track.x + text.as_ref().map_or(0.0, |g| space::S + g.size().x);
    let height = layout::CONTROL.max(text.as_ref().map_or(0.0, |g| g.size().y));
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
        let off = theme::mix(color::RAISED, color::HOVER, hover);
        painter.rect_filled(track_rect, theme::radius(radius::FULL), theme::mix(off, color::ACCENT, t));
        let outline = theme::mix(color::BORDER_STRONG, color::ACCENT, t);
        painter.rect_stroke(
            track_rect,
            theme::radius(radius::FULL),
            egui::Stroke::new(line::HAIRLINE, outline),
            StrokeKind::Inside,
        );
        let knob = track.y / 2.0 - layout::KNOB_INSET;
        let x = egui::lerp(
            track_rect.left() + knob + layout::KNOB_INSET..=track_rect.right() - knob - layout::KNOB_INSET,
            t,
        );
        let knob_color = theme::mix(color::TEXT_MUTED, color::ON_ACCENT, t);
        painter.circle_filled(egui::pos2(x, track_rect.center().y), knob, knob_color);
        if let Some(galley) = text {
            let pos = egui::pos2(track_rect.right() + space::S, rect.center().y - galley.size().y / 2.0);
            painter.galley(pos, galley, color::TEXT);
        }
        focus_ring(ui, &response, track_rect, radius::FULL);
    }
    let (state, name) = (*on, label.to_owned());
    response.widget_info(|| WidgetInfo::selected(Role::CheckBox, true, state, &name));
    response
}

/// One part per choice inside a hairline outline; returns true when the choice changed.
pub fn segmented(ui: &mut egui::Ui, current: &mut usize, choices: &[&str]) -> bool {
    let height = layout::CONTROL;
    let galleys: Vec<_> = choices
        .iter()
        .map(|c| ui.painter().layout_no_wrap((*c).to_owned(), font::regular(size::BODY), Color32::PLACEHOLDER))
        .collect();
    let widths: Vec<f32> = galleys.iter().map(|g| g.size().x + space::L * 2.0).collect();
    let (outer, block) = ui.allocate_exact_size(egui::vec2(widths.iter().sum(), height), Sense::hover());
    let mut changed = false;
    let mut x = outer.left();
    for (n, (galley, width)) in galleys.into_iter().zip(widths).enumerate() {
        let rect = egui::Rect::from_min_size(egui::pos2(x, outer.top()), egui::vec2(width, height));
        x += width;
        let response = ui.interact(rect, block.id.with(n), Sense::click());
        let selected = *current == n;
        let (hover, _) = hover_press(ui, &response);
        let t = ui.ctx().animate_bool_with_time(response.id.with("sel"), selected, anim::HOVER);
        let fill = theme::mix(theme::mix(Color32::TRANSPARENT, color::HOVER, hover), color::RAISED, t);
        ui.painter().rect_filled(rect.shrink(layout::INSET), theme::radius(radius::S), fill);
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
    ui.painter().rect_stroke(outer, theme::radius(radius::M), theme::hairline(), StrokeKind::Inside);
    changed
}

/// A one-line text field `width` wide; `name` is for screen readers when there is no visible label next to it.
pub fn text_field(ui: &mut egui::Ui, text: &mut String, hint: &str, width: f32) -> Response {
    let edit = egui::TextEdit::singleline(text)
        .hint_text(RichText::new(hint).color(color::TEXT_MUTED))
        .desired_width(width)
        .min_size(egui::vec2(width, layout::CONTROL))
        .align(egui::Align2::LEFT_CENTER)
        .margin(egui::Margin::symmetric(space::S as i8, 0))
        .font(font::regular(size::BODY));
    ui.add(edit)
}

/// A text field with a search icon inside.
pub fn search_field(ui: &mut egui::Ui, text: &mut String, hint: &str, width: f32) -> Response {
    let inner = egui::Frame::new()
        .fill(color::BASE)
        .corner_radius(theme::radius(radius::M))
        .inner_margin(egui::Margin::symmetric(space::S as i8, 0))
        .show(ui, |ui| {
            ui.set_height(layout::CONTROL);
            ui.horizontal_centered(|ui| {
                ui.label(super::icon(icons::SEARCH, color::TEXT_MUTED));
                let edit = egui::TextEdit::singleline(text)
                    .hint_text(RichText::new(hint).color(color::TEXT_MUTED))
                    .frame(egui::Frame::NONE)
                    .align(egui::Align2::LEFT_CENTER)
                    .min_size(egui::vec2(0.0, layout::CONTROL))
                    .desired_width(width - size::ICON - space::S * 3.0)
                    .font(font::regular(size::BODY));
                ui.add(edit)
            })
            .inner
        });
    let focused = inner.inner.has_focus();
    let stroke = if focused { color::ACCENT } else { color::BORDER_STRONG };
    ui.painter().rect_stroke(
        inner.response.rect,
        theme::radius(radius::M),
        egui::Stroke::new(line::HAIRLINE, stroke),
        StrokeKind::Inside,
    );
    inner.inner
}

/// A small label in a hairline outline, such as a mod's tag; `tint` colours only the text.
pub fn tag(ui: &mut egui::Ui, text: &str, tint: Color32) -> Response {
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font::regular(size::SMALL), tint);
    let size = galley.size() + egui::vec2(space::S, layout::LINE_GAP * 2.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_stroke(rect, theme::radius(radius::S), theme::hairline(), StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, tint);
    response
}
