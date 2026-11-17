//! The launcher's dark look: a warm chestnut accent on dark grey.

use eframe::egui::{self, Color32, CornerRadius, FontId, Stroke, TextStyle, Visuals};

pub const ACCENT: Color32 = Color32::from_rgb(201, 122, 64);
pub const ACCENT_DARK: Color32 = Color32::from_rgb(150, 86, 40);
pub const GOOD: Color32 = Color32::from_rgb(120, 190, 110);
pub const WARN: Color32 = Color32::from_rgb(230, 180, 80);
pub const BAD: Color32 = Color32::from_rgb(230, 100, 90);
pub const MUTED: Color32 = Color32::from_rgb(150, 150, 150);
pub const PANEL: Color32 = Color32::from_rgb(30, 30, 32);
pub const CARD: Color32 = Color32::from_rgb(40, 40, 43);

pub fn apply(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();
    visuals.panel_fill = Color32::from_rgb(24, 24, 26);
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = Color32::from_rgb(16, 16, 18);
    visuals.faint_bg_color = CARD;
    visuals.selection.bg_fill = ACCENT_DARK;
    visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.hyperlink_color = ACCENT;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(214, 214, 214));
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(226, 226, 226));
    let radius = CornerRadius::same(4);
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = radius;
    }
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.active.bg_fill = ACCENT_DARK;
    ctx.set_visuals_of(egui::Theme::Dark, visuals);
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.text_styles.insert(TextStyle::Heading, FontId::proportional(22.0));
        style.text_styles.insert(TextStyle::Body, FontId::proportional(15.0));
        style.text_styles.insert(TextStyle::Button, FontId::proportional(15.0));
        style.text_styles.insert(TextStyle::Small, FontId::proportional(12.0));
        style.text_styles.insert(TextStyle::Monospace, FontId::monospace(13.0));
    });
}

/// A filled card frame for list rows and detail boxes.
pub fn card() -> egui::Frame {
    egui::Frame::new().fill(CARD).corner_radius(CornerRadius::same(6)).inner_margin(egui::Margin::same(10))
}

/// The big accent button (Play).
pub fn big_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new(text.to_owned()).size(22.0).strong().color(Color32::WHITE))
        .fill(ACCENT_DARK)
        .min_size(egui::vec2(180.0, 48.0))
}
