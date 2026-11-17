//! Profiles: the list on the left, the editor of the chosen one on the right (name, how the game starts, version,
//! graphics, relay, extra arguments, environment). Changes are saved at once.

use castanea_launcher_core::launch;
use castanea_launcher_core::profile::{Graphics, StartIn};
use eframe::egui::{self, RichText};

use crate::app::{App, Confirm, open_or_note};
use crate::pages::profile_line;
use crate::theme::{color, layout, size, space};
use crate::widgets::{self, Card, Kind, Pill, icon_button, paint};
use crate::{icons, probe};

const LIST_WIDTH: f32 = 280.0;

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    let title = app.tr.get("nav.profiles").to_owned();
    widgets::page_header(ui, &title, |ui| {
        if Pill::new(Kind::Accent, app.tr.get("profile.new")).icon(icons::PLUS).show(ui).clicked() {
            let name = app.tr.get("profile.new_name").to_owned();
            let id = app.profiles.add(&name);
            app.save_profiles();
            app.set_profile(id);
        }
    });
    ui.add_space(space::M);
    let area = ui.available_rect_before_wrap();
    let wide = area.width() >= layout::NARROW;
    if wide {
        let list = area.with_max_x(area.left() + LIST_WIDTH);
        probe::mark(ui.ctx(), "profiles:list", list);
        ui.scope_builder(egui::UiBuilder::new().max_rect(list), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("profile-list")
                .auto_shrink([false, false])
                .show(ui, |ui| list_ui(app, ui));
        });
        let editor = area.with_min_x(list.right() + space::L);
        probe::mark(ui.ctx(), "profiles:editor", editor);
        ui.scope_builder(egui::UiBuilder::new().max_rect(editor), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("profile-editor")
                .auto_shrink([false, false])
                .show(ui, |ui| editor_ui(app, ui));
        });
        ui.allocate_rect(area, egui::Sense::hover());
    } else {
        egui::ScrollArea::vertical().id_salt("profiles").auto_shrink([false, false]).show(ui, |ui| {
            list_ui(app, ui);
            ui.add_space(space::M);
            editor_ui(app, ui);
        });
    }
}

fn list_ui(app: &mut App, ui: &mut egui::Ui) {
    let mut chosen = None;
    for profile in &app.profiles.profiles {
        let selected = profile.id == app.profile_id;
        let running = app.is_running(&profile.id);
        let card = Card::new().padding(space::M).clickable().outline_if(selected, color::ACCENT);
        let response = card.show(ui, |ui| {
            ui.horizontal(|ui| {
                let (tile, _) = ui.allocate_exact_size(egui::vec2(38.0, 38.0), egui::Sense::hover());
                paint::tile(ui.painter(), tile, &profile.id, &profile.name);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    widgets::one_line(ui, widgets::strong(&profile.name, size::BODY));
                    let line = crate::pages::ago(&app.tr, profile.last_played);
                    let line = if running { app.tr.get("home.running").to_owned() } else { line };
                    let tint = if running { color::PLAY } else { color::TEXT_MUTED };
                    widgets::one_line(ui, RichText::new(line).size(size::CAPTION).color(tint));
                });
            });
        });
        let name = profile.name.clone();
        response.response.widget_info(|| egui::WidgetInfo::selected(egui::Role::ListItem, true, selected, &name));
        if response.response.clicked() {
            chosen = Some(profile.id.clone());
        }
        ui.add_space(space::S);
    }
    if let Some(id) = chosen {
        app.set_profile(id);
    }
}

