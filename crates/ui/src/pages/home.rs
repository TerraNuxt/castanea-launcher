//! Home: the welcome line, the hero with the current profile and Play, the last played profiles, and the right column
//! of cards (`home_cards.rs`), which folds under the list when the window is narrow.

use castanea_launcher_core::launch;
use eframe::egui::{self, Rect, RichText, Sense};

use crate::app::{App, ModsView, Page};
use crate::pages::{home_cards, profile_line};
use crate::theme::{color, font, layout, size, space};
use crate::widgets::{self, Card, Kind, Pill, icon_button, paint};
use crate::{hero, icons, probe};

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    if !app.index_loaded && !app.settings.indexes.is_empty() {
        app.load_index(ui.ctx());
    }
    welcome(app, ui);
    ui.add_space(space::L);
    let area = ui.available_rect_before_wrap();
    let wide = area.width() >= layout::WIDE;
    let main = if wide { area.with_max_x(area.right() - layout::RIGHT_COLUMN - space::XL) } else { area };
    ui.scope_builder(egui::UiBuilder::new().max_rect(main), |ui| main_column(app, ui, !wide));
    if wide {
        let right = area.with_min_x(area.right() - layout::RIGHT_COLUMN);
        probe::mark(ui.ctx(), "home:right", right);
        ui.scope_builder(egui::UiBuilder::new().max_rect(right), |ui| {
            egui::ScrollArea::vertical().id_salt("right").auto_shrink([false, false]).show(ui, |ui| {
                home_cards::column(app, ui);
            });
        });
    }
    ui.allocate_rect(area, Sense::hover());
}

fn welcome(app: &App, ui: &mut egui::Ui) {
    let name = app.profile().player_name.trim().to_owned();
    let name = if name.is_empty() { system_user() } else { Some(name) };
    let text = match name {
        Some(name) => app.tr.fill("home.welcome", &[("name", &name)]),
        None => app.tr.get("home.welcome_anon").to_owned(),
    };
    let response = ui.horizontal(|ui| {
        let (mark, _) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), Sense::hover());
        paint::chestnut(ui.painter(), mark.center(), 36.0, color::ACCENT);
        ui.add_space(space::XS);
        widgets::one_line(ui, RichText::new(text).font(font::bold(size::DISPLAY)).color(color::TEXT));
    });
    probe::mark(ui.ctx(), "home:welcome", response.response.rect);
}

/// The login name, for a welcome without a player name.
fn system_user() -> Option<String> {
    ["USER", "USERNAME"].iter().find_map(|k| std::env::var(k).ok()).filter(|n| !n.is_empty() && n != "root")
}

fn main_column(app: &mut App, ui: &mut egui::Ui, narrow: bool) {
    let area = ui.max_rect();
    let hero_height = (area.height() * 0.36).clamp(150.0, 260.0);
    let hero_rect = Rect::from_min_size(area.min, egui::vec2(area.width(), hero_height));
    hero_panel(app, ui, hero_rect);
    let actions_rect = Rect::from_min_max(egui::pos2(area.left(), area.bottom() - layout::BUTTON_HEIGHT), area.max);
    let header_top = hero_rect.bottom() + space::L;
    let header_rect = Rect::from_min_size(egui::pos2(area.left(), header_top), egui::vec2(area.width(), 28.0));
    let list_rect = Rect::from_min_max(
        egui::pos2(area.left(), header_rect.bottom() + space::S),
        egui::pos2(area.right(), actions_rect.top() - space::M),
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(header_rect), |ui| list_header(app, ui));
    probe::mark(ui.ctx(), "home:list", list_rect);
    ui.scope_builder(egui::UiBuilder::new().max_rect(list_rect), |ui| {
        egui::ScrollArea::vertical().id_salt("profiles").auto_shrink([false, false]).show(ui, |ui| {
            let ids: Vec<String> = app.profiles.by_last_played().iter().map(|p| p.id.clone()).collect();
            for id in ids {
                profile_card(app, ui, &id);
                ui.add_space(space::S);
            }
            if narrow {
                ui.add_space(space::M);
                home_cards::grid(app, ui);
            }
        });
    });
    probe::mark(ui.ctx(), "home:actions", actions_rect);
    ui.scope_builder(egui::UiBuilder::new().max_rect(actions_rect), |ui| bottom_actions(app, ui));
}

fn hero_panel(app: &mut App, ui: &mut egui::Ui, rect: Rect) {
    probe::mark(ui.ctx(), "home:hero", rect);
    let profile = app.profile().clone();
    hero::paint(ui.painter(), rect, paint::scheme_for(&profile.id), hero::seed(&profile.id));
    let inner = rect.shrink2(egui::vec2(space::XL, space::L + 2.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::bottom_up(egui::Align::Min)), |ui| {
        ui.horizontal(|ui| {
            profile_picker(app, ui);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let id = profile.id.clone();
                let running = app.is_running(&id);
                let ready = launch::pick_build(&app.registry, &profile).is_ok();
                let play = if running {
                    Pill::new(Kind::Danger, app.tr.get("play.stop")).icon(icons::STOP).big().show(ui)
                } else {
                    let pill = Pill::new(Kind::Play, app.tr.get("play.play")).icon(icons::PLAY).big().enabled(ready);
                    let response = pill.show(ui);
                    if ready {
                        response.on_hover_text("F5")
                    } else {
                        response.on_hover_text(app.tr.get("play.no_build"))
                    }
                };
                probe::mark(ui.ctx(), "home:hero:play", play.rect);
                if play.clicked() {
                    if running { app.stop(&id) } else { app.play() }
                }
                let edit = icon_button(ui, icons::PENCIL, app.tr.get("home.edit_profile"));
                probe::mark(ui.ctx(), "home:hero:edit", edit.rect);
                if edit.clicked() {
                    app.go(Page::Profiles);
                }
            });
        });
        ui.add_space(space::S);
        let line = profile_line(app, &profile, false);
        widgets::one_line(ui, RichText::new(line).size(size::BODY).color(egui::Color32::from_white_alpha(210)));
        widgets::one_line(
            ui,
            RichText::new(&profile.name).font(font::bold(size::DISPLAY + 4.0)).color(color::ON_ACCENT),
        );
    });
}

