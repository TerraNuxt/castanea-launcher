//! Settings in one column: General, Game (what used to be per profile), Mod indexes, and Advanced (closed by
//! default: a local build folder for people who build the game, the release server and beta builds, log length, the launcher's folders).

use castanea_launcher_core::install::{Platform, ReleaseApi};
use castanea_launcher_core::settings::{DEFAULT_RELEASE_URL, Graphics};
use eframe::egui;

use crate::app::{App, open_or_note};
use crate::game::Update;
use crate::i18n::LANGUAGES;
use crate::pages;
use crate::theme::{layout, space};
use crate::widgets::{self, Button, Kind, icon_button, setting_row};
use crate::{icons, probe};

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    let title = app.tr.get("nav.settings").to_owned();
    widgets::page_header(ui, &title, |_| {});
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.set_max_width(layout::READABLE);
        group(ui, "settings:general", |ui| general(app, ui));
        group(ui, "settings:game", |ui| game(app, ui));
        group(ui, "settings:indexes", |ui| indexes(app, ui));
        group(ui, "settings:storage", |ui| pages::storage::ui(app, ui));
        let mut open = app.advanced_open;
        let toggle = widgets::rows::expander(ui, &mut open, app.tr.get("settings.advanced"));
        if (toggle.clicked() && open) || std::mem::take(&mut app.reveal_advanced) {
            toggle.scroll_to_me(Some(egui::Align::Min));
        }
        app.advanced_open = open;
        if open {
            group(ui, "settings:advanced", |ui| advanced(app, ui));
        }
        ui.add_space(space::XL);
    });
}

fn group(ui: &mut egui::Ui, name: &str, add: impl FnOnce(&mut egui::Ui)) {
    let top = ui.cursor().top();
    add(ui);
    let rect = egui::Rect::from_x_y_ranges(ui.max_rect().x_range(), top..=ui.cursor().top());
    probe::mark(ui.ctx(), name, rect);
}

fn general(app: &mut App, ui: &mut egui::Ui) {
    widgets::section(ui, app.tr.get("settings.general"));
    setting_row(ui, app.tr.get("settings.language").to_owned(), None, |ui| {
        let mut language = app.settings.language.clone();
        let shown = LANGUAGES
            .iter()
            .find(|(c, _)| *c == language)
            .map_or(app.tr.get("settings.system_language"), |(_, n)| n)
            .to_owned();
        egui::ComboBox::from_id_salt("language").width(layout::FIELD).selected_text(shown).show_ui(ui, |ui| {
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
    });
    let hint = app.tr.get("settings.offline_hint").to_owned();
    setting_row(ui, app.tr.get("settings.offline").to_owned(), Some(&hint), |ui| {
        let mut offline = app.settings.offline;
        if widgets::switch_named(ui, &mut offline, app.tr.get("settings.offline")).changed() {
            app.settings.offline = offline;
            app.save_settings();
            app.reset_launcher_check();
            app.index_loaded = false;
            if offline {
                if !matches!(app.update, Update::Downloading { .. } | Update::Applying { .. }) {
                    app.update = Update::Offline;
                }
            } else {
                app.check_updates(ui.ctx());
            }
        }
    });
    let hint = app.tr.get("settings.launcher_update_hint").to_owned();
    setting_row(ui, app.tr.get("settings.launcher_update").to_owned(), Some(&hint), |ui| {
        let mut on = app.settings.launcher_update;
        if widgets::switch_named(ui, &mut on, app.tr.get("settings.launcher_update")).changed() {
            app.settings.launcher_update = on;
            app.save_settings();
            app.reset_launcher_check();
        }
    });
}

/// A text setting that is saved as soon as it changes.
fn text_row(
    app: &mut App,
    ui: &mut egui::Ui,
    key: &str,
    hint: Option<&str>,
    placeholder: &str,
    get: fn(&mut App) -> &mut String,
) {
    let label = app.tr.get(key).to_owned();
    let hint = hint.map(|k| app.tr.get(k).to_owned());
    let changed = setting_row(ui, label.clone(), hint.as_deref(), |ui| {
        widgets::text_field(ui, get(app), placeholder, layout::FIELD).changed()
    });
    if changed {
        app.save_settings();
    }
}

fn game(app: &mut App, ui: &mut egui::Ui) {
    widgets::section(ui, app.tr.get("settings.game"));
    text_row(app, ui, "settings.player", None, "", |a| &mut a.settings.game.player_name);
    text_row(app, ui, "settings.window", Some("settings.window_hint"), "1280x720", |a| &mut a.settings.game.window);
    text_row(app, ui, "settings.extra", Some("settings.extra_hint"), "", |a| &mut a.settings.game.extra_args);
    environment(app, ui);
    let folder = app.paths.game();
    let path = folder.display().to_string();
    setting_row(ui, app.tr.get("settings.game_folder").to_owned(), Some(&path), |ui| {
        if Button::new(Kind::Secondary, app.tr.get("common.open")).show(ui).clicked() {
            open_or_note(app, &folder);
        }
    });
}

fn environment(app: &mut App, ui: &mut egui::Ui) {
    let hint = app.tr.get("settings.env_hint").to_owned();
    let label = app.tr.get("settings.env").to_owned();
    let mut changed = false;
    let mut remove = None;
    setting_row(ui, label.clone(), Some(&hint), |ui| {
        let half = (layout::FIELD - space::S) / 2.0;
        for (n, (key, value)) in app.settings.game.env.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                changed |= widgets::text_field(ui, key, "NAME", half).changed();
                changed |= widgets::text_field(ui, value, "value", half).changed();
                if icon_button(ui, icons::X, &app.tr.fill("settings.env_remove", &[("name", key)])).clicked() {
                    remove = Some(n);
                }
            });
        }
        if Button::new(Kind::Secondary, app.tr.get("settings.env_add")).show(ui).clicked() {
            app.settings.game.env.push((String::new(), String::new()));
        }
    });
    if let Some(n) = remove {
        app.settings.game.env.remove(n);
        changed = true;
    }
    if changed {
        app.save_settings();
    }
}

