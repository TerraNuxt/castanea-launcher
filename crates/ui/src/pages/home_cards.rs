//! The home page's right column: news, mods (updates or featured), multiplayer and shortcuts, each with its accent.

use castanea_launcher_core::index::install as index_install;
use castanea_launcher_core::install::{Origin, local};
use castanea_launcher_core::launch;
use castanea_launcher_core::mods::import;
use castanea_launcher_core::profile::StartIn;
use eframe::egui::{self, RichText};

use crate::app::{App, ModsView, Page, open_or_note};
use crate::theme::{color, size, space};
use crate::widgets::card::card_title;
use crate::widgets::{self, Card, Kind, Pill};
use crate::{icons, probe};

type CardFn = fn(&mut App, &mut egui::Ui);

const CARDS: [(&str, CardFn); 4] =
    [("news", news), ("mods", mods), ("multiplayer", multiplayer), ("shortcuts", shortcuts)];

/// The cards one under the other, for the wide layout.
pub fn column(app: &mut App, ui: &mut egui::Ui) {
    for (name, card) in CARDS {
        let top = ui.cursor().top();
        card(app, ui);
        let rect = egui::Rect::from_x_y_ranges(ui.max_rect().x_range(), top..=ui.min_rect().bottom());
        probe::mark(ui.ctx(), format!("home:card:{name}"), rect);
        ui.add_space(space::M);
    }
}

/// The cards in two columns, under the profile list in the narrow layout.
pub fn grid(app: &mut App, ui: &mut egui::Ui) {
    ui.columns(2, |columns| {
        for (n, (name, card)) in CARDS.into_iter().enumerate() {
            let ui = &mut columns[n % 2];
            let top = ui.cursor().top();
            card(app, ui);
            probe::mark(
                ui.ctx(),
                format!("home:card:{name}"),
                egui::Rect::from_x_y_ranges(ui.max_rect().x_range(), top..=ui.min_rect().bottom()),
            );
            ui.add_space(space::M);
        }
    });
}

fn news(app: &mut App, ui: &mut egui::Ui) {
    Card::new().accent(color::ACCENT).show(ui, |ui| {
        card_title(ui, icons::NEWSPAPER, app.tr.get("play.news"), color::ACCENT, None);
        ui.add_space(space::XS);
        if app.index.news.is_empty() {
            let key = if app.settings.indexes.is_empty() { "play.no_news_source" } else { "play.no_news" };
            ui.label(widgets::muted(app.tr.get(key)).size(size::SMALL));
        }
        for item in app.index.news.iter().take(2) {
            ui.horizontal(|ui| {
                widgets::one_line(ui, widgets::strong(&item.title, size::BODY));
            });
            ui.label(widgets::faint(&item.date));
            ui.add(egui::Label::new(RichText::new(&item.body).size(size::SMALL).color(color::TEXT_MUTED)).wrap());
            if let Some(url) = &item.url {
                ui.hyperlink_to(RichText::new(app.tr.get("home.read_more")).size(size::SMALL), url);
            }
            ui.add_space(space::XS);
        }
        let builds = app.registry.builds.iter().filter(|b| b.is_present()).count().to_string();
        let enabled = app.mods.iter().filter(|m| m.enabled).count().to_string();
        let total = app.mods.len().to_string();
        let summary = app.tr.fill("play.summary", &[("builds", &builds), ("enabled", &enabled), ("mods", &total)]);
        ui.add(egui::Label::new(widgets::faint(summary)).wrap());
    });
}

