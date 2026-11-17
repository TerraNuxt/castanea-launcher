//! Mods → Browse: search and filter the index, a details card, install with progress. Narrow windows show the list
//! or the details with a Back button.

use castanea_launcher_core::index::install as index_install;
use castanea_launcher_core::index::{IndexMod, IndexVersion};
use castanea_launcher_core::version::Compat;
use eframe::egui::{self, RichText};

use crate::app::{App, ImageState, Page};
use crate::pages::mods::empty;
use crate::shell::megabytes;
use crate::theme::{self, color, font, layout, radius, size, space};
use crate::widgets::{self, Card, Kind, Pill, icon_button};
use crate::{icons, probe};

const LIST_WIDTH: f32 = 340.0;

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    if app.settings.indexes.is_empty() {
        empty(ui, icons::BOOK_OPEN, app.tr.get("browse.no_index"));
        ui.vertical_centered(|ui| {
            if Pill::new(Kind::Accent, app.tr.get("browse.go_settings")).icon(icons::GEAR).show(ui).clicked() {
                app.go(Page::Settings);
            }
        });
        return;
    }
    if !app.index_loaded {
        app.load_index(ui.ctx());
    }
    filters(app, ui);
    for warning in &app.index_warnings {
        ui.horizontal(|ui| {
            ui.label(widgets::icon(icons::WARNING, 14.0, color::WARN));
            ui.label(RichText::new(warning).size(size::SMALL).color(color::WARN));
        });
    }
    ui.add_space(space::S);
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
    let area = ui.available_rect_before_wrap();
    let narrow = area.width() < layout::NARROW;
    let selected = app.browse.selected.clone().and_then(|id| app.index.get(&id).cloned());
    let (list_rect, details_rect) = if narrow {
        (area, area)
    } else {
        let list = area.with_max_x(area.left() + LIST_WIDTH);
        (list, area.with_min_x(list.right() + space::L))
    };
    let mut select = None;
    let mut install = None;
    if !narrow || selected.is_none() {
        probe::mark(ui.ctx(), "browse:list", list_rect);
        ui.scope_builder(egui::UiBuilder::new().max_rect(list_rect), |ui| {
            egui::ScrollArea::vertical().id_salt("browse-list").auto_shrink([false, false]).show(ui, |ui| {
                if shown.is_empty() {
                    empty(ui, icons::SEARCH, app.tr.get("browse.nothing"));
                }
                for item in &shown {
                    let is_selected = app.browse.selected.as_deref() == Some(&item.id);
                    if list_row(app, ui, item, is_selected, game.as_deref()).clicked() {
                        select = Some(item.id.clone());
                    }
                    ui.add_space(space::S);
                }
            });
        });
    }
    if !narrow || selected.is_some() {
        probe::mark(ui.ctx(), "browse:details", details_rect);
        ui.scope_builder(egui::UiBuilder::new().max_rect(details_rect), |ui| match &selected {
            Some(item) => {
                if narrow && icon_button(ui, icons::ARROW_LEFT, app.tr.get("browse.back")).clicked() {
                    app.browse.selected = None;
                }
                details(app, ui, item, game.as_deref(), &mut install);
            }
            None => empty(ui, icons::PUZZLE, app.tr.get("browse.pick")),
        });
    }
    ui.allocate_rect(area, egui::Sense::hover());
    if let Some(id) = select {
        app.browse.selected = Some(id);
    }
    if let Some(id) = install {
        app.install_mod(ui.ctx(), &id);
    }
}

fn filters(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let search = widgets::search_field(ui, &mut app.browse.query, app.tr.get("browse.search"), 260.0);
        if app.focus_search {
            search.request_focus();
            app.focus_search = false;
        }
        let tags = app.index.tags();
        let current = app.browse.tag.clone().unwrap_or_else(|| app.tr.get("browse.all_tags").to_owned());
        egui::ComboBox::from_id_salt("tag")
            .selected_text(format!("{}  {current}", icons::TAG))
            .show_ui(ui, |ui| {
                if ui.selectable_label(app.browse.tag.is_none(), app.tr.get("browse.all_tags")).clicked() {
                    app.browse.tag = None;
                }
                for (tag, count) in tags {
                    if ui.selectable_label(app.browse.tag.as_ref() == Some(&tag), format!("{tag} ({count})")).clicked()
                    {
                        app.browse.tag = Some(tag);
                    }
                }
            })
            .response
            .on_hover_text(app.tr.get("browse.tag"));
        widgets::switch(ui, &mut app.browse.compatible_only, app.tr.get("browse.compatible_only"));
        if icon_button(ui, icons::ARROWS_CLOCKWISE, app.tr.get("common.refresh")).clicked() {
            app.load_index(ui.ctx());
        }
    });
}