fn indexes(app: &mut App, ui: &mut egui::Ui) {
    widgets::section(ui, app.tr.get("settings.indexes"));
    let mut remove = None;
    for (n, location) in app.settings.indexes.iter().enumerate() {
        ui.add_space(space::S);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_button(ui, icons::TRASH, &app.tr.fill("settings.index_remove", &[("location", location)]))
                    .clicked()
                {
                    remove = Some(n);
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    widgets::one_line(ui, widgets::text(location));
                });
            });
        });
        ui.add_space(space::S);
        widgets::hairline(ui);
    }
    if let Some(n) = remove {
        app.settings.indexes.remove(n);
        app.save_settings();
        app.index_loaded = false;
    }
    ui.add_space(space::M);
    ui.horizontal(|ui| {
        let add = app.tr.get("common.add").to_owned();
        widgets::text_field(ui, &mut app.new_index, "https://…/index.json", layout::FIELD + layout::LABEL / 2.0);
        let text = app.new_index.trim().to_owned();
        if Button::new(Kind::Secondary, &add).enabled(!text.is_empty()).show(ui).clicked() {
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
        if Button::new(Kind::Secondary, app.tr.get("settings.index_file")).show(ui).clicked()
            && let Some(file) = rfd::FileDialog::new().add_filter("json", &["json"]).pick_file()
        {
            app.settings.indexes.push(file.display().to_string());
            app.save_settings();
            app.index_loaded = false;
        }
    });
    ui.add_space(space::XS);
    widgets::wrapped(ui, widgets::small(app.tr.get("settings.indexes_hint")));
}

/// Auto (the launcher checks Vulkan first), or one backend for good. A change forgets what the launcher learned.
fn graphics(app: &mut App, ui: &mut egui::Ui) {
    let hint = app.tr.get("settings.graphics_hint").to_owned();
    setting_row(ui, app.tr.get("settings.graphics").to_owned(), Some(&hint), |ui| {
        let mut current = Graphics::ALL.iter().position(|g| *g == app.settings.game.graphics).unwrap_or(0);
        let choices = [app.tr.get("settings.graphics_auto"), "Vulkan", "wgpu"];
        if widgets::segmented(ui, &mut current, &choices) {
            app.settings.game.graphics = Graphics::ALL[current];
            app.save_settings();
            castanea_launcher_core::backend::forget(&app.paths);
        }
    });
}

