//! The Play tab: profile and version picker, the Play button, news, and the game's log.

use castanea_launcher_core::launch;
use eframe::egui::{self, RichText};

use crate::app::{App, open_folder};
use crate::tabs::profile;
use crate::theme;

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    if !app.index_loaded && !app.settings.indexes.is_empty() {
        app.load_index(ui.ctx());
    }
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(app.tr.get("play.profile")).color(theme::MUTED));
            profile_picker(app, ui);
        });
        ui.add_space(12.0);
        ui.vertical(|ui| {
            ui.label(RichText::new(app.tr.get("play.version")).color(theme::MUTED));
            version_picker(app, ui);
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if app.game.is_some() {
                if ui.add(theme::big_button(app.tr.get("play.stop"))).clicked() {
                    app.stop_game();
                }
            } else {
                let ready = launch::pick_build(&app.registry, app.profile()).is_ok();
                let button = ui.add_enabled(ready, theme::big_button(app.tr.get("play.play"))).on_hover_text("F5");
                if button.clicked() {
                    app.play();
                }
                if !ready {
                    ui.label(RichText::new(app.tr.get("play.no_build")).color(theme::WARN));
                }
            }
        });
    });
    ui.horizontal(|ui| {
        let label = if app.editing_profile { "play.hide_options" } else { "play.edit_options" };
        if ui.button(app.tr.get(label)).clicked() {
            app.editing_profile = !app.editing_profile;
        }
        if ui.button(app.tr.get("play.open_folder")).clicked() {
            let dir = app.profile().game_dir(&app.paths);
            if let Err(error) = open_folder(&dir) {
                app.error(format!("{}: {error}", dir.display()));
            }
        }
        ui.label(RichText::new(app.profile().game_dir(&app.paths).display().to_string()).small().color(theme::MUTED));
    });
    ui.separator();
    if app.editing_profile {
        egui::ScrollArea::vertical().id_salt("profile").show(ui, |ui| profile::ui(app, ui));
        return;
    }
    if app.game.is_some() || app.last_outcome.is_some() || !app.log.is_empty() {
        outcome(app, ui);
        log(app, ui);
    } else {
        news(app, ui);
    }
}

fn profile_picker(app: &mut App, ui: &mut egui::Ui) {
    let current = app.profile().name.clone();
    let mut chosen = None;
    egui::ComboBox::from_id_salt("profile").width(220.0).selected_text(current).show_ui(ui, |ui| {
        for profile in &app.profiles.profiles {
            if ui.selectable_label(profile.id == app.profile_id, &profile.name).clicked() {
                chosen = Some(profile.id.clone());
            }
        }
    });
    if let Some(id) = chosen {
        app.set_profile(id);
    }
}

fn version_picker(app: &mut App, ui: &mut egui::Ui) {
    let newest = launch::newest(&app.registry).map(|b| b.id.clone());
    let newest_text = match &newest {
        Some(id) => app.tr.fill("play.newest", &[("version", id)]),
        None => app.tr.get("play.none_installed").to_owned(),
    };
    let current = app.profile().build.clone();
    let shown = current.clone().unwrap_or_else(|| newest_text.clone());
    let mut chosen: Option<Option<String>> = None;
    egui::ComboBox::from_id_salt("version").width(220.0).selected_text(shown).show_ui(ui, |ui| {
        if ui.selectable_label(current.is_none(), &newest_text).clicked() {
            chosen = Some(None);
        }
        for build in &app.registry.builds {
            let text = format!("{}  ({})", build.id, build.version);
            let label =
                ui.add_enabled(build.is_present(), egui::Button::selectable(current.as_ref() == Some(&build.id), text));
            if label.clicked() {
                chosen = Some(Some(build.id.clone()));
            }
        }
    });
    if let Some(build) = chosen {
        app.profile_mut().build = build;
        app.save_profiles();
    }
}

fn outcome(app: &mut App, ui: &mut egui::Ui) {
    if let Some(game) = &app.game {
        ui.label(
            RichText::new(app.tr.fill("play.running", &[("pid", &game.running.pid().to_string())])).color(theme::GOOD),
        );
        ui.label(RichText::new(&game.command).monospace().small().color(theme::MUTED));
        return;
    }
    let Some(outcome) = app.last_outcome.clone() else { return };
    let code = outcome.code.map_or("-".to_owned(), |c| c.to_string());
    let seconds = outcome.seconds.to_string();
    if outcome.crashed {
        ui.label(
            RichText::new(app.tr.fill("play.crashed", &[("code", &code), ("seconds", &seconds)]))
                .color(theme::BAD)
                .strong(),
        );
        for report in &outcome.crash_reports {
            ui.horizontal(|ui| {
                ui.label(RichText::new(report.display().to_string()).monospace().small());
                if ui.small_button(app.tr.get("play.show_report")).clicked()
                    && let Ok(text) = std::fs::read_to_string(report)
                {
                    app.log = text.lines().map(str::to_owned).collect();
                }
            });
        }
    } else {
        ui.label(
            RichText::new(app.tr.fill("play.exited", &[("code", &code), ("seconds", &seconds)])).color(theme::GOOD),
        );
    }
}

fn log(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(app.tr.get("play.log")).strong());
        if ui.small_button(app.tr.get("play.copy_log")).clicked() {
            ui.ctx().copy_text(app.log.join("\n"));
        }
        if ui.small_button(app.tr.get("play.open_log")).clicked() {
            let dir = app.profile().game_dir(&app.paths).join(launch::run::LOG_DIR);
            if let Err(error) = open_folder(&dir) {
                app.error(format!("{}: {error}", dir.display()));
            }
        }
    });
    theme::card().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show_rows(
            ui,
            16.0,
            app.log.len(),
            |ui, range| {
                for line in &app.log[range] {
                    let color = if line.contains("panic") || line.contains("error") {
                        theme::BAD
                    } else {
                        egui::Color32::LIGHT_GRAY
                    };
                    ui.label(RichText::new(line).monospace().color(color));
                }
            },
        );
    });
}

fn news(app: &mut App, ui: &mut egui::Ui) {
    ui.label(RichText::new(app.tr.get("play.news")).heading());
    if app.index.news.is_empty() {
        let key = if app.settings.indexes.is_empty() { "play.no_news_source" } else { "play.no_news" };
        ui.label(RichText::new(app.tr.get(key)).color(theme::MUTED));
    }
    egui::ScrollArea::vertical().id_salt("news").show(ui, |ui| {
        for item in &app.index.news {
            theme::card().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&item.title).strong().size(17.0));
                    ui.label(RichText::new(&item.date).color(theme::MUTED));
                });
                ui.label(&item.body);
                if let Some(url) = &item.url {
                    ui.hyperlink(url);
                }
            });
            ui.add_space(6.0);
        }
        ui.add_space(8.0);
        status_summary(app, ui);
    });
}

fn status_summary(app: &App, ui: &mut egui::Ui) {
    let builds = app.registry.builds.iter().filter(|b| b.is_present()).count().to_string();
    let enabled = app.mods.iter().filter(|m| m.enabled).count().to_string();
    let total = app.mods.len().to_string();
    ui.label(
        RichText::new(app.tr.fill("play.summary", &[("builds", &builds), ("enabled", &enabled), ("mods", &total)]))
            .color(theme::MUTED),
    );
}
