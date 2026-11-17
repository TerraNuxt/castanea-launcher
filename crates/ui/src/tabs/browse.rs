//! Mods → Browse: search and filter the index, a details page, install with progress.

use castanea_launcher_core::index::install as index_install;
use castanea_launcher_core::index::{IndexMod, IndexVersion};
use castanea_launcher_core::version::Compat;
use eframe::egui::{self, RichText};

use crate::app::{App, ImageState, Tab};
use crate::shell::megabytes;
use crate::theme;

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    if app.settings.indexes.is_empty() {
        ui.label(RichText::new(app.tr.get("browse.no_index")).color(theme::MUTED));
        if ui.button(app.tr.get("browse.go_settings")).clicked() {
            app.tab = Tab::Settings;
        }
        return;
    }
    if !app.index_loaded {
        app.load_index(ui.ctx());
    }
    filters(app, ui);
    for warning in &app.index_warnings {
        ui.label(RichText::new(warning).small().color(theme::WARN));
    }
    let game = app.game_version();
    let shown: Vec<IndexMod> = app
        .index
        .mods
        .iter()
        .filter(|m| m.matches(&app.browse.query))
        .filter(|m| app.browse.tag.as_ref().is_none_or(|t| m.tags.contains(t)))
        .filter(|m| !app.browse.compatible_only || m.compat(game.as_deref()) != Compat::No)
        .cloned()
        .collect();
    let mut select = None;
    let mut install = None;
    egui::Panel::left("browse_list").resizable(true).default_size(330.0).show(ui, |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if shown.is_empty() {
                ui.label(RichText::new(app.tr.get("browse.nothing")).color(theme::MUTED));
            }
            for item in &shown {
                let selected = app.browse.selected.as_deref() == Some(&item.id);
                let response = list_row(app, ui, item, selected, game.as_deref());
                if response.clicked() {
                    select = Some(item.id.clone());
                }
            }
        });
    });
    egui::CentralPanel::default().frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 0))).show(
        ui,
        |ui| {
            let selected = app.browse.selected.clone().and_then(|id| app.index.get(&id).cloned());
            match selected {
                Some(item) => details(app, ui, &item, game.as_deref(), &mut install),
                None => {
                    ui.label(RichText::new(app.tr.get("browse.pick")).color(theme::MUTED));
                }
            }
        },
    );
    if let Some(id) = select {
        app.browse.selected = Some(id);
    }
    if let Some(id) = install {
        app.install_mod(ui.ctx(), &id);
    }
}

fn filters(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let search = ui.add(
            egui::TextEdit::singleline(&mut app.browse.query)
                .hint_text(app.tr.get("browse.search"))
                .desired_width(240.0),
        );
        if app.focus_search {
            search.request_focus();
            app.focus_search = false;
        }
        let tags = app.index.tags();
        let current = app.browse.tag.clone().unwrap_or_else(|| app.tr.get("browse.all_tags").to_owned());
        egui::ComboBox::from_id_salt("tag").selected_text(current).show_ui(ui, |ui| {
            if ui.selectable_label(app.browse.tag.is_none(), app.tr.get("browse.all_tags")).clicked() {
                app.browse.tag = None;
            }
            for (tag, count) in tags {
                if ui.selectable_label(app.browse.tag.as_ref() == Some(&tag), format!("{tag} ({count})")).clicked() {
                    app.browse.tag = Some(tag);
                }
            }
        });
        ui.checkbox(&mut app.browse.compatible_only, app.tr.get("browse.compatible_only"));
        if ui.button(app.tr.get("common.refresh")).clicked() {
            app.load_index(ui.ctx());
        }
    });
}

fn compat_label(app: &App, compat: Compat) -> RichText {
    match compat {
        Compat::Yes => RichText::new(app.tr.get("compat.yes")).color(theme::GOOD),
        Compat::No => RichText::new(app.tr.get("compat.no")).color(theme::BAD),
        Compat::Unknown => RichText::new(app.tr.get("compat.unknown")).color(theme::MUTED),
    }
}

fn list_row(app: &App, ui: &mut egui::Ui, item: &IndexMod, selected: bool, game: Option<&str>) -> egui::Response {
    let frame = if selected { theme::card().stroke(egui::Stroke::new(1.0, theme::ACCENT)) } else { theme::card() };
    let inner = frame.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(RichText::new(&item.name).strong());
            if app.mods.iter().any(|m| m.id() == item.id) {
                ui.label(RichText::new(app.tr.get("browse.installed")).small().color(theme::GOOD));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(compat_label(app, item.compat(game)).small());
            });
        });
        ui.add(egui::Label::new(RichText::new(&item.summary).small().color(theme::MUTED)).truncate());
    });
    ui.add_space(4.0);
    inner.response.interact(egui::Sense::click())
}

