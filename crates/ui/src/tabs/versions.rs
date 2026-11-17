//! The Versions tab: installed builds (several side by side), adding a local folder, and releases to download.

use castanea_launcher_core::install::local;
use castanea_launcher_core::install::{Origin, detect};
use eframe::egui::{self, RichText};

use crate::app::{App, Confirm, open_folder};
use crate::shell::megabytes;
use crate::theme;

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    if !app.releases_loaded {
        app.load_releases(ui.ctx());
    }
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        installed(app, ui);
        ui.add_space(14.0);
        releases(app, ui);
    });
}

fn installed(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(app.tr.get("versions.installed_title")).heading());
        if ui.button(app.tr.get("versions.add_folder")).clicked()
            && let Some(folder) = rfd::FileDialog::new().pick_folder()
        {
            add_folder(app, &folder);
        }
        if ui.button(app.tr.get("versions.scan")).clicked() {
            match detect::refresh(&app.paths) {
                Ok(registry) => app.registry = registry,
                Err(error) => app.error(error.to_string()),
            }
        }
    });
    if app.registry.builds.is_empty() {
        ui.label(RichText::new(app.tr.get("versions.none")).color(theme::MUTED));
    }
    let mut remove = None;
    let mut open = None;
    for build in &app.registry.builds {
        theme::card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(&build.id).strong().size(16.0));
                ui.label(RichText::new(&build.version).color(theme::MUTED));
                let origin = match &build.origin {
                    Origin::Github { repo, .. } => format!("GitHub {repo}"),
                    Origin::Local => app.tr.get("versions.origin_local").to_owned(),
                    Origin::Detected => app.tr.get("versions.origin_detected").to_owned(),
                };
                ui.label(RichText::new(origin).small().color(theme::MUTED));
                if !build.is_present() {
                    ui.label(RichText::new(app.tr.get("versions.missing")).color(theme::BAD));
                }
                if build.sha256.is_some() {
                    ui.label(RichText::new(app.tr.get("versions.verified")).small().color(theme::GOOD))
                        .on_hover_text(build.sha256.clone().unwrap_or_default());
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let key = if build.managed { "common.remove" } else { "versions.forget" };
                    if ui.small_button(app.tr.get(key)).clicked() {
                        remove = Some(build.id.clone());
                    }
                    if ui.small_button(app.tr.get("versions.open")).clicked() {
                        open = Some(build.root.clone());
                    }
                });
            });
            ui.label(RichText::new(build.root.display().to_string()).small().monospace().color(theme::MUTED));
        });
        ui.add_space(4.0);
    }
    if let Some(id) = remove {
        app.confirm = Some(Confirm::RemoveBuild { id });
    }
    if let Some(dir) = open
        && let Err(error) = open_folder(&dir)
    {
        app.error(format!("{}: {error}", dir.display()));
    }
}

fn add_folder(app: &mut App, folder: &std::path::Path) {
    match local::inspect(&app.registry, folder, Origin::Local) {
        Ok(build) => {
            let text = app.tr.fill("versions.added", &[("version", &build.id)]);
            app.registry.put(build);
            app.save_registry();
            app.info(text);
        }
        Err(error) => app.error(error.to_string()),
    }
}

fn releases(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(app.tr.get("versions.releases")).heading());
        if ui.add_enabled(!app.settings.offline, egui::Button::new(app.tr.get("common.refresh"))).clicked() {
            app.load_releases(ui.ctx());
        }
    });
    if app.settings.offline {
        ui.label(RichText::new(app.tr.get("versions.offline")).color(theme::WARN));
        return;
    }
    for error in &app.release_errors {
        ui.label(RichText::new(error).color(theme::BAD));
    }
    if app.releases.is_empty() && app.release_errors.is_empty() {
        let key = if app.jobs.running > 0 { "status.working" } else { "versions.no_releases" };
        ui.label(RichText::new(app.tr.get(key)).color(theme::MUTED));
    }
    let mut install = None;
    let busy = app.jobs.progress.lock().is_ok_and(|p| p.is_some());
    for (source, build) in &app.releases {
        theme::card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(&build.title).strong());
                if build.prerelease {
                    ui.label(RichText::new(app.tr.get("versions.prerelease")).small().color(theme::WARN));
                }
                ui.label(RichText::new(build.published.get(..10).unwrap_or("")).small().color(theme::MUTED));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if app.registry.get(&build.id).is_some() {
                        ui.label(RichText::new(app.tr.get("browse.installed")).color(theme::GOOD));
                    } else if let Some(asset) = &build.asset {
                        let text = app.tr.fill("versions.download", &[("size", &megabytes(asset.size, None))]);
                        if ui.add_enabled(!busy, egui::Button::new(text)).clicked() {
                            install = Some((*source, build.clone()));
                        }
                    } else {
                        ui.label(RichText::new(app.tr.get("versions.no_asset")).small().color(theme::MUTED));
                    }
                });
            });
            if !build.notes.is_empty() {
                ui.collapsing(app.tr.get("versions.notes"), |ui| ui.label(&build.notes));
            }
        });
        ui.add_space(4.0);
    }
    if let Some((source, build)) = install {
        app.install_release(ui.ctx(), source, build);
    }
}
