//! Reusable components (docs/ui-design.md): buttons, cards, controls and painted pictures. Pages use only these and
//! the theme tokens.

pub mod button;
pub mod card;
pub mod controls;
pub mod paint;

pub use button::{Kind, Pill, icon_button};
pub use card::Card;
pub use controls::{chip, search_field, section_title, segmented, switch};

use eframe::egui::{self, RichText};

use crate::theme::{color, font, size};

/// A muted second line.
pub fn muted(text: impl Into<String>) -> RichText {
    RichText::new(text).color(color::TEXT_MUTED)
}

/// A faint hint.
pub fn faint(text: impl Into<String>) -> RichText {
    RichText::new(text).color(color::TEXT_FAINT).size(size::CAPTION)
}

/// Semibold text in `size`.
pub fn strong(text: impl Into<String>, px: f32) -> RichText {
    RichText::new(text).font(font::semibold(px)).color(color::TEXT)
}

/// An icon glyph as text.
pub fn icon(glyph: &str, px: f32, tint: egui::Color32) -> RichText {
    RichText::new(glyph).font(font::icon(px, false)).color(tint)
}

/// A page title with optional actions on the right; returns the actions' result.
pub fn page_header<R>(ui: &mut egui::Ui, title: &str, actions: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).font(font::bold(size::HEADING)).color(color::TEXT));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), actions).inner
    })
    .inner
}

/// A label that keeps one line and ends with … when it does not fit.
pub fn one_line(ui: &mut egui::Ui, text: RichText) -> egui::Response {
    ui.add(egui::Label::new(text).truncate())
}
