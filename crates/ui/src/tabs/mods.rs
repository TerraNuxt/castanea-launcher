//! The Mods tab: Installed (enable, disable, remove, import, problems) and Browse (`browse.rs`).

use std::collections::HashMap;

use castanea_launcher_core::index::install as index_install;
use castanea_launcher_core::mods::checks::{self, Problem};
use castanea_launcher_core::mods::{InstalledMod, Packaging, import, installed};
use eframe::egui::{self, RichText};

use crate::app::{App, Confirm, ModsView, open_folder};
use crate::tabs::browse;
use crate::theme;

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.selectable_value(
            &mut app.mods_view,
            ModsView::Installed,
            RichText::new(app.tr.get("mods.installed")).size(16.0),
        );
        ui.selectable_value(&mut app.mods_view, ModsView::Browse, RichText::new(app.tr.get("mods.browse")).size(16.0));
        ui.label(
            RichText::new(app.tr.fill("mods.for_profile", &[("profile", &app.profile().name)])).color(theme::MUTED),
        );
    });
    ui.separator();
    match app.mods_view {
        ModsView::Installed => installed_ui(app, ui),
        ModsView::Browse => browse::ui(app, ui),
    }
}

fn installed_ui(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        if ui.button(app.tr.get("mods.import_zip")).clicked()
            && let Some(file) = rfd::FileDialog::new().add_filter("zip", &["zip"]).pick_file()
        {
            import_from(app, &file);
        }
        if ui.button(app.tr.get("mods.import_folder")).clicked()
            && let Some(folder) = rfd::FileDialog::new().pick_folder()
        {
            import_from(app, &folder);
        }
        if ui.button(app.tr.get("mods.open_folder")).clicked() {
            let dir = app.mods_dir();
            if let Err(error) = open_folder(&dir) {
                app.error(format!("{}: {error}", dir.display()));
            }
        }
        if ui.button(app.tr.get("common.refresh")).clicked() {
            app.reload_mods();
        }
    });
    let game = app.game_version();
    let ranges: HashMap<String, String> = app
        .index
        .mods
        .iter()
        .filter_map(|m| m.best_version(game.as_deref()).and_then(|v| v.game.clone()).map(|g| (m.id.clone(), g)))
        .collect();
    let problems = checks::check(&app.mods, game.as_deref(), &ranges);
    let updates = index_install::updates(&app.mods, &app.index, game.as_deref());
    if app.mods.is_empty() {
        ui.add_space(20.0);
        ui.label(RichText::new(app.tr.get("mods.none")).color(theme::MUTED));
        return;
    }
    let mut action: Option<Action> = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for package in &app.mods {
            let mine: Vec<&Problem> =
                problems.iter().filter(|(f, _)| *f == package.file_name).map(|(_, p)| p).collect();
            let update = updates.iter().find(|u| u.id == package.id());
            row(app, ui, package, &mine, update.map(|u| u.available.as_str()), &mut action);
            ui.add_space(4.0);
        }
    });
    if let Some(action) = action {
        run(app, ui.ctx(), action);
    }
}

enum Action {
    Enable(String),
    Disable(String),
    Remove(String),
    Update(String),
}

fn row(
    app: &App,
    ui: &mut egui::Ui,
    package: &InstalledMod,
    problems: &[&Problem],
    update: Option<&str>,
    action: &mut Option<Action>,
) {
    let file = package.file_name.clone();
    theme::card().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            let mut enabled = package.enabled;
            let toggle = ui.checkbox(&mut enabled, "").on_hover_text(app.tr.get(if package.enabled {
                "mods.disable"
            } else {
                "mods.enable"
            }));
            if toggle.changed() {
                *action = Some(if enabled { Action::Enable(file.clone()) } else { Action::Disable(file.clone()) });
            }
            match &package.manifest {
                Ok(manifest) => {
                    ui.label(RichText::new(&manifest.name).strong().size(16.0));
                    ui.label(RichText::new(&manifest.version).color(theme::MUTED));
                    if !manifest.author.is_empty() {
                        ui.label(
                            RichText::new(app.tr.fill("mods.by", &[("author", &manifest.author)])).color(theme::MUTED),
                        );
                    }
                    if manifest.has_code() {
                        ui.label(RichText::new(app.tr.get("mods.has_code")).color(theme::WARN))
                            .on_hover_text(app.tr.get("mods.has_code_hint"));
                    }
                    if manifest.side != "both" {
                        ui.label(
                            RichText::new(app.tr.fill("mods.side", &[("side", &manifest.side)])).color(theme::MUTED),
                        );
                    }
                }
                Err(_) => {
                    ui.label(RichText::new(&package.file_name).strong());
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button(app.tr.get("common.remove")).clicked() {
                    *action = Some(Action::Remove(file.clone()));
                }
                if let Some(version) = update
                    && ui.small_button(app.tr.fill("mods.update_to", &[("version", version)])).clicked()
                {
                    *action = Some(Action::Update(package.id().to_owned()));
                }
                let kind = if package.packaging == Packaging::Zip { "zip" } else { "folder" };
                ui.label(RichText::new(format!("{} · {kind}", package.id())).small().color(theme::MUTED));
            });
        });
        if let Ok(manifest) = &package.manifest
            && !manifest.depends.is_empty()
        {
            ui.label(RichText::new(app.tr.fill("mods.depends", &[("list", &manifest.depends.join(", "))])).small());
        }
        for problem in problems {
            ui.label(RichText::new(problem_text(app, problem)).color(theme::BAD));
        }
    });
}

