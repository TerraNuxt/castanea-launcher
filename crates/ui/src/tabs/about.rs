//! The About tab: version, licence, links, keyboard shortcuts, and the security rules in short.

use eframe::egui::{self, RichText};

use crate::app::App;
use crate::theme;

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.label(RichText::new("castanea launcher").size(26.0).strong().color(theme::ACCENT));
        ui.label(
            RichText::new(format!("{} {}", app.tr.get("about.version"), env!("CARGO_PKG_VERSION"))).color(theme::MUTED),
        );
        ui.add_space(6.0);
        ui.label(app.tr.get("about.text"));
        ui.hyperlink_to("github.com/TerraNuxt/castanea-launcher", env!("CARGO_PKG_REPOSITORY"));
        ui.label(RichText::new(app.tr.get("about.license")).small().color(theme::MUTED));
        ui.add_space(12.0);
        ui.label(RichText::new(app.tr.get("about.keys")).heading());
        egui::Grid::new("keys").num_columns(2).spacing([24.0, 4.0]).show(ui, |ui| {
            for (key, text) in [
                ("Ctrl+1 … Ctrl+5", "about.key_tabs"),
                ("F5", "about.key_play"),
                ("Ctrl+F", "about.key_search"),
                ("Tab / Shift+Tab", "about.key_focus"),
                ("Esc", "about.key_escape"),
            ] {
                ui.label(RichText::new(key).monospace());
                ui.label(app.tr.get(text));
                ui.end_row();
            }
        });
        ui.add_space(12.0);
        ui.label(RichText::new(app.tr.get("about.security")).heading());
        ui.label(app.tr.get("about.security_text"));
    });
}