fn compat_chip(app: &App, ui: &mut egui::Ui, compat: Compat) {
    let (key, tint) = match compat {
        Compat::Yes => ("compat.yes", color::PLAY),
        Compat::No => ("compat.no", color::DANGER),
        Compat::Unknown => ("compat.unknown", color::TEXT_MUTED),
    };
    widgets::chip(ui, app.tr.get(key), tint);
}

fn list_row(app: &App, ui: &mut egui::Ui, item: &IndexMod, selected: bool, game: Option<&str>) -> egui::Response {
    let card = Card::new().padding(space::M).clickable().outline_if(selected, color::ACCENT);
    let response = card.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                compat_chip(app, ui, item.compat(game));
                if app.mods.iter().any(|m| m.id() == item.id) {
                    ui.label(widgets::icon(icons::CHECK_CIRCLE, 16.0, color::PLAY))
                        .on_hover_text(app.tr.get("browse.installed"));
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    widgets::one_line(ui, widgets::strong(&item.name, size::TITLE));
                });
            });
        });
        widgets::one_line(ui, RichText::new(&item.summary).size(size::SMALL).color(color::TEXT_MUTED));
    });
    let name = item.name.clone();
    response.response.widget_info(|| egui::WidgetInfo::selected(egui::Role::ListItem, true, selected, &name));
    probe::mark(ui.ctx(), format!("browse:row:{}", item.id), response.response.rect);
    response.response
}

fn details(app: &mut App, ui: &mut egui::Ui, item: &IndexMod, game: Option<&str>, install: &mut Option<String>) {
    egui::ScrollArea::vertical().id_salt("details").auto_shrink([false, false]).show(ui, |ui| {
        Card::new().padding(space::XL).show(ui, |ui| {
            ui.horizontal(|ui| {
                match &item.icon {
                    Some(icon) => image(app, ui, icon, 56.0),
                    None => {
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(56.0, 56.0), egui::Sense::hover());
                        crate::widgets::paint::tile(ui.painter(), rect, &item.id, &item.name);
                    }
                }
                ui.vertical(|ui| {
                    ui.label(RichText::new(&item.name).font(font::bold(size::HEADING)).color(color::TEXT));
                    let authors = item.authors.join(", ");
                    widgets::one_line(ui, widgets::muted(format!("{} · {} · {}", item.id, authors, item.license)));
                });
            });
            ui.add_space(space::S);
            ui.horizontal_wrapped(|ui| {
                for tag in &item.tags {
                    widgets::chip(ui, &format!("#{tag}"), color::ACCENT);
                }
                widgets::chip(ui, &app.tr.fill("mods.side", &[("side", &item.side)]), color::INFO);
                if item.has_code {
                    widgets::chip(ui, app.tr.get("mods.has_code"), color::WARN)
                        .on_hover_text(app.tr.get("mods.has_code_hint"));
                }
            });
            ui.add_space(space::M);
            install_box(app, ui, item, game, install);
            ui.add_space(space::M);
            let text = if item.description.is_empty() { &item.summary } else { &item.description };
            ui.add(egui::Label::new(RichText::new(text).color(color::TEXT)).wrap());
            for url in [&item.homepage, &item.source].into_iter().flatten() {
                ui.horizontal(|ui| {
                    ui.label(widgets::icon(icons::LINK, 14.0, color::ACCENT));
                    ui.hyperlink_to(short_url(url), url);
                });
            }
            if !item.screenshots.is_empty() {
                ui.add_space(space::S);
                ui.horizontal_wrapped(|ui| {
                    for url in item.screenshots.clone() {
                        image(app, ui, &url, 220.0);
                    }
                });
            }
            ui.add_space(space::M);
            widgets::section_title(ui, icons::STACK, app.tr.get("browse.versions"));
            for version in &item.versions {
                version_row(app, ui, version, game);
            }
        });
    });
}

