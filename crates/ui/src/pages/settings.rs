//! Settings: general (language, offline mode, window frame, log lines), game sources and token, mod indexes, folders.
//! Two columns of cards when wide, one when narrow.

use castanea_launcher_core::install::token;
use castanea_launcher_core::settings::SourceConfig;
use eframe::egui::{self, RichText};

use crate::app::{App, open_or_note};
use crate::i18n::LANGUAGES;
use crate::theme::{color, layout, size, space};
use crate::widgets::{self, Card, Kind, Pill, icon_button};
use crate::{icons, probe};

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    let title = app.tr.get("nav.settings").to_owned();
    widgets::page_header(ui, &title, |_| {});
    ui.add_space(space::M);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        if ui.available_width() >= layout::WIDE {
            ui.columns(2, |cols| {
                section(&mut cols[0], "settings:general", |ui| general(app, ui));
                section(&mut cols[0], "settings:folders", |ui| folders(app, ui));
                section(&mut cols[1], "settings:sources", |ui| sources(app, ui));
                section(&mut cols[1], "settings:indexes", |ui| indexes(app, ui));
            });
        } else {
            section(ui, "settings:general", |ui| general(app, ui));
            section(ui, "settings:sources", |ui| sources(app, ui));
            section(ui, "settings:indexes", |ui| indexes(app, ui));
            section(ui, "settings:folders", |ui| folders(app, ui));
        }
    });
}

fn section(ui: &mut egui::Ui, name: &str, add: impl FnOnce(&mut egui::Ui)) {
    let response = Card::new().padding(space::XL).show(ui, add);
    probe::mark(ui.ctx(), name, response.response.rect);
    ui.add_space(space::L);
}

fn row_label(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).color(color::TEXT_MUTED));
}

