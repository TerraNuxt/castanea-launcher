//! Reusable components (docs/ui-design.md): buttons, controls and rows. Pages use only these and the theme tokens.

pub mod button;
pub mod controls;
pub mod rows;

pub use button::{Button, Kind, icon_button};
pub use controls::{search_field, segmented, switch, switch_named, tag, text_field};
pub use rows::{empty, hairline, page_header, section, setting_row};

use eframe::egui::{self, RichText};

use crate::theme::{color, font, size};

/// Body text in the main colour.
pub fn text(text: impl Into<String>) -> RichText {
    RichText::new(text).font(font::regular(size::BODY)).color(color::TEXT)
}

/// A muted second line.
pub fn muted(text: impl Into<String>) -> RichText {
    RichText::new(text).font(font::regular(size::BODY)).color(color::TEXT_MUTED)
}

/// Small muted text: hints, captions, meta lines.
pub fn small(text: impl Into<String>) -> RichText {
    RichText::new(text).font(font::regular(size::SMALL)).color(color::TEXT_MUTED)
}

/// Semibold text in `px`.
pub fn strong(text: impl Into<String>, px: f32) -> RichText {
    RichText::new(text).font(font::semibold(px)).color(color::TEXT)
}

/// An icon glyph as text.
pub fn icon(glyph: &str, tint: egui::Color32) -> RichText {
    RichText::new(glyph).font(font::icon(size::ICON)).color(tint)
}

/// A label that keeps one line and ends with … when it does not fit.
pub fn one_line(ui: &mut egui::Ui, text: RichText) -> egui::Response {
    ui.add(egui::Label::new(text).truncate())
}

/// A label that wraps at the available width.
pub fn wrapped(ui: &mut egui::Ui, text: RichText) -> egui::Response {
    ui.add(egui::Label::new(text).wrap())
}