fn profile_picker(app: &mut App, ui: &mut egui::Ui) {
    let mut chosen = None;
    let current = app.profile().name.clone();
    let response = egui::ComboBox::from_id_salt("hero-profile")
        .width(220.0)
        .height(300.0)
        .selected_text(RichText::new(format!("{}   {current}", icons::USER)).font(font::semibold(size::BODY)))
        .show_ui(ui, |ui| {
            for profile in &app.profiles.profiles {
                if ui.selectable_label(profile.id == app.profile_id, &profile.name).clicked() {
                    chosen = Some(profile.id.clone());
                }
            }
        });
    response.response.on_hover_text(app.tr.get("play.profile"));
    if let Some(id) = chosen {
        app.set_profile(id);
    }
}

fn list_header(app: &App, ui: &mut egui::Ui) {
    ui.horizontal_centered(|ui| {
        ui.label(widgets::icon(icons::CLOCK, 18.0, color::TEXT_MUTED));
        ui.label(RichText::new(app.tr.get("home.last_played")).font(font::semibold(size::TITLE)).color(color::TEXT));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let count = app.profiles.profiles.len().to_string();
            widgets::chip(ui, &app.tr.fill("home.profiles_count", &[("count", &count)]), color::TEXT_MUTED);
        });
    });
}

fn profile_card(app: &mut App, ui: &mut egui::Ui, id: &str) {
    let Some(profile) = app.profiles.get(id).cloned() else { return };
    let running = app.is_running(id);
    let current = app.profile_id == id;
    let ready = launch::pick_build(&app.registry, &profile).is_ok();
    let outline = if running { Some(color::PLAY) } else { current.then(|| color::ACCENT.gamma_multiply(0.55)) };
    let mut card = Card::new().padding(space::M);
    if let Some(outline) = outline {
        card = card.outline(outline);
    }
    let mut select = false;
    let response = card.show(ui, |ui| {
        ui.horizontal(|ui| {
            let (tile, _) = ui.allocate_exact_size(egui::vec2(46.0, 46.0), Sense::hover());
            paint::tile(ui.painter(), tile, &profile.id, &profile.name);
            if running {
                ui.painter().circle_filled(tile.right_bottom() - egui::vec2(3.0, 3.0), 6.0, color::PLAY);
            }
            ui.add_space(space::XS);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let gear =
                    icon_button(ui, icons::GEAR, &app.tr.fill("home.profile_options", &[("profile", &profile.name)]));
                probe::mark(ui.ctx(), format!("card:{id}:gear"), gear.rect);
                if gear.clicked() {
                    app.set_profile(id.to_owned());
                    app.go(Page::Profiles);
                }
                if running {
                    let stop = Pill::new(Kind::Danger, app.tr.get("play.stop")).icon(icons::STOP).show(ui);
                    probe::mark(ui.ctx(), format!("card:{id}:stop"), stop.rect);
                    if stop.clicked() {
                        app.stop(id);
                    }
                    let output = Pill::new(Kind::Ghost, app.tr.get("home.output")).icon(icons::TERMINAL).show(ui);
                    probe::mark(ui.ctx(), format!("card:{id}:output"), output.rect);
                    if output.clicked() {
                        app.log_profile = Some(id.to_owned());
                        app.go(Page::Log);
                    }
                } else {
                    let play = Pill::new(Kind::Play, app.tr.get("play.play")).icon(icons::PLAY).enabled(ready).show(ui);
                    probe::mark(ui.ctx(), format!("card:{id}:play"), play.rect);
                    if play.clicked() {
                        app.play_profile(id, None);
                    }
                }
                ui.add_space(space::S);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    let text = ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        widgets::one_line(ui, widgets::strong(&profile.name, size::TITLE));
                        let line = profile_line(app, &profile, true);
                        widgets::one_line(ui, RichText::new(line).size(size::SMALL).color(color::TEXT_MUTED));
                    });
                    let rect = text.response.rect.with_max_x(ui.max_rect().right());
                    probe::mark(ui.ctx(), format!("card:{id}:text"), text.response.rect);
                    let hit = ui.interact(rect, ui.scope_id().with(("select", id)), Sense::click());
                    if hit.clicked() {
                        select = true;
                    }
                    hit.on_hover_text(app.tr.get("home.select_profile"));
                });
            });
        });
    });
    probe::mark(ui.ctx(), format!("card:{id}"), response.response.rect);
    if select {
        app.set_profile(id.to_owned());
    }
}

fn bottom_actions(app: &mut App, ui: &mut egui::Ui) {
    let width = (ui.available_width() - space::M) / 2.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = space::M;
        let manage = Pill::new(Kind::Accent, app.tr.get("home.manage_profiles")).icon(icons::SQUARES_FOUR).width(width);
        if manage.show(ui).clicked() {
            app.go(Page::Profiles);
        }
        if Pill::new(Kind::Ghost, app.tr.get("home.add_mods")).icon(icons::PLUS).width(width).show(ui).clicked() {
            app.mods_view = ModsView::Browse;
            app.go(Page::Mods);
        }
    });
}
