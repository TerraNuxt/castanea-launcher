//! Versions: installed builds (several side by side), adding a local folder, and releases to download.

use castanea_launcher_core::install::local;
use castanea_launcher_core::install::{Origin, detect};
use eframe::egui::{self, RichText};

use crate::app::{App, Confirm, open_or_note};
use crate::pages::mods::empty;
use crate::shell::megabytes;
use crate::theme::{color, size, space};
use crate::widgets::{self, Card, Kind, Pill, icon_button};
use crate::{icons, probe};

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    if !app.releases_loaded {
        app.load_releases(ui.ctx());
    }
    let title = app.tr.get("nav.versions").to_owned();
    widgets::page_header(ui, &title, |ui| {
        if Pill::new(Kind::Ghost, app.tr.get("versions.scan")).icon(icons::ARROWS_CLOCKWISE).show(ui).clicked() {
            match detect::refresh(&app.paths) {
                Ok(registry) => app.registry = registry,
                Err(error) => app.error(error.to_string()),
            }
        }
        if Pill::new(Kind::Accent, app.tr.get("versions.add_folder")).icon(icons::FOLDER_OPEN).show(ui).clicked()
            && let Some(folder) = rfd::FileDialog::new().pick_folder()
        {
            add_folder(app, &folder);
        }
    });
    ui.add_space(space::M);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        installed(app, ui);
        ui.add_space(space::XL);
        releases(app, ui);
    });
}

fn installed(app: &mut App, ui: &mut egui::Ui) {
    widgets::section_title(ui, icons::HARD_DRIVES, app.tr.get("versions.installed_title"));
    ui.add_space(space::XS);
    if app.registry.builds.is_empty() {
        empty(ui, icons::CUBE, app.tr.get("versions.none"));
    }
    let mut remove = None;
    let mut open = None;
    for build in &app.registry.builds {
        let response = Card::new().padding(space::M).show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::hover());
                ui.painter().rect_filled(
                    rect,
                    crate::theme::radius(crate::theme::radius::M),
                    color::ACCENT.gamma_multiply(0.18),
                );
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    icons::CUBE,
                    crate::theme::font::icon(22.0, true),
                    color::ACCENT,
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let key = if build.managed { "common.remove" } else { "versions.forget" };
                    let label = format!("{} {}", app.tr.get(key), build.id);
                    if icon_button(ui, icons::TRASH, &label).clicked() {
                        remove = Some(build.id.clone());
                    }
                    let label = format!("{} {}", app.tr.get("versions.open"), build.id);
                    if icon_button(ui, icons::FOLDER_OPEN, &label).clicked() {
                        open = Some(build.root.clone());
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.horizontal(|ui| {
                                ui.label(widgets::strong(&build.id, size::TITLE));
                                ui.label(widgets::muted(&build.version).size(size::SMALL));
                                let origin = match &build.origin {
                                    Origin::Github { repo, .. } => format!("GitHub {repo}"),
                                    Origin::Local => app.tr.get("versions.origin_local").to_owned(),
                                    Origin::Detected => app.tr.get("versions.origin_detected").to_owned(),
                                };
                                widgets::chip(ui, &origin, color::INFO);
                                if !build.is_present() {
                                    widgets::chip(ui, app.tr.get("versions.missing"), color::DANGER);
                                }
                                if build.sha256.is_some() {
                                    widgets::chip(ui, app.tr.get("versions.verified"), color::PLAY)
                                        .on_hover_text(build.sha256.clone().unwrap_or_default());
                                }
                            });
                            widgets::one_line(
                                ui,
                                RichText::new(build.root.display().to_string())
                                    .monospace()
                                    .size(size::CAPTION)
                                    .color(color::TEXT_FAINT),
                            );
                        });
                    });
                });
            });
        });
        probe::mark(ui.ctx(), format!("build:{}", build.id), response.response.rect);
        ui.add_space(space::S);
    }
    if let Some(id) = remove {
        app.confirm = Some(Confirm::RemoveBuild { id });
    }
    if let Some(dir) = open {
        open_or_note(app, &dir);
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
        widgets::section_title(ui, icons::GITHUB, app.tr.get("versions.releases"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled_ui(!app.settings.offline, |ui| {
                    icon_button(ui, icons::ARROWS_CLOCKWISE, app.tr.get("common.refresh"))
                })
                .inner
                .clicked()
            {
                app.load_releases(ui.ctx());
            }
        });
    });
    ui.add_space(space::XS);
    if app.settings.offline {
        ui.horizontal(|ui| {
            ui.label(widgets::icon(icons::WIFI_SLASH, 16.0, color::WARN));
            ui.label(RichText::new(app.tr.get("versions.offline")).color(color::WARN));
        });
        return;
    }
    for error in &app.release_errors {
        ui.horizontal(|ui| {
            ui.label(widgets::icon(icons::WARNING, 16.0, color::DANGER));
            ui.add(egui::Label::new(RichText::new(error).color(color::DANGER)).wrap());
        });
    }
    if app.releases.is_empty() && app.release_errors.is_empty() {
        let key = if app.jobs.running > 0 { "status.working" } else { "versions.no_releases" };
        ui.label(widgets::muted(app.tr.get(key)));
    }
    let mut install = None;
    let busy = app.jobs.progress.lock().is_ok_and(|p| p.is_some());
    for (source, build) in &app.releases {
        Card::new().padding(space::M).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if app.registry.get(&build.id).is_some() {
                        widgets::chip(ui, app.tr.get("browse.installed"), color::PLAY);
                    } else if let Some(asset) = &build.asset {
                        let text = app.tr.fill("versions.download", &[("size", &megabytes(asset.size, None))]);
                        if Pill::new(Kind::Accent, &text).icon(icons::DOWNLOAD).enabled(!busy).show(ui).clicked() {
                            install = Some((*source, build.clone()));
                        }
                    } else {
                        ui.label(widgets::faint(app.tr.get("versions.no_asset")));
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        widgets::one_line(ui, widgets::strong(&build.title, size::TITLE));
                        if build.prerelease {
                            widgets::chip(ui, app.tr.get("versions.prerelease"), color::WARN);
                        }
                        ui.label(widgets::faint(build.published.get(..10).unwrap_or("")));
                    });
                });
            });
            if !build.notes.is_empty() {
                ui.collapsing(app.tr.get("versions.notes"), |ui| ui.label(&build.notes));
            }
        });
        ui.add_space(space::S);
    }
    if let Some((source, build)) = install {
        app.install_release(ui.ctx(), source, build);
    }
}
