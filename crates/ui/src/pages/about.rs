//! About: name, version, what the launcher does, licence and credits, keyboard shortcuts, and the safety rules.

use castanea_launcher_core::backend::Reason;
use castanea_launcher_core::settings::Graphics;
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
        diagnostics(app, ui);
        mark(ui, "about:diagnostics", top);

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

/// The game version, the Graphics setting, and the backend the last start used with the reason.
fn diagnostics(app: &App, ui: &mut egui::Ui) {
    widgets::section(ui, app.tr.get("about.diagnostics"));
    let game = app.installed.as_ref().map_or_else(|| app.tr.get("about.diag_none").to_owned(), |b| b.version.clone());
    let setting = match app.settings.game.graphics {
        Graphics::Auto => app.tr.get("settings.graphics_auto").to_owned(),
        Graphics::Vulkan => "Vulkan".to_owned(),
        Graphics::Wgpu => "wgpu".to_owned(),
    };
    let last = app.backend_last.as_ref();
    let backend = match last {
        Some(last) if last.backend == "auto" => app.tr.get("about.diag_game_picks").to_owned(),
        Some(last) => last.backend.replace("vulkan", "Vulkan"),
        None => app.tr.get("about.diag_not_started").to_owned(),
    };
    let reason = last.map(|l| app.tr.get(reason_key(l.reason)).to_owned());
    let adapter = last
        .filter(|l| !l.adapter.is_empty())
        .map(|l| if l.driver.is_empty() { l.adapter.clone() } else { format!("{} · {}", l.adapter, l.driver) });
    let mut rows = vec![("about.diag_game", game), ("about.diag_setting", setting), ("about.diag_backend", backend)];
    rows.extend(reason.map(|r| ("about.diag_reason", r)));
    rows.extend(adapter.map(|a| ("about.diag_adapter", a)));
    for (key, value) in rows {
        setting_row(ui, app.tr.get(key), None, |ui| {
            ui.add_space(crate::theme::layout::LABEL_NUDGE);
            ui.label(widgets::muted(value));
        });
    }
}

fn reason_key(reason: Reason) -> &'static str {
    match reason {
        Reason::VulkanWorks => "backend.reason_vulkan_works",
        Reason::VulkanFailed => "backend.reason_vulkan_failed",
        Reason::VulkanCrashed => "backend.reason_vulkan_crashed",
        Reason::NoProbe => "backend.reason_no_probe",
        Reason::Setting => "backend.reason_setting",
        Reason::Environment => "backend.reason_environment",
    }
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
