//! The window frame: tab bar, status bar, keyboard shortcuts, the confirm dialog, and the tab in the middle.

use castanea_launcher_core::mods::{checks, installed};
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers, RichText};

use crate::app::{App, Confirm, ModsView, Note, Tab};
use crate::tabs;
use crate::theme;

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    app.collect_jobs();
    app.poll_game();
    if app.game.is_some() || app.jobs.running > 0 {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
    }
    shortcuts(app, ui.ctx());
    egui::Panel::top("tabs")
        .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(egui::Margin::symmetric(12, 8)))
        .show(ui, |ui| tab_bar(app, ui));
    egui::Panel::bottom("status")
        .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(egui::Margin::symmetric(12, 6)))
        .show(ui, |ui| status_bar(app, ui));
    egui::CentralPanel::default().frame(egui::Frame::new().inner_margin(egui::Margin::same(14))).show(
        ui,
        |ui| match app.tab {
            Tab::Play => tabs::play::ui(app, ui),
            Tab::Mods => tabs::mods::ui(app, ui),
            Tab::Versions => tabs::versions::ui(app, ui),
            Tab::Settings => tabs::settings::ui(app, ui),
            Tab::About => tabs::about::ui(app, ui),
        },
    );
    confirm_dialog(app, ui.ctx());
}

fn shortcuts(app: &mut App, ctx: &egui::Context) {
    let keys = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5];
    for (key, tab) in keys.into_iter().zip(Tab::ALL) {
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, key))) {
            app.tab = tab;
        }
    }
    if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::NONE, Key::F5))) {
        app.play();
    }
    if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::F))) {
        app.tab = Tab::Mods;
        app.mods_view = ModsView::Browse;
        app.focus_search = true;
    }
    if app.confirm.is_some() && ctx.input(|i| i.key_pressed(Key::Escape)) {
        app.confirm = None;
    }
}

fn tab_bar(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("castanea").size(20.0).strong().color(theme::ACCENT));
        ui.add_space(16.0);
        for (n, tab) in Tab::ALL.into_iter().enumerate() {
            let text = RichText::new(app.tr.get(tab.key())).size(16.0);
            let response = ui.selectable_label(app.tab == tab, text).on_hover_text(format!("Ctrl+{}", n + 1));
            if response.clicked() {
                app.tab = tab;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if app.settings.offline {
                ui.label(RichText::new(app.tr.get("status.offline")).color(theme::WARN));
            }
            if let Some(game) = &app.game {
                ui.label(
                    RichText::new(app.tr.fill("status.running", &[("profile", &game.profile)])).color(theme::GOOD),
                );
            }
        });
    });
}

fn status_bar(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let progress = app.jobs.progress.lock().ok().and_then(|p| p.clone());
        if let Some(progress) = progress {
            let fraction = progress.total.map(|t| progress.done as f32 / t.max(1) as f32);
            let text = format!("{} · {}", progress.label, megabytes(progress.done, progress.total));
            match fraction {
                Some(f) => ui.add(egui::ProgressBar::new(f).desired_width(220.0).text(text)),
                None => ui.add(egui::Spinner::new()).on_hover_text(text),
            };
            if ui.button(app.tr.get("common.cancel")).clicked() {
                app.jobs.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        } else if app.jobs.running > 0 {
            ui.add(egui::Spinner::new());
            ui.label(RichText::new(app.tr.get("status.working")).color(theme::MUTED));
        }
        let Some(note) = app.notes.last() else {
            ui.label(RichText::new(app.tr.get("status.ready")).color(theme::MUTED));
            return;
        };
        let (text, color) = match note {
            Note::Info(text) => (text.clone(), theme::GOOD),
            Note::Error(text) => (text.clone(), theme::BAD),
        };
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("✕").on_hover_text(app.tr.get("common.dismiss")).clicked() {
                app.notes.pop();
            }
            ui.add(egui::Label::new(RichText::new(text).color(color)).truncate());
        });
    });
}

/// `1.5 MB`, `12 KB`; with a total, `done / total`.
pub fn megabytes(done: u64, total: Option<u64>) -> String {
    let size = |b: u64| {
        if b < 1 << 20 { format!("{} KB", b.div_ceil(1024)) } else { format!("{:.1} MB", b as f64 / 1_048_576.0) }
    };
    match total {
        Some(total) => format!("{} / {}", size(done), size(total)),
        None => size(done),
    }
}

fn confirm_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(confirm) = &app.confirm else { return };
    let (title, body, yes) = match confirm {
        Confirm::EnableMod { file_name } => {
            let body = app
                .mods
                .iter()
                .find(|m| &m.file_name == file_name)
                .map(|m| tabs::mods::declared_text(app, m))
                .unwrap_or_default();
            (app.tr.get("mods.enable_title").to_owned(), body, app.tr.get("mods.enable").to_owned())
        }
        Confirm::RemoveMod { file_name } => (
            app.tr.get("mods.remove_title").to_owned(),
            app.tr.fill("mods.remove_body", &[("name", file_name)]),
            app.tr.get("common.remove").to_owned(),
        ),
        Confirm::RemoveBuild { id } => (
            app.tr.get("versions.remove_title").to_owned(),
            app.tr.fill("versions.remove_body", &[("version", id)]),
            app.tr.get("common.remove").to_owned(),
        ),
        Confirm::RemoveProfile { id } => (
            app.tr.get("profile.remove_title").to_owned(),
            app.tr.fill("profile.remove_body", &[("profile", id)]),
            app.tr.get("common.remove").to_owned(),
        ),
    };
    let mut answer = None;
    egui::Window::new(title).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(
        ctx,
        |ui| {
            ui.set_max_width(460.0);
            ui.label(body);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let yes = ui.add(egui::Button::new(RichText::new(yes).strong()).fill(theme::ACCENT_DARK));
                if yes.clicked() {
                    answer = Some(true);
                }
                if ui.button(app.tr.get("common.cancel")).clicked() {
                    answer = Some(false);
                }
            });
        },
    );
    match answer {
        Some(true) => {
            if let Some(confirm) = app.confirm.take() {
                apply(app, confirm);
            }
        }
        Some(false) => app.confirm = None,
        None => {}
    }
}

fn apply(app: &mut App, confirm: Confirm) {
    let mods_dir = app.mods_dir();
    let result = match confirm {
        Confirm::EnableMod { file_name } => match app.mods.iter().find(|m| m.file_name == file_name) {
            Some(package) => {
                let mut wanted = checks::dependencies_of(&app.mods, package.id());
                wanted.push(package);
                wanted
                    .into_iter()
                    .filter(|m| !m.enabled)
                    .try_for_each(|m| installed::set_enabled(&mods_dir, m, true).map(|_| ()))
            }
            None => Ok(()),
        },
        Confirm::RemoveMod { file_name } => match app.mods.iter().find(|m| m.file_name == file_name) {
            Some(package) => installed::remove(package),
            None => Ok(()),
        },
        Confirm::RemoveBuild { id } => {
            let paths = app.paths.clone();
            let result = app.registry.remove(&paths, &id).map(|_| ());
            app.save_registry();
            result
        }
        Confirm::RemoveProfile { id } => {
            let result = app.profiles.remove(&id).map(|_| ());
            app.save_profiles();
            let first = app.profiles.profiles[0].id.clone();
            app.set_profile(first);
            result
        }
    };
    if let Err(error) = result {
        app.error(error.to_string());
    }
    app.reload_mods();
}
