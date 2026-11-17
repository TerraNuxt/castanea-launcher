//! About: name, version, what the launcher does, licence and credits, keyboard shortcuts, and the safety rules.

use eframe::egui;

use crate::app::App;
use crate::probe;
use crate::theme::{color, layout, size, space};
use crate::widgets::{self, setting_row};

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.set_max_width(layout::READABLE);
        let top = ui.cursor().top();
        ui.label(widgets::strong("castanea launcher", size::TITLE));
        ui.label(widgets::small(format!("{} {}", app.tr.get("about.version"), env!("CARGO_PKG_VERSION"))));
        ui.add_space(space::M);
        widgets::wrapped(ui, widgets::text(app.tr.get("about.text")));
        ui.add_space(space::S);
        let link = egui::RichText::new("github.com/TerraNuxt/castanea-launcher").size(size::BODY).color(color::TEXT);
        ui.hyperlink_to(link.underline(), env!("CARGO_PKG_REPOSITORY"));
        ui.add_space(space::S);
        widgets::wrapped(ui, widgets::small(app.tr.get("about.license")));
        mark(ui, "about:intro", top);

        let top = ui.cursor().top();
        widgets::section(ui, app.tr.get("about.keys"));
        for (keys, text) in [
            ("Ctrl+1 … Ctrl+5", "about.key_pages"),
            ("F5", "about.key_play"),
            ("Ctrl+F", "about.key_search"),
            ("Tab / Shift+Tab", "about.key_focus"),
            ("Enter / Space", "about.key_activate"),
            ("Esc", "about.key_escape"),
        ] {
            setting_row(ui, keys, None, |ui| {
                ui.add_space(crate::theme::layout::LABEL_NUDGE);
                ui.label(widgets::muted(app.tr.get(text)));
            });
        }
        mark(ui, "about:keys", top);

        let top = ui.cursor().top();
        widgets::section(ui, app.tr.get("about.security"));
        ui.add_space(space::M);
        widgets::wrapped(ui, widgets::muted(app.tr.get("about.security_text")));
        mark(ui, "about:security", top);
        ui.add_space(space::XL);
    });
}

fn mark(ui: &egui::Ui, name: &str, top: f32) {
    let rect = egui::Rect::from_x_y_ranges(ui.max_rect().x_range(), top..=ui.cursor().top());
    probe::mark(ui.ctx(), name, rect);
}