fn mods(app: &mut App, ui: &mut egui::Ui) {
    let game = app.game_version();
    let updates = index_install::updates(&app.mods, &app.index, game.as_deref());
    Card::new().accent(color::VIOLET).show(ui, |ui| {
        let note = (!updates.is_empty()).then(|| app.tr.fill("home.updates", &[("count", &updates.len().to_string())]));
        card_title(ui, icons::PUZZLE, app.tr.get("home.mods_card"), color::VIOLET, note.as_deref());
        ui.add_space(space::XS);
        let mut open = None;
        if updates.is_empty() {
            let featured: Vec<(String, String, String)> = app
                .index
                .mods
                .iter()
                .filter(|m| !app.mods.iter().any(|i| i.id() == m.id))
                .take(3)
                .map(|m| (m.id.clone(), m.name.clone(), m.summary.clone()))
                .collect();
            if featured.is_empty() {
                ui.label(widgets::muted(app.tr.get("home.no_featured")).size(size::SMALL));
            } else {
                ui.label(widgets::faint(app.tr.get("home.featured")));
            }
            for (id, name, summary) in featured {
                let row = ui.horizontal(|ui| {
                    ui.label(widgets::icon(icons::SPARKLE, 16.0, color::VIOLET));
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        widgets::one_line(ui, widgets::strong(&name, size::BODY));
                        widgets::one_line(ui, RichText::new(summary).size(size::CAPTION).color(color::TEXT_MUTED));
                    });
                });
                let hit = ui.interact(row.response.rect, ui.scope_id().with(("featured", &id)), egui::Sense::click());
                if hit.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    open = Some(id);
                }
            }
        } else {
            for update in updates.iter().take(3) {
                ui.horizontal(|ui| {
                    ui.label(widgets::icon(icons::ARROWS_CLOCKWISE, 16.0, color::VIOLET));
                    widgets::one_line(
                        ui,
                        RichText::new(format!("{}  {} → {}", update.id, update.installed, update.available))
                            .size(size::SMALL),
                    );
                });
            }
            let busy = app.jobs.progress.lock().is_ok_and(|p| p.is_some());
            let width = ui.available_width();
            let first = updates[0].id.clone();
            let label = app.tr.fill("home.update_one", &[("id", &first)]);
            let button = Pill::new(Kind::Ghost, &label).icon(icons::DOWNLOAD).width(width);
            if button.enabled(!busy && !app.settings.offline).show(ui).clicked() {
                app.install_mod(ui.ctx(), &first);
            }
        }
        if let Some(id) = open {
            app.browse.selected = Some(id);
            app.mods_view = ModsView::Browse;
            app.go(Page::Mods);
        }
    });
}

fn multiplayer(app: &mut App, ui: &mut egui::Ui) {
    Card::new().accent(color::INFO).show(ui, |ui| {
        card_title(ui, icons::USERS, app.tr.get("home.multiplayer"), color::INFO, None);
        ui.add_space(space::XS);
        ui.label(RichText::new(app.tr.get("home.join_title")).size(size::SMALL).color(color::TEXT_MUTED));
        let ready = launch::pick_build(&app.registry, app.profile()).is_ok();
        let id = app.profile_id.clone();
        let mut join = false;
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let can = ready && !app.join_address.trim().is_empty() && !app.is_running(&id);
                let button = Pill::new(Kind::Play, app.tr.get("home.join")).icon(icons::SIGN_IN).enabled(can).show(ui);
                let edit = egui::TextEdit::singleline(&mut app.join_address)
                    .hint_text("10.0.0.5:47800")
                    .desired_width(ui.available_width());
                let response = ui.add(edit);
                let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                join = can && (button.clicked() || enter);
            });
        });
        if join {
            let address = app.join_address.trim().to_owned();
            app.play_profile(&id, Some(StartIn::Join { address }));
        }
        ui.add_space(space::S);
        let mut hosting = matches!(app.profile().start, StartIn::World { host_port: Some(_), .. });
        let in_world = matches!(app.profile().start, StartIn::World { .. });
        let response =
            ui.add_enabled_ui(in_world, |ui| widgets::switch(ui, &mut hosting, app.tr.get("home.host"))).inner;
        if response.changed()
            && let StartIn::World { host_port, .. } = &mut app.profile_mut().start
        {
            *host_port = hosting.then_some(47800);
            app.save_profiles();
        }
        let hint = if in_world { "home.host_hint" } else { "home.host_needs_world" };
        ui.add(egui::Label::new(widgets::faint(app.tr.get(hint))).wrap());
        ui.add_space(space::XS);
        let relay = app.profile().backend_url.clone();
        ui.horizontal(|ui| {
            let (glyph, tint) = if relay.is_empty() {
                (icons::CLOUD_SLASH, color::TEXT_FAINT)
            } else {
                (icons::BROADCAST, color::PLAY)
            };
            ui.label(widgets::icon(glyph, 16.0, tint));
            let text = if relay.is_empty() {
                app.tr.get("home.no_relay").to_owned()
            } else {
                app.tr.fill("home.relay", &[("url", &relay)])
            };
            widgets::one_line(ui, RichText::new(text).size(size::SMALL).color(color::TEXT_MUTED));
        });
    });
}