fn editor_ui(app: &mut App, ui: &mut egui::Ui) {
    let labels = Labels::new(app);
    let line = profile_line(app, app.profile(), true);
    let mut changed = false;
    Card::new().padding(space::XL).show(ui, |ui| {
        ui.horizontal(|ui| {
            let (tile, _) = ui.allocate_exact_size(egui::vec2(52.0, 52.0), egui::Sense::hover());
            paint::tile(ui.painter(), tile, &app.profile().id, &app.profile().name);
            ui.vertical(|ui| {
                widgets::one_line(ui, widgets::strong(&app.profile().name, size::HEADING));
                widgets::one_line(ui, widgets::muted(&line).size(size::SMALL));
            });
        });
        ui.add_space(space::L);
        version_picker(app, ui, &labels);
        ui.add_space(space::M);
        let profile = app.profile_mut();
        egui::Grid::new("profile_grid").num_columns(2).spacing([space::L, space::M]).min_col_width(150.0).show(
            ui,
            |ui| {
                let field_width = (ui.available_width() - 170.0).clamp(200.0, 420.0);
                label(ui, &labels.name);
                changed |= ui.add(egui::TextEdit::singleline(&mut profile.name).desired_width(field_width)).changed();
                ui.end_row();

                label(ui, &labels.player);
                changed |=
                    ui.add(egui::TextEdit::singleline(&mut profile.player_name).desired_width(field_width)).changed();
                ui.end_row();

                label(ui, &labels.start);
                let mut kind = match profile.start {
                    StartIn::Menu => 0,
                    StartIn::World { .. } => 1,
                    StartIn::Join { .. } => 2,
                };
                if widgets::segmented(ui, &mut kind, &[&labels.menu, &labels.world, &labels.join]) {
                    profile.start = match kind {
                        1 => StartIn::World { name: String::new(), seed: String::new(), host_port: None },
                        2 => StartIn::Join { address: String::new() },
                        _ => StartIn::Menu,
                    };
                    changed = true;
                }
                ui.end_row();

                match &mut profile.start {
                    StartIn::Menu => {}
                    StartIn::World { name, seed, host_port } => {
                        label(ui, &labels.world_name);
                        changed |= ui.add(egui::TextEdit::singleline(name).desired_width(field_width)).changed();
                        ui.end_row();
                        label(ui, &labels.seed);
                        changed |= ui
                            .add(egui::TextEdit::singleline(seed).hint_text("0x5eed").desired_width(field_width))
                            .changed();
                        ui.end_row();
                        label(ui, &labels.host);
                        ui.horizontal(|ui| {
                            let mut hosting = host_port.is_some();
                            if widgets::switch(ui, &mut hosting, &labels.host_open).changed() {
                                *host_port = hosting.then_some(47800);
                                changed = true;
                            }
                            if let Some(port) = host_port {
                                changed |= ui.add(egui::DragValue::new(port).range(1024..=65535)).changed();
                            }
                        });
                        ui.end_row();
                    }
                    StartIn::Join { address } => {
                        label(ui, &labels.address);
                        let edit =
                            egui::TextEdit::singleline(address).hint_text("10.0.0.5:47800").desired_width(field_width);
                        changed |= ui.add(edit).changed();
                        ui.end_row();
                    }
                }

                label(ui, &labels.graphics);
                let mut graphics = Graphics::ALL.iter().position(|g| *g == profile.graphics).unwrap_or(0);
                if widgets::segmented(ui, &mut graphics, &[&labels.auto, "Vulkan", "wgpu"]) {
                    profile.graphics = Graphics::ALL[graphics];
                    changed = true;
                }
                ui.end_row();

                label(ui, &labels.window);
                changed |= ui
                    .add(
                        egui::TextEdit::singleline(&mut profile.window)
                            .hint_text("1280x720")
                            .desired_width(field_width),
                    )
                    .changed();
                ui.end_row();

                label(ui, &labels.backend);
                let edit = egui::TextEdit::singleline(&mut profile.backend_url).hint_text("https://relay.example.org");
                changed |= ui.add(edit.desired_width(field_width)).changed();
                ui.end_row();

                label(ui, &labels.extra);
                let edit = egui::TextEdit::singleline(&mut profile.extra_args).hint_text("--gamemode creative");
                changed |= ui.add(edit.desired_width(field_width)).changed();
                ui.end_row();

                label(ui, &labels.env);
                ui.vertical(|ui| {
                    let mut remove = None;
                    for (n, (key, value)) in profile.env.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            changed |= ui
                                .add(egui::TextEdit::singleline(key).desired_width(140.0).hint_text("NAME"))
                                .changed();
                            ui.label("=");
                            changed |= ui.add(egui::TextEdit::singleline(value).desired_width(180.0)).changed();
                            if icon_button(ui, icons::X, &labels.remove_env).clicked() {
                                remove = Some(n);
                            }
                        });
                    }
                    if let Some(n) = remove {
                        profile.env.remove(n);
                        changed = true;
                    }
                    if Pill::new(Kind::Ghost, &labels.add_env).icon(icons::PLUS).show(ui).clicked() {
                        profile.env.push((String::new(), String::new()));
                    }
                });
                ui.end_row();
            },
        );
        ui.add_space(space::M);
        ui.label(widgets::faint(&labels.hint));
        ui.add_space(space::M);
        ui.horizontal(|ui| {
            if Pill::new(Kind::Ghost, &labels.open_folder).icon(icons::FOLDER_OPEN).show(ui).clicked() {
                let dir = app.profile().game_dir(&app.paths);
                open_or_note(app, &dir);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let can_remove = app.profiles.profiles.len() > 1 && !app.is_running(&app.profile_id);
                if Pill::new(Kind::Danger, &labels.remove).icon(icons::TRASH).enabled(can_remove).show(ui).clicked() {
                    app.confirm = Some(Confirm::RemoveProfile { id: app.profile_id.clone() });
                }
            });
        });
    });
    if changed {
        app.save_profiles();
    }
}