pub fn problem_text(app: &App, problem: &Problem) -> String {
    match problem {
        Problem::Broken(error) => app.tr.fill("problem.broken", &[("error", error)]),
        Problem::DuplicateId => app.tr.get("problem.duplicate").to_owned(),
        Problem::MissingDependency(id) => app.tr.fill("problem.missing_dep", &[("id", id)]),
        Problem::DisabledDependency(id) => app.tr.fill("problem.disabled_dep", &[("id", id)]),
        Problem::Cycle => app.tr.get("problem.cycle").to_owned(),
        Problem::UnknownKeys(keys) => app.tr.fill("problem.unknown_keys", &[("keys", &keys.join(", "))]),
        Problem::Incompatible { range, game } => {
            app.tr.fill("problem.incompatible", &[("range", range), ("game", game)])
        }
    }
}

fn run(app: &mut App, ctx: &egui::Context, action: Action) {
    let mods_dir = app.mods_dir();
    let result = match action {
        Action::Enable(file) => {
            app.confirm = Some(Confirm::EnableMod { file_name: file });
            Ok(())
        }
        Action::Disable(file) => match app.mods.iter().find(|m| m.file_name == file) {
            Some(package) => installed::set_enabled(&mods_dir, package, false).map(|_| ()),
            None => Ok(()),
        },
        Action::Remove(file) => {
            app.confirm = Some(Confirm::RemoveMod { file_name: file });
            Ok(())
        }
        Action::Update(id) => {
            app.install_mod(ctx, &id);
            Ok(())
        }
    };
    if let Err(error) = result {
        app.error(error.to_string());
    }
    app.reload_mods();
}

fn import_from(app: &mut App, source: &std::path::Path) {
    match import::import(&app.mods_dir(), source, false) {
        Ok(package) => {
            let text = app.tr.fill("mods.installed_disabled", &[("id", package.id())]);
            app.info(text);
        }
        Err(error) => app.error(error.to_string()),
    }
    app.reload_mods();
}

/// What a mod declares and holds, for the "enable this mod?" dialog.
pub fn declared_text(app: &App, package: &InstalledMod) -> String {
    let mut lines = Vec::new();
    match &package.manifest {
        Ok(manifest) => {
            lines.push(format!("{} {} ({})", manifest.name, manifest.version, manifest.id));
            if !manifest.author.is_empty() {
                lines.push(app.tr.fill("mods.by", &[("author", &manifest.author)]));
            }
            lines.push(app.tr.fill("mods.side", &[("side", &manifest.side)]));
            lines.push(match &manifest.wasm {
                Some(file) => app.tr.fill("mods.declares_code", &[("file", file)]),
                None => app.tr.get("mods.declares_data").to_owned(),
            });
            if !manifest.depends.is_empty() {
                lines.push(app.tr.fill("mods.depends", &[("list", &manifest.depends.join(", "))]));
                let deps: Vec<String> = checks::dependencies_of(&app.mods, &manifest.id)
                    .iter()
                    .filter(|m| !m.enabled)
                    .map(|m| m.id().to_owned())
                    .collect();
                if !deps.is_empty() {
                    lines.push(app.tr.fill("mods.also_enable", &[("list", &deps.join(", "))]));
                }
            }
            if !manifest.aliases.is_empty() {
                lines.push(app.tr.fill("mods.aliases", &[("count", &manifest.aliases.len().to_string())]));
            }
        }
        Err(error) => lines.push(app.tr.fill("problem.broken", &[("error", error)])),
    }
    if let Ok(declared) = installed::declared(package) {
        let kinds: Vec<String> = declared.kinds.iter().map(|(k, n)| format!("{n} {k}")).collect();
        lines.push(app.tr.fill("mods.files", &[("count", &declared.files.to_string()), ("kinds", &kinds.join(", "))]));
    }
    lines.push(String::new());
    lines.push(app.tr.get("mods.enable_warning").to_owned());
    lines.join("\n")
}