fn install_box(app: &mut App, ui: &mut egui::Ui, item: &IndexMod, game: Option<&str>, install: &mut Option<String>) {
    let Some(best) = item.best_version(game) else {
        ui.horizontal(|ui| {
            ui.label(widgets::icon(icons::X_CIRCLE, 16.0, color::DANGER));
            ui.label(RichText::new(app.tr.get("browse.no_fitting_version")).color(color::DANGER));
        });
        return;
    };
    let installed =
        app.mods.iter().find(|m| m.id() == item.id).and_then(|m| m.manifest.as_ref().ok()).map(|m| m.version.clone());
    Card::new().fill(color::SUNKEN).padding(space::M).show(ui, |ui| {
        ui.horizontal(|ui| {
            let label = match &installed {
                Some(v) if *v == best.version => None,
                Some(_) => Some(app.tr.fill("mods.update_to", &[("version", &best.version)])),
                None => Some(app.tr.fill("browse.install", &[("version", &best.version)])),
            };
            let busy = app.jobs.running > 0 && app.jobs.progress.lock().is_ok_and(|p| p.is_some());
            match label {
                Some(label) => {
                    let pill =
                        Pill::new(Kind::Accent, &label).icon(icons::DOWNLOAD).enabled(!busy && !app.settings.offline);
                    if pill.show(ui).clicked() {
                        *install = Some(item.id.clone());
                    }
                }
                None => {
                    ui.label(widgets::icon(icons::CHECK_CIRCLE, 18.0, color::PLAY));
                    ui.label(RichText::new(app.tr.get("browse.up_to_date")).color(color::PLAY));
                }
            }
            ui.label(widgets::muted(megabytes(best.size, None)));
            compat_chip(app, ui, castanea_launcher_core::version::compat(best.game.as_deref(), game));
        });
        let missing = index_install::missing_dependencies(&app.index, item, best, &app.mods, game);
        if !missing.is_empty() {
            let list: Vec<String> = missing.iter().map(|(m, v)| format!("{} {}", m.id, v.version)).collect();
            ui.label(RichText::new(app.tr.fill("browse.needs", &[("list", &list.join(", "))])).color(color::WARN));
        }
        ui.horizontal(|ui| {
            ui.label(widgets::icon(icons::SHIELD_CHECK, 14.0, color::TEXT_FAINT));
            ui.add(egui::Label::new(widgets::faint(app.tr.get("browse.install_note"))).wrap());
        });
    });
}

fn version_row(app: &App, ui: &mut egui::Ui, version: &IndexVersion, game: Option<&str>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(&version.version).monospace().color(color::TEXT));
        ui.label(widgets::faint(&version.published));
        let range = version.game.clone().unwrap_or_else(|| "?".to_owned());
        ui.label(RichText::new(app.tr.fill("browse.game_range", &[("range", &range)])).size(size::SMALL));
        compat_chip(app, ui, castanea_launcher_core::version::compat(version.game.as_deref(), game));
    });
    if !version.changelog.is_empty() {
        ui.label(RichText::new(&version.changelog).size(size::SMALL).color(color::TEXT_MUTED));
    }
}

fn image(app: &mut App, ui: &mut egui::Ui, url: &str, side: f32) {
    app.request_image(ui.ctx(), url);
    match app.images.get(url) {
        Some(ImageState::Ready(bytes)) => {
            let uri = format!("bytes://{url}");
            ui.add(
                egui::Image::from_bytes(uri, bytes.clone()).max_width(side).max_height(side).corner_radius(radius::M),
            );
        }
        Some(ImageState::Failed) => {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(side.min(56.0), side.min(56.0)), egui::Sense::hover());
            ui.painter().rect_filled(rect, theme::radius(radius::M), color::SURFACE_HOVER);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                icons::PACKAGE,
                font::icon(24.0, false),
                color::TEXT_FAINT,
            );
        }
        _ => {
            ui.add(egui::Spinner::new());
        }
    }
}

/// A link's text short enough for one line: the host and the last part of the path.
fn short_url(url: &str) -> String {
    let bare = url.split_once("://").map_or(url, |(_, rest)| rest).trim_end_matches('/');
    match (bare.split_once('/'), bare.rsplit_once('/')) {
        (Some((host, path)), Some((_, last))) if path.len() > 30 => format!("{host}/…/{last}"),
        _ => bare.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn short_url_keeps_host_and_last_part() {
        assert_eq!(super::short_url("https://example.org/mods"), "example.org/mods");
        let long = "https://github.com/TerraNuxt/castanea/tree/main/examples/mods/amber";
        assert_eq!(super::short_url(long), "github.com/…/amber");
    }
}
