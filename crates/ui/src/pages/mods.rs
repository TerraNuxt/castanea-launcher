//! Mods: Installed (switches, update, remove, import, problems) and Browse (`browse.rs`).

use std::collections::HashMap;

use castanea_launcher_core::index::install as index_install;
use castanea_launcher_core::mods::checks::{self, Problem};
use castanea_launcher_core::mods::{InstalledMod, Packaging, import, installed};
use eframe::egui::{self, RichText};

use crate::app::{App, Confirm, ModsView, open_or_note};
use crate::pages::browse;
use crate::theme::{color, size, space};
use crate::widgets::{self, Button, Kind, icon_button};
use crate::{icons, probe};

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    let title = app.tr.get("nav.mods").to_owned();
    widgets::page_header(ui, &title, |ui| {
        let mut view = if app.mods_view == ModsView::Installed { 0 } else { 1 };
        let choices = [app.tr.get("mods.installed"), app.tr.get("mods.browse")];
        if widgets::segmented(ui, &mut view, &choices) {
            app.mods_view = if view == 0 { ModsView::Installed } else { ModsView::Browse };
        }
    });
    match app.mods_view {
        ModsView::Installed => installed_ui(app, ui),
        ModsView::Browse => browse::ui(app, ui),
    }
}

fn installed_ui(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        if Button::new(Kind::Secondary, app.tr.get("mods.import_zip")).show(ui).clicked()
            && let Some(file) = rfd::FileDialog::new().add_filter("zip", &["zip"]).pick_file()
        {
            import_from(app, &file);
        }
        if Button::new(Kind::Secondary, app.tr.get("mods.import_folder")).show(ui).clicked()
            && let Some(folder) = rfd::FileDialog::new().pick_folder()
        {
            import_from(app, &folder);
        }
        if Button::new(Kind::Secondary, app.tr.get("mods.open_folder")).show(ui).clicked() {
            let dir = app.mods_dir();
            open_or_note(app, &dir);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icon_button(ui, icons::ARROWS_CLOCKWISE, app.tr.get("common.refresh")).clicked() {
                app.reload_mods();
            }
        });
    });
    ui.add_space(space::L);
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
        widgets::empty(ui, app.tr.get("mods.none"));
        return;
    }
    let mut action: Option<Action> = None;
    widgets::hairline(ui);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for package in &app.mods {
            let mine: Vec<&Problem> =
                problems.iter().filter(|(f, _)| *f == package.file_name).map(|(_, p)| p).collect();
            let update = updates.iter().find(|u| u.id == package.id());
            row(app, ui, package, &mine, update.map(|u| u.available.as_str()), &mut action);
            widgets::hairline(ui);
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
    let name = package.manifest.as_ref().map_or(package.file_name.clone(), |m| m.name.clone());
    let top = ui.cursor().top();
    ui.add_space(space::M);
    ui.horizontal(|ui| {
        let mut enabled = package.enabled;
        let label =
            app.tr.fill(if package.enabled { "mods.disable_named" } else { "mods.enable_named" }, &[("name", &name)]);
        if widgets::switch_named(ui, &mut enabled, &label).changed() {
            *action = Some(if enabled { Action::Enable(file.clone()) } else { Action::Disable(file.clone()) });
        }
        ui.add_space(space::S);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icon_button(ui, icons::TRASH, &app.tr.fill("mods.remove_named", &[("name", &name)])).clicked() {
                *action = Some(Action::Remove(file.clone()));
            }
            if let Some(version) = update
                && Button::new(Kind::Secondary, &app.tr.fill("mods.update_to", &[("version", version)]))
                    .show(ui)
                    .clicked()
            {
                *action = Some(Action::Update(package.id().to_owned()));
            }
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = crate::theme::layout::LINE_GAP;
                    let tint = if package.enabled { color::TEXT } else { color::TEXT_MUTED };
                    match &package.manifest {
                        Ok(manifest) => {
                            ui.horizontal(|ui| {
                                widgets::one_line(ui, widgets::strong(&manifest.name, size::BODY).color(tint));
                                ui.label(widgets::small(&manifest.version));
                                if manifest.has_code() {
                                    widgets::tag(ui, app.tr.get("mods.has_code"), color::WARN)
                                        .on_hover_text(app.tr.get("mods.has_code_hint"));
                                }
                            });
                            widgets::one_line(ui, widgets::small(meta_line(app, package)));
                        }
                        Err(_) => {
                            widgets::one_line(ui, widgets::strong(&package.file_name, size::BODY).color(tint));
                        }
                    }
                });
            });
        });
    });
    for problem in problems {
        ui.horizontal(|ui| {
            ui.add_space(theme_switch_width());
            ui.label(widgets::icon(icons::WARNING, color::DANGER));
            widgets::wrapped(ui, RichText::new(problem_text(app, problem)).size(size::SMALL).color(color::DANGER));
        });
    }
    ui.add_space(space::M);
    let rect = egui::Rect::from_x_y_ranges(ui.max_rect().x_range(), top..=ui.cursor().top());
    probe::mark(ui.ctx(), format!("mod:{}", package.file_name), rect);
}

/// The indent of a row's text: the switch and the gap after it.
fn theme_switch_width() -> f32 {
    crate::theme::layout::SWITCH[0] + space::S * 2.0
}

/// "by Lilly · amber · folder · side: client · needs frost"
fn meta_line(app: &App, package: &InstalledMod) -> String {
    let mut parts = Vec::new();
    if let Ok(manifest) = &package.manifest {
        if !manifest.author.is_empty() {
            parts.push(app.tr.fill("mods.by", &[("author", &manifest.author)]));
        }
        parts.push(manifest.id.clone());
    }
    parts.push(
        app.tr.get(if package.packaging == Packaging::Zip { "mods.kind_zip" } else { "mods.kind_folder" }).to_owned(),
    );
    if let Ok(manifest) = &package.manifest {
        if manifest.side != "both" {
            parts.push(app.tr.fill("mods.side", &[("side", &manifest.side)]));
        }
        if !manifest.depends.is_empty() {
            parts.push(app.tr.fill("mods.depends", &[("list", &manifest.depends.join(", "))]));
        }
    }
    parts.join("  ·  ")
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