fn general(app: &mut App, ui: &mut egui::Ui) {
    widgets::section_title(ui, icons::GEAR, app.tr.get("settings.general"));
    ui.add_space(space::S);
    egui::Grid::new("general").num_columns(2).spacing([space::L, space::M]).show(ui, |ui| {
        row_label(ui, app.tr.get("settings.language"));
        let mut language = app.settings.language.clone();
        let shown = LANGUAGES
            .iter()
            .find(|(c, _)| *c == language)
            .map_or(app.tr.get("settings.system_language"), |(_, n)| n)
            .to_owned();
        egui::ComboBox::from_id_salt("language")
            .width(180.0)
            .selected_text(format!("{}  {shown}", icons::GLOBE))
            .show_ui(ui, |ui| {
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

        row_label(ui, app.tr.get("settings.offline"));
        let mut offline = app.settings.offline;
        if widgets::switch(ui, &mut offline, app.tr.get("settings.offline_hint")).changed() {
            app.settings.offline = offline;
            app.save_settings();
            app.index_loaded = false;
            app.releases_loaded = false;
        }
        ui.end_row();

        row_label(ui, app.tr.get("settings.frame"));
        let mut system = app.settings.system_frame;
        if widgets::switch(ui, &mut system, app.tr.get("settings.frame_hint")).changed() {
            app.settings.system_frame = system;
            app.save_settings();
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Decorations(system));
        }
        ui.end_row();

        row_label(ui, app.tr.get("settings.log_lines"));
        let mut lines = app.settings.log_lines;
        if ui.add(egui::DragValue::new(&mut lines).range(100..=100_000)).changed() {
            app.settings.log_lines = lines;
            app.save_settings();
        }
        ui.end_row();
    });
}

fn sources(app: &mut App, ui: &mut egui::Ui) {
    widgets::section_title(ui, icons::GITHUB, app.tr.get("settings.sources"));
    ui.add_space(space::S);
    let mut remove = None;
    for (n, source) in app.settings.sources.iter().enumerate() {
        let SourceConfig::Github { repo } = source;
        ui.horizontal(|ui| {
            widgets::one_line(ui, RichText::new(format!("GitHub {repo}")).monospace());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_button(ui, icons::TRASH, &format!("{} {repo}", app.tr.get("common.remove"))).clicked() {
                    remove = Some(n);
                }
            });
        });
    }
    if let Some(n) = remove {
        app.settings.sources.remove(n);
        app.save_settings();
        app.releases_loaded = false;
    }
    ui.horizontal(|ui| {
        let width = (ui.available_width() - 90.0).max(140.0);
        ui.add(egui::TextEdit::singleline(&mut app.new_source).hint_text("owner/repository").desired_width(width));
        let valid = app.new_source.trim().split('/').filter(|p| !p.is_empty()).count() == 2;
        if Pill::new(Kind::Ghost, app.tr.get("common.add")).icon(icons::PLUS).enabled(valid).show(ui).clicked() {
            app.settings.sources.push(SourceConfig::Github { repo: app.new_source.trim().to_owned() });
            app.new_source.clear();
            app.save_settings();
            app.releases_loaded = false;
        }
    });
    ui.add_space(space::M);
    ui.label(widgets::strong(app.tr.get("settings.token"), size::BODY));
    match token::origin(&app.paths) {
        Some(origin) => {
            ui.label(RichText::new(app.tr.fill("settings.token_from", &[("origin", &origin)])).color(color::PLAY))
        }
        None => ui.label(widgets::muted(app.tr.get("settings.token_none"))),
    };
    ui.horizontal(|ui| {
        let width = (ui.available_width() - 150.0).max(140.0);
        ui.add(
            egui::TextEdit::singleline(&mut app.token_input)
                .password(true)
                .hint_text("github_pat_...")
                .desired_width(width),
        );
        if Pill::new(Kind::Ghost, app.tr.get("settings.token_save")).show(ui).clicked() {
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
    ui.add(egui::Label::new(widgets::faint(app.tr.get("settings.token_hint"))).wrap());
}

fn indexes(app: &mut App, ui: &mut egui::Ui) {
    widgets::section_title(ui, icons::BOOK_OPEN, app.tr.get("settings.indexes"));
    ui.add_space(space::S);
    let mut remove = None;
    for (n, location) in app.settings.indexes.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_button(ui, icons::TRASH, &format!("{} {location}", app.tr.get("common.remove"))).clicked() {
                    remove = Some(n);
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    widgets::one_line(ui, RichText::new(location).monospace().size(size::SMALL));
                });
            });
        });
    }
    if let Some(n) = remove {
        app.settings.indexes.remove(n);
        app.save_settings();
        app.index_loaded = false;
    }
    ui.horizontal(|ui| {
        let width = (ui.available_width() - 90.0).max(140.0);
        ui.add(egui::TextEdit::singleline(&mut app.new_index).hint_text("https://.../index.json").desired_width(width));
        let text = app.new_index.trim().to_owned();
        if Pill::new(Kind::Ghost, app.tr.get("common.add"))
            .icon(icons::PLUS)
            .enabled(!text.is_empty())
            .show(ui)
            .clicked()
        {
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
    });
    if Pill::new(Kind::Ghost, app.tr.get("settings.index_file")).icon(icons::FOLDER_OPEN).show(ui).clicked()
        && let Some(file) = rfd::FileDialog::new().add_filter("json", &["json"]).pick_file()
    {
        app.settings.indexes.push(file.display().to_string());
        app.save_settings();
        app.index_loaded = false;
    }
    ui.add(egui::Label::new(widgets::faint(app.tr.get("settings.indexes_hint"))).wrap());
}

fn folders(app: &mut App, ui: &mut egui::Ui) {
    widgets::section_title(ui, icons::FOLDER_OPEN, app.tr.get("settings.folders"));
    ui.add_space(space::S);
    let rows = [
        ("settings.folder_config", app.paths.config.clone()),
        ("settings.folder_data", app.paths.data.clone()),
        ("settings.folder_cache", app.paths.cache.clone()),
    ];
    for (key, path) in rows {
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_button(
                    ui,
                    icons::ARROW_SQUARE_OUT,
                    &format!("{} {}", app.tr.get("versions.open"), app.tr.get(key)),
                )
                .clicked()
                {
                    open_or_note(app, &path);
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(widgets::strong(app.tr.get(key), size::BODY));
                        widgets::one_line(
                            ui,
                            RichText::new(path.display().to_string())
                                .monospace()
                                .size(size::CAPTION)
                                .color(color::TEXT_FAINT),
                        );
                    });
                });
            });
        });
        ui.add_space(space::XS);
    }
}
