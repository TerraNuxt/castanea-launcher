//! Mods → Browse: search and filter the index, the details of one mod, install with progress. Narrow windows show the
//! list or the details with a Back button.

use castanea_launcher_core::index::install as index_install;
use castanea_launcher_core::index::{IndexMod, IndexVersion};
use castanea_launcher_core::version::{self, Compat};
use eframe::egui::{self, RichText, Sense};

use crate::app::{App, ImageState, Page};
use crate::shell::megabytes;
use crate::theme::{self, color, layout, radius, size, space};
use crate::widgets::{self, Button, Kind, icon_button};
use crate::{icons, probe};

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    if app.settings.indexes.is_empty() {
        widgets::empty(ui, app.tr.get("browse.no_index"));
        ui.add_space(space::L);
        ui.vertical_centered(|ui| {
            if Button::new(Kind::Secondary, app.tr.get("browse.go_settings")).show(ui).clicked() {
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
        widgets::wrapped(ui, RichText::new(warning).size(size::SMALL).color(color::WARN));
    }
    ui.add_space(space::M);
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
        let list = area.with_max_x(area.left() + layout::LIST);
        (list, area.with_min_x(list.right() + space::XL))
    };
    let mut select = None;
    let mut install = None;
    if !narrow || selected.is_none() {
        probe::mark(ui.ctx(), "browse:list", list_rect);
        ui.scope_builder(egui::UiBuilder::new().max_rect(list_rect), |ui| {
            egui::ScrollArea::vertical().id_salt("browse-list").auto_shrink([false, false]).show(ui, |ui| {
                if shown.is_empty() {
                    widgets::empty(ui, app.tr.get("browse.nothing"));
                }
                for item in &shown {
                    let is_selected = app.browse.selected.as_deref() == Some(&item.id);
                    if list_row(app, ui, item, is_selected, game.as_deref()).clicked() {
                        select = Some(item.id.clone());
                    }
                }
            });
        });
    }
    if !narrow || selected.is_some() {
        probe::mark(ui.ctx(), "browse:details", details_rect);
        if !narrow {
            let x = details_rect.left() - space::M;
            ui.painter().vline(x, details_rect.y_range(), theme::hairline());
        }
        ui.scope_builder(egui::UiBuilder::new().max_rect(details_rect), |ui| match &selected {
            Some(item) => {
                if narrow && icon_button(ui, icons::ARROW_LEFT, app.tr.get("browse.back")).clicked() {
                    app.browse.selected = None;
                }
                details(app, ui, item, game.as_deref(), &mut install);
            }
            None => widgets::empty(ui, app.tr.get("browse.pick")),
        });
    }
    ui.allocate_rect(area, Sense::hover());
    if let Some(id) = select {
        app.browse.selected = Some(id);
    }
    if let Some(id) = install {
        app.install_mod(ui.ctx(), &id);
    }
}

fn filters(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let search = widgets::search_field(ui, &mut app.browse.query, app.tr.get("browse.search"), layout::FIELD);
        if app.focus_search {
            search.request_focus();
            app.focus_search = false;
        }
        let tags = app.index.tags();
        let current = app.browse.tag.clone().unwrap_or_else(|| app.tr.get("browse.all_tags").to_owned());
        egui::ComboBox::from_id_salt("tag")
            .selected_text(current)
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
        ui.add_space(space::S);
        widgets::switch(ui, &mut app.browse.compatible_only, app.tr.get("browse.compatible_only"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icon_button(ui, icons::ARROWS_CLOCKWISE, app.tr.get("common.refresh")).clicked() {
                app.load_index(ui.ctx());
            }
        });
    });
}

fn compat_text(app: &App, compat: Compat) -> RichText {
    let (key, tint) = match compat {
        Compat::Yes => ("compat.yes", color::TEXT_MUTED),
        Compat::No => ("compat.no", color::DANGER),
        Compat::Unknown => ("compat.unknown", color::TEXT_MUTED),
    };
    RichText::new(app.tr.get(key)).size(size::SMALL).color(tint)
}

