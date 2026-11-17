//! About: name, version, what the launcher does, licence and credits, keyboard shortcuts, and the safety rules.

use eframe::egui;

use crate::app::{App, open_url};
use crate::launcher::{self, LauncherUpdate};
use crate::probe;
use crate::theme::{color, layout, size, space};
use crate::widgets::{self, Button, Kind, setting_row};

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().id_salt("about").auto_shrink([false, false]).show(ui, |ui| {
        ui.set_max_width(layout::READABLE);
        let top = ui.cursor().top();
        crate::bmfont::bm_label(ui, "castanea launcher", size::HEADING);
        ui.add_space(space::XS);
        ui.label(widgets::small(format!("{} {}", app.tr.get("about.version"), env!("CARGO_PKG_VERSION"))));
        ui.add_space(space::XS);
        ui.label(widgets::small(app.launcher_line()));
        launcher_update(app, ui);
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

/// What the launcher's update asks of the user: a restart, or the download page when something else owns the install.
fn launcher_update(app: &mut App, ui: &mut egui::Ui) {
    let state = app.launcher.clone();
    match &state {
        LauncherUpdate::Restart { .. } => {
            ui.add_space(space::S);
            if Button::new(Kind::Secondary, app.tr.get("launcher.restart")).show(ui).clicked() {
                app.restart_launcher(ui.ctx());
            }
        }
        LauncherUpdate::Managed { why, .. } => {
            ui.add_space(space::S);
            widgets::wrapped(ui, widgets::muted(app.why_managed(*why)));
            ui.add_space(space::S);
            if Button::new(Kind::Secondary, app.tr.get("launcher.open_download")).show(ui).clicked() {
                open_url(app, &launcher::download_page());
            }
        }
        LauncherUpdate::Refused { .. } => {
            ui.add_space(space::S);
            widgets::wrapped(ui, widgets::muted(app.tr.get("launcher.refused_text")));
        }
        _ => {}
    }
    let shows_notes = matches!(state, LauncherUpdate::Restart { .. } | LauncherUpdate::Managed { .. });
    if let Some((version, text)) = app.launcher_notes.clone().filter(|(_, t)| shows_notes && !t.is_empty()) {
        ui.add_space(space::M);
        ui.label(widgets::strong(app.tr.fill("launcher.notes_title", &[("version", &version)]), size::BODY));
        ui.add_space(space::XS);
        widgets::wrapped(ui, widgets::muted(text));
    }
    let skipped = castanea_launcher_core::selfupdate::Files::new(&app.paths).skipped();
    if !skipped.is_empty() {
        ui.add_space(space::S);
        widgets::wrapped(ui, widgets::small(app.tr.fill("launcher.skipped", &[("versions", &skipped.join(", "))])));
    }
}