fn shortcuts(app: &mut App, ui: &mut egui::Ui) {
    Card::new().accent(color::TEAL).show(ui, |ui| {
        card_title(ui, icons::LIGHTNING, app.tr.get("home.shortcuts"), color::TEAL, None);
        ui.add_space(space::XS);
        let rows: [(&str, &str); 5] = [
            (icons::FOLDER_OPEN, "play.open_folder"),
            (icons::PUZZLE, "mods.open_folder"),
            (icons::FILE_ZIP, "mods.import_zip"),
            (icons::HARD_DRIVES, "versions.add_folder"),
            (icons::TERMINAL, "play.open_log"),
        ];
        for (n, (glyph, key)) in rows.into_iter().enumerate() {
            if shortcut_row(ui, glyph, app.tr.get(key)).clicked() {
                run_shortcut(app, n);
            }
        }
    });
}

fn shortcut_row(ui: &mut egui::Ui, glyph: &str, text: &str) -> egui::Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 30.0), egui::Sense::click());
    let (hover, _) = crate::widgets::button::hover_press(ui, &response);
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        crate::theme::radius(crate::theme::radius::S),
        color::SURFACE_HOVER.gamma_multiply(hover),
    );
    painter.text(
        rect.left_center() + egui::vec2(8.0, 0.0),
        egui::Align2::LEFT_CENTER,
        glyph,
        crate::theme::font::icon(16.0, false),
        color::TEAL,
    );
    painter.text(
        rect.left_center() + egui::vec2(32.0, 0.0),
        egui::Align2::LEFT_CENTER,
        text,
        crate::theme::font::regular(size::SMALL),
        color::TEXT,
    );
    painter.text(
        rect.right_center() - egui::vec2(8.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        icons::CARET_RIGHT,
        crate::theme::font::icon(14.0, false),
        color::TEXT_FAINT,
    );
    crate::widgets::button::focus_ring(ui, &response, rect, crate::theme::radius::S);
    let label = text.to_owned();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn run_shortcut(app: &mut App, n: usize) {
    match n {
        0 => {
            let dir = app.profile().game_dir(&app.paths);
            open_or_note(app, &dir);
        }
        1 => {
            let dir = app.mods_dir();
            open_or_note(app, &dir);
        }
        2 => {
            if let Some(file) = rfd::FileDialog::new().add_filter("zip", &["zip"]).pick_file() {
                match import::import(&app.mods_dir(), &file, false) {
                    Ok(package) => {
                        let text = app.tr.fill("mods.installed_disabled", &[("id", package.id())]);
                        app.info(text);
                    }
                    Err(error) => app.error(error.to_string()),
                }
                app.reload_mods();
            }
        }
        3 => {
            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                match local::inspect(&app.registry, &folder, Origin::Local) {
                    Ok(build) => {
                        let text = app.tr.fill("versions.added", &[("version", &build.id)]);
                        app.registry.put(build);
                        app.save_registry();
                        app.info(text);
                    }
                    Err(error) => app.error(error.to_string()),
                }
            }
        }
        _ => {
            let dir = app.profile().game_dir(&app.paths).join(launch::run::LOG_DIR);
            open_or_note(app, &dir);
        }
    }
}