fn advanced(app: &mut App, ui: &mut egui::Ui) {
    widgets::hairline(ui);
    graphics(app, ui);
    let hint = match app.dev_folder() {
        Some(folder) => app.tr.fill("settings.dev_build_set", &[("folder", &folder.display().to_string())]),
        None => app.tr.get("settings.dev_build_hint").to_owned(),
    };
    setting_row(ui, app.tr.get("settings.dev_build").to_owned(), Some(&hint), |ui| {
        ui.horizontal(|ui| {
            if Button::new(Kind::Secondary, app.tr.get("settings.dev_build_pick")).show(ui).clicked()
                && let Some(folder) = rfd::FileDialog::new().pick_folder()
            {
                app.settings.dev_build = Some(folder);
                set_dev_build(app, ui.ctx());
            }
            let set = app.settings.dev_build.is_some();
            if Button::new(Kind::Secondary, app.tr.get("settings.dev_build_clear")).enabled(set).show(ui).clicked() {
                app.settings.dev_build = None;
                set_dev_build(app, ui.ctx());
            }
        });
    });
    release_server(app, ui);
    setting_row(ui, app.tr.get("settings.log_lines").to_owned(), None, |ui| {
        let mut lines = app.settings.log_lines;
        if ui.add(egui::DragValue::new(&mut lines).range(100..=100_000)).changed() {
            app.settings.log_lines = lines;
            app.save_settings();
        }
    });
    let rows = [
        ("settings.folder_config", app.paths.config.clone()),
        ("settings.folder_data", app.paths.data.clone()),
        ("settings.folder_cache", app.paths.cache.clone()),
    ];
    for (key, path) in rows {
        let shown = path.display().to_string();
        setting_row(ui, app.tr.get(key).to_owned(), Some(&shown), |ui| {
            let name = format!("{} {}", app.tr.get("common.open"), app.tr.get(key));
            if Button::new(Kind::Secondary, app.tr.get("common.open")).name(name).show(ui).clicked() {
                open_or_note(app, &path);
            }
        });
    }
}

fn set_dev_build(app: &mut App, ctx: &egui::Context) {
    app.save_settings();
    app.refresh_installed();
    app.update = Update::Idle;
    app.check_updates(ctx);
    app.reload_mods();
}

/// The release server address and the beta switch.
fn release_server(app: &mut App, ui: &mut egui::Ui) {
    let hint = app.tr.get("settings.release_url_hint").to_owned();
    setting_row(ui, app.tr.get("settings.release_url").to_owned(), Some(&hint), |ui| {
        ui.horizontal(|ui| {
            let width = layout::FIELD - layout::LABEL / 2.0;
            widgets::text_field(ui, &mut app.release_url_input, DEFAULT_RELEASE_URL, width);
            if Button::new(Kind::Secondary, app.tr.get("common.save")).show(ui).clicked() {
                let url = app.release_url_input.trim().to_owned();
                match ReleaseApi::new(&url, Platform::LinuxX86_64, false) {
                    Ok(_) => {
                        app.settings.release_url = if url.is_empty() { DEFAULT_RELEASE_URL.to_owned() } else { url };
                        app.release_url_input = app.settings.release_url.clone();
                        app.save_settings();
                        app.update = Update::Idle;
                        app.check_updates(ui.ctx());
                    }
                    Err(error) => app.error(error.to_string()),
                }
            }
        });
    });
    let hint = app.tr.get("settings.beta_hint").to_owned();
    setting_row(ui, app.tr.get("settings.beta").to_owned(), Some(&hint), |ui| {
        let mut beta = app.settings.beta;
        if widgets::switch_named(ui, &mut beta, app.tr.get("settings.beta")).changed() {
            app.settings.beta = beta;
            app.save_settings();
            app.reset_launcher_check();
            app.update = Update::Idle;
            app.check_updates(ui.ctx());
        }
    });
    let hint = app.tr.get("settings.allow_unsigned_hint").to_owned();
    setting_row(ui, app.tr.get("settings.allow_unsigned").to_owned(), Some(&hint), |ui| {
        let mut on = app.settings.allow_unsigned;
        if widgets::switch_named(ui, &mut on, app.tr.get("settings.allow_unsigned")).changed() {
            app.settings.allow_unsigned = on;
            app.save_settings();
        }
    });
}
