//! Page structure: the page title, section titles, settings rows with a label column, hairlines, the Advanced
//! expander and the empty-list line.

use eframe::egui::{self, Response, Role, Sense, WidgetInfo};

use super::button::{focus_ring, hover_press};
use crate::theme::{self, color, layout, line, radius, size, space};

/// The page title with optional actions on the right.
pub fn page_header<R>(ui: &mut egui::Ui, title: &str, actions: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let inner = ui
        .allocate_ui_with_layout(
            egui::vec2(ui.available_width(), layout::CONTROL),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.label(super::strong(title, size::TITLE));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), actions).inner
            },
        )
        .inner;
    ui.add_space(space::L);
    inner
}

/// A 1 px line across the available width.
pub fn hairline(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), line::HAIRLINE), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, color::BORDER);
}

/// A section title with space above it and a hairline below.
pub fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(space::XL);
    ui.label(super::strong(title, size::BODY));
    ui.add_space(space::S);
    hairline(ui);
}

/// One settings row: the label (and a hint below it) in the left column, the control on the right, a hairline below.
pub fn setting_row<R>(
    ui: &mut egui::Ui,
    label: impl Into<String>,
    hint: Option<&str>,
    control: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.add_space(space::M);
    let inner = ui
        .horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(layout::LABEL, layout::CONTROL),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(layout::LABEL - space::L);
                    ui.spacing_mut().item_spacing.y = crate::theme::layout::LINE_GAP;
                    ui.add_space(crate::theme::layout::LABEL_NUDGE);
                    ui.label(super::text(label.into()));
                    if let Some(hint) = hint {
                        super::wrapped(ui, super::small(hint));
                    }
                },
            );
            ui.vertical(control).inner
        })
        .inner;
    ui.add_space(space::M);
    hairline(ui);
    inner
}

/// A row that opens and closes the part below it, with a drawn chevron; returns whether it is open now.
pub fn expander(ui: &mut egui::Ui, open: &mut bool, title: &str) -> Response {
    ui.add_space(space::XL);
    let (rect, mut response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width().min(layout::READABLE), layout::CONTROL), Sense::click());
    if response.clicked() {
        *open = !*open;
        response.mark_changed();
    }
    let (hover, _) = hover_press(ui, &response);
    let tint = theme::mix(color::TEXT_MUTED, color::TEXT, hover.max(if *open { 1.0 } else { 0.0 }));
    let painter = ui.painter();
    let c = egui::pos2(rect.left() + space::S, rect.center().y);
    let r = space::XS;
    let stroke = egui::Stroke::new(line::FOCUS, tint);
    let points = if *open {
        [c + egui::vec2(-r, -r / 2.0), c + egui::vec2(0.0, r / 2.0), c + egui::vec2(r, -r / 2.0)]
    } else {
        [c + egui::vec2(-r / 2.0, -r), c + egui::vec2(r / 2.0, 0.0), c + egui::vec2(-r / 2.0, r)]
    };
    painter.line(points.to_vec(), stroke);
    let galley = painter.layout_no_wrap(title.to_owned(), crate::theme::font::semibold(size::BODY), tint);
    painter.galley(egui::pos2(rect.left() + space::XL, rect.center().y - galley.size().y / 2.0), galley, tint);
    focus_ring(ui, &response, rect, radius::M);
    let (state, name) = (*open, title.to_owned());
    response.widget_info(|| {
        let mut info = WidgetInfo::labeled(Role::Button, true, &name);
        info.selected = Some(state);
        info
    });
    response
}

/// One muted line for an empty list.
pub fn empty(ui: &mut egui::Ui, text: &str) {
    ui.add_space(space::XXL);
    ui.vertical_centered(|ui| super::wrapped(ui, super::muted(text)));
}