fn label(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).color(color::TEXT_MUTED));
}

fn version_picker(app: &mut App, ui: &mut egui::Ui, labels: &Labels) {
    let newest = launch::newest(&app.registry).map(|b| b.id.clone());
    let newest_text = match &newest {
        Some(id) => app.tr.fill("play.newest", &[("version", id)]),
        None => app.tr.get("play.none_installed").to_owned(),
    };
    let current = app.profile().build.clone();
    let shown = current.clone().unwrap_or_else(|| newest_text.clone());
    let mut chosen: Option<Option<String>> = None;
    ui.horizontal(|ui| {
        ui.label(widgets::icon(icons::CUBE, 18.0, color::TEXT_MUTED));
        ui.label(RichText::new(&labels.version).color(color::TEXT_MUTED));
        egui::ComboBox::from_id_salt("version").width(260.0).selected_text(shown).show_ui(ui, |ui| {
            if ui.selectable_label(current.is_none(), &newest_text).clicked() {
                chosen = Some(None);
            }
            for build in &app.registry.builds {
                let text = format!("{}  ({})", build.id, build.version);
                let label = ui.add_enabled(
                    build.is_present(),
                    egui::Button::selectable(current.as_ref() == Some(&build.id), text),
                );
                if label.clicked() {
                    chosen = Some(Some(build.id.clone()));
                }
            }
        });
    });
    if let Some(build) = chosen {
        app.profile_mut().build = build;
        app.save_profiles();
    }
}

/// The editor's strings, taken before the profile is borrowed.
struct Labels {
    name: String,
    start: String,
    menu: String,
    world: String,
    join: String,
    world_name: String,
    seed: String,
    host: String,
    host_open: String,
    address: String,
    player: String,
    backend: String,
    graphics: String,
    auto: String,
    window: String,
    extra: String,
    env: String,
    add_env: String,
    remove_env: String,
    hint: String,
    version: String,
    open_folder: String,
    remove: String,
}

impl Labels {
    fn new(app: &App) -> Self {
        let t = |key: &str| app.tr.get(key).to_owned();
        Self {
            name: t("profile.name"),
            start: t("profile.start"),
            menu: t("profile.start_menu"),
            world: t("profile.start_world"),
            join: t("profile.start_join"),
            world_name: t("profile.world"),
            seed: t("profile.seed"),
            host: t("profile.host"),
            host_open: t("profile.host_open"),
            address: t("profile.address"),
            player: t("profile.player"),
            backend: t("profile.backend"),
            graphics: t("profile.graphics"),
            auto: t("profile.graphics_auto"),
            window: t("profile.window"),
            extra: t("profile.extra"),
            env: t("profile.env"),
            add_env: t("profile.add_env"),
            remove_env: t("profile.remove_env"),
            hint: t("profile.hint"),
            version: t("play.version"),
            open_folder: t("play.open_folder"),
            remove: t("profile.remove"),
        }
    }
}