fn list_row(app: &App, ui: &mut egui::Ui, item: &IndexMod, selected: bool, game: Option<&str>) -> egui::Response {
    let rect = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), layout::LIST_ROW));
    let response = ui.allocate_rect(rect, Sense::click());
    let background = ui.painter().add(egui::Shape::Noop);
    let inner = rect.shrink(space::M);
    let line = inner.height() / 2.0;
    let mut notes = vec![compat_text(app, item.compat(game))];
    if app.mods.iter().any(|m| m.id() == item.id) {
        notes.push(widgets::small(app.tr.get("browse.installed")));
    }
    let mut right = inner.right();
    for note in notes {
        let galley = egui::WidgetText::from(note).into_galley(ui, None, f32::INFINITY, egui::TextStyle::Small);
        right -= galley.size().x;
        let at = egui::pos2(right, inner.top() + (line - galley.size().y) / 2.0);
        ui.painter().galley(at, galley, color::TEXT_MUTED);
        right -= space::S;
    }
    let name_rect = egui::Rect::from_min_max(inner.min, egui::pos2(right - space::S, inner.top() + line));
    let summary_rect = egui::Rect::from_min_max(egui::pos2(inner.left(), inner.top() + line), inner.max);
    for (area, text) in
        [(name_rect, widgets::strong(&item.name, size::BODY)), (summary_rect, widgets::small(&item.summary))]
    {
        // A child that does not move the parent's cursor: the row's rect is already allocated.
        let mut child = ui
            .new_child(egui::UiBuilder::new().max_rect(area).layout(egui::Layout::left_to_right(egui::Align::Center)));
        widgets::one_line(&mut child, text);
    }
    let (hover, _) = widgets::button::hover_press(ui, &response);
    let fill = if selected { color::RAISED } else { theme::mix(egui::Color32::TRANSPARENT, color::HOVER, hover) };
    ui.painter().set(background, egui::Shape::rect_filled(rect, theme::radius(radius::M), fill));
    widgets::button::focus_ring(ui, &response, rect, radius::M);
    let name = item.name.clone();
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::ListItem, true, selected, &name));
    probe::mark(ui.ctx(), format!("browse:row:{}", item.id), rect);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn details(app: &mut App, ui: &mut egui::Ui, item: &IndexMod, game: Option<&str>, install: &mut Option<String>) {
    egui::ScrollArea::vertical().id_salt("details").auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal(|ui| {
            if let Some(icon) = &item.icon {
                image(app, ui, icon, layout::IMAGE);
            }
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = crate::theme::layout::LINE_GAP;
                ui.label(widgets::strong(&item.name, size::TITLE));
                let authors = item.authors.join(", ");
                widgets::one_line(ui, widgets::small(format!("{}  ·  {}  ·  {}", item.id, authors, item.license)));
            });
        });
        ui.add_space(space::S);
        ui.horizontal_wrapped(|ui| {
            for tag in &item.tags {
                widgets::tag(ui, tag, color::TEXT_MUTED);
            }
            widgets::tag(ui, &app.tr.fill("mods.side", &[("side", &item.side)]), color::TEXT_MUTED);
            if item.has_code {
                widgets::tag(ui, app.tr.get("mods.has_code"), color::WARN)
                    .on_hover_text(app.tr.get("mods.has_code_hint"));
            }
        });
        ui.add_space(space::L);
        install_row(app, ui, item, game, install);
        ui.add_space(space::L);
        let text = if item.description.is_empty() { &item.summary } else { &item.description };
        widgets::wrapped(ui, widgets::text(text));
        for url in [&item.homepage, &item.source].into_iter().flatten() {
            ui.hyperlink_to(RichText::new(short_url(url)).size(size::BODY).color(color::TEXT).underline(), url);
        }
        if !item.screenshots.is_empty() {
            ui.add_space(space::S);
            ui.horizontal_wrapped(|ui| {
                for url in item.screenshots.clone() {
                    image(app, ui, &url, layout::SCREENSHOT);
                }
            });
        }
        widgets::section(ui, app.tr.get("browse.versions"));
        for version in &item.versions {
            version_row(app, ui, version, game);
        }
    });
}

fn install_row(app: &mut App, ui: &mut egui::Ui, item: &IndexMod, game: Option<&str>, install: &mut Option<String>) {
    let Some(best) = item.best_version(game) else {
        ui.label(RichText::new(app.tr.get("browse.no_fitting_version")).color(color::DANGER));
        return;
    };
    let installed =
        app.mods.iter().find(|m| m.id() == item.id).and_then(|m| m.manifest.as_ref().ok()).map(|m| m.version.clone());
    ui.horizontal(|ui| {
        let label = match &installed {
            Some(v) if *v == best.version => None,
            Some(_) => Some(app.tr.fill("mods.update_to", &[("version", &best.version)])),
            None => Some(app.tr.fill("browse.install", &[("version", &best.version)])),
        };
        let busy = crate::jobs::get(&app.jobs.progress).is_some();
        match label {
            Some(label) => {
                let button = Button::new(Kind::Primary, &label).enabled(!busy && !app.settings.offline);
                if button.show(ui).clicked() {
                    *install = Some(item.id.clone());
                }
            }
            None => {
                ui.label(widgets::muted(app.tr.get("browse.up_to_date")));
            }
        }
        ui.add_space(space::S);
        ui.label(widgets::small(megabytes(best.size)));
        ui.label(widgets::small("·"));
        ui.label(compat_text(app, version::compat(best.game.as_deref(), game)));
    });
    let missing = index_install::missing_dependencies(&app.index, item, best, &app.mods, game);
    if !missing.is_empty() {
        let list: Vec<String> = missing.iter().map(|(m, v)| format!("{} {}", m.id, v.version)).collect();
        widgets::wrapped(
            ui,
            RichText::new(app.tr.fill("browse.needs", &[("list", &list.join(", "))])).color(color::WARN),
        );
    }
    widgets::wrapped(ui, widgets::small(app.tr.get("browse.install_note")));
}

fn version_row(app: &App, ui: &mut egui::Ui, version: &IndexVersion, game: Option<&str>) {
    ui.add_space(space::S);
    ui.horizontal(|ui| {
        ui.label(widgets::strong(&version.version, size::BODY));
        ui.label(widgets::small(&version.published));
        let range = version.game.clone().unwrap_or_else(|| "?".to_owned());
        ui.label(widgets::small(app.tr.fill("browse.game_range", &[("range", &range)])));
        ui.label(compat_text(app, version::compat(version.game.as_deref(), game)));
    });
    if !version.changelog.is_empty() {
        widgets::wrapped(ui, widgets::small(&version.changelog));
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
        Some(ImageState::Failed) => {}
        _ => {
            ui.add(egui::Spinner::new().size(size::ICON));
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
