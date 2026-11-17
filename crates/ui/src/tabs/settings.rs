//! The Settings tab: language, offline mode, game sources and token, mod indexes, folders.

use castanea_launcher_core::install::token;
use castanea_launcher_core::settings::SourceConfig;
use eframe::egui::{self, RichText};

use crate::app::{App, open_folder};
use crate::i18n::LANGUAGES;
use crate::theme;

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        general(app, ui);
        ui.add_space(12.0);
        sources(app, ui);
        ui.add_space(12.0);
        indexes(app, ui);
        ui.add_space(12.0);
        folders(app, ui);
    });
}

fn general(app: &mut App, ui: &mut egui::Ui) {
    ui.label(RichText::new(app.tr.get("settings.general")).heading());
    egui::Grid::new("general").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
        ui.label(app.tr.get("settings.language"));
        let mut language = app.settings.language.clone();
        let shown = LANGUAGES
            .iter()
            .find(|(c, _)| *c == language)
            .map_or(app.tr.get("settings.system_language"), |(_, n)| n)
            .to_owned();
        egui::ComboBox::from_id_salt("language").selected_text(shown).show_ui(ui, |ui| {
            ui.selectable_value(&mut language, String::new(), app.tr.get("settings.system_language"));
            for (code, name) in LANGUAGES {
                ui.selectable_value(&mut language, code.to_owned(), name);
            }
        });
        if language != app.settings.language {
            app.settings.language = language;
            app.save_settings();
            app.reload_language();
        }
        ui.end_row();

        ui.label(app.tr.get("settings.offline"));
        let mut offline = app.settings.offline;
        if ui.checkbox(&mut offline, app.tr.get("settings.offline_hint")).changed() {
            app.settings.offline = offline;
            app.save_settings();
            app.index_loaded = false;
            app.releases_loaded = false;
        }
        ui.end_row();

        ui.label(app.tr.get("settings.log_lines"));
        let mut lines = app.settings.log_lines;
        if ui.add(egui::DragValue::new(&mut lines).range(100..=100_000)).changed() {
            app.settings.log_lines = lines;
            app.save_settings();
        }
        ui.end_row();
    });
}

fn sources(app: &mut App, ui: &mut egui::Ui) {
    ui.label(RichText::new(app.tr.get("settings.sources")).heading());
    let mut remove = None;
    for (n, source) in app.settings.sources.iter().enumerate() {
        let SourceConfig::Github { repo } = source;
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("GitHub {repo}")).monospace());
            if ui.small_button(app.tr.get("common.remove")).clicked() {
                remove = Some(n);
            }
        });
    }
    if let Some(n) = remove {
        app.settings.sources.remove(n);
        app.save_settings();
        app.releases_loaded = false;
    }
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut app.new_source).hint_text("owner/repository").desired_width(240.0));
        let valid = app.new_source.trim().split('/').filter(|p| !p.is_empty()).count() == 2;
        if ui.add_enabled(valid, egui::Button::new(app.tr.get("common.add"))).clicked() {
            app.settings.sources.push(SourceConfig::Github { repo: app.new_source.trim().to_owned() });
            app.new_source.clear();
            app.save_settings();
            app.releases_loaded = false;
        }
    });
    ui.add_space(6.0);
    ui.label(app.tr.get("settings.token"));
    match token::origin(&app.paths) {
        Some(origin) => {
            ui.label(RichText::new(app.tr.fill("settings.token_from", &[("origin", &origin)])).color(theme::GOOD))
        }
        None => ui.label(RichText::new(app.tr.get("settings.token_none")).color(theme::MUTED)),
    };
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.token_input)
                .password(true)
                .hint_text("github_pat_...")
                .desired_width(240.0),
        );
        if ui.button(app.tr.get("settings.token_save")).clicked() {
            let result = token::save(&app.paths, &app.token_input);
            app.token_input.clear();
            match result {
                Ok(()) => {
                    app.releases_loaded = false;
                    let text = app.tr.get("settings.token_saved").to_owned();
                    app.info(text);
                }
                Err(error) => app.error(error.to_string()),
            }
        }
    });
    ui.label(RichText::new(app.tr.get("settings.token_hint")).small().color(theme::MUTED));
}

fn indexes(app: &mut App, ui: &mut egui::Ui) {
    ui.label(RichText::new(app.tr.get("settings.indexes")).heading());
    let mut remove = None;
    for (n, location) in app.settings.indexes.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(RichText::new(location).monospace());
            if ui.small_button(app.tr.get("common.remove")).clicked() {
                remove = Some(n);
            }
        });
    }
    if let Some(n) = remove {
        app.settings.indexes.remove(n);
        app.save_settings();
        app.index_loaded = false;
    }
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut app.new_index).hint_text("https://.../index.json").desired_width(320.0));
        let text = app.new_index.trim().to_owned();
        if ui.add_enabled(!text.is_empty(), egui::Button::new(app.tr.get("common.add"))).clicked() {
            if text.starts_with("http://") {
                let error = app.tr.get("settings.index_https").to_owned();
                app.error(error);
            } else {
                app.settings.indexes.push(text);
                app.new_index.clear();
                app.save_settings();
                app.index_loaded = false;
            }
        }
        if ui.button(app.tr.get("settings.index_file")).clicked()
            && let Some(file) = rfd::FileDialog::new().add_filter("json", &["json"]).pick_file()
        {
            app.settings.indexes.push(file.display().to_string());
            app.save_settings();
            app.index_loaded = false;
        }
    });
    ui.label(RichText::new(app.tr.get("settings.indexes_hint")).small().color(theme::MUTED));
}

fn folders(app: &mut App, ui: &mut egui::Ui) {
    ui.label(RichText::new(app.tr.get("settings.folders")).heading());
    let rows = [
        ("settings.folder_config", app.paths.config.clone()),
        ("settings.folder_data", app.paths.data.clone()),
        ("settings.folder_cache", app.paths.cache.clone()),
    ];
    egui::Grid::new("folders").num_columns(3).spacing([16.0, 6.0]).show(ui, |ui| {
        for (key, path) in rows {
            ui.label(app.tr.get(key));
            ui.label(RichText::new(path.display().to_string()).monospace().small());
            if ui.small_button(app.tr.get("versions.open")).clicked()
                && let Err(error) = open_folder(&path)
            {
                app.error(format!("{}: {error}", path.display()));
            }
            ui.end_row();
        }
    });
}