fn details(app: &mut App, ui: &mut egui::Ui, item: &IndexMod, game: Option<&str>, install: &mut Option<String>) {
    egui::ScrollArea::vertical().id_salt("details").auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal(|ui| {
            if let Some(icon) = &item.icon {
                image(app, ui, icon, 48.0);
            }
            ui.vertical(|ui| {
                ui.label(RichText::new(&item.name).heading());
                let authors = item.authors.join(", ");
                ui.label(RichText::new(format!("{} · {} · {}", item.id, authors, item.license)).color(theme::MUTED));
            });
        });
        ui.horizontal_wrapped(|ui| {
            for tag in &item.tags {
                ui.label(RichText::new(format!("#{tag}")).small().color(theme::ACCENT));
            }
            ui.label(RichText::new(app.tr.fill("mods.side", &[("side", &item.side)])).small().color(theme::MUTED));
            if item.has_code {
                ui.label(RichText::new(app.tr.get("mods.has_code")).small().color(theme::WARN))
                    .on_hover_text(app.tr.get("mods.has_code_hint"));
            }
        });
        ui.add_space(6.0);
        install_box(app, ui, item, game, install);
        ui.add_space(6.0);
        let text = if item.description.is_empty() { &item.summary } else { &item.description };
        ui.label(text);
        for url in [&item.homepage, &item.source].into_iter().flatten() {
            ui.hyperlink(url);
        }
        if !item.screenshots.is_empty() {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                for url in item.screenshots.clone() {
                    image(app, ui, &url, 220.0);
                }
            });
        }
        ui.add_space(8.0);
        ui.label(RichText::new(app.tr.get("browse.versions")).strong());
        for version in &item.versions {
            version_row(app, ui, version, game);
        }
    });
}

fn install_box(app: &mut App, ui: &mut egui::Ui, item: &IndexMod, game: Option<&str>, install: &mut Option<String>) {
    let Some(best) = item.best_version(game) else {
        ui.label(RichText::new(app.tr.get("browse.no_fitting_version")).color(theme::BAD));
        return;
    };
    let installed =
        app.mods.iter().find(|m| m.id() == item.id).and_then(|m| m.manifest.as_ref().ok()).map(|m| m.version.clone());
    theme::card().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            let label = match &installed {
                Some(v) if *v == best.version => None,
                Some(_) => Some(app.tr.fill("mods.update_to", &[("version", &best.version)])),
                None => Some(app.tr.fill("browse.install", &[("version", &best.version)])),
            };
            let busy = app.jobs.running > 0 && app.jobs.progress.lock().is_ok_and(|p| p.is_some());
            match label {
                Some(label) => {
                    let button = egui::Button::new(RichText::new(label).strong()).fill(theme::ACCENT_DARK);
                    if ui.add_enabled(!busy && !app.settings.offline, button).clicked() {
                        *install = Some(item.id.clone());
                    }
                }
                None => {
                    ui.label(RichText::new(app.tr.get("browse.up_to_date")).color(theme::GOOD));
                }
            }
            ui.label(RichText::new(megabytes(best.size, None)).color(theme::MUTED));
            ui.label(compat_label(app, castanea_launcher_core::version::compat(best.game.as_deref(), game)));
        });
        let missing = index_install::missing_dependencies(&app.index, item, best, &app.mods, game);
        if !missing.is_empty() {
            let list: Vec<String> = missing.iter().map(|(m, v)| format!("{} {}", m.id, v.version)).collect();
            ui.label(RichText::new(app.tr.fill("browse.needs", &[("list", &list.join(", "))])).color(theme::WARN));
        }
        ui.label(RichText::new(app.tr.get("browse.install_note")).small().color(theme::MUTED));
    });
}

fn version_row(app: &App, ui: &mut egui::Ui, version: &IndexVersion, game: Option<&str>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(&version.version).monospace());
        ui.label(RichText::new(&version.published).small().color(theme::MUTED));
        let range = version.game.clone().unwrap_or_else(|| "?".to_owned());
        ui.label(RichText::new(app.tr.fill("browse.game_range", &[("range", &range)])).small());
        ui.label(compat_label(app, castanea_launcher_core::version::compat(version.game.as_deref(), game)).small());
    });
    if !version.changelog.is_empty() {
        ui.label(RichText::new(&version.changelog).small().color(theme::MUTED));
    }
}

fn image(app: &mut App, ui: &mut egui::Ui, url: &str, size: f32) {
    app.request_image(ui.ctx(), url);
    match app.images.get(url) {
        Some(ImageState::Ready(bytes)) => {
            let uri = format!("bytes://{url}");
            ui.add(egui::Image::from_bytes(uri, bytes.clone()).max_width(size).max_height(size).corner_radius(4));
        }
        Some(ImageState::Failed) => {
            ui.label(RichText::new("🖼").size(size.min(32.0)).color(theme::MUTED));
        }
        _ => {
            ui.add(egui::Spinner::new());
        }
    }
}
