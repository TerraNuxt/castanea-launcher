//! The profile editor on the Play tab: name, how the game starts, player, backend, graphics, extra args, env.

use castanea_launcher_core::profile::{Graphics, StartIn};
use eframe::egui::{self, RichText};

use crate::app::{App, Confirm};
use crate::theme;

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    let tr_new = app.tr.get("profile.new").to_owned();
    let tr_remove = app.tr.get("profile.remove").to_owned();
    ui.horizontal(|ui| {
        if ui.button(&tr_new).clicked() {
            let name = app.tr.get("profile.new_name").to_owned();
            let id = app.profiles.add(&name);
            app.save_profiles();
            app.set_profile(id);
        }
        let can_remove = app.profiles.profiles.len() > 1;
        if ui.add_enabled(can_remove, egui::Button::new(&tr_remove)).clicked() {
            app.confirm = Some(Confirm::RemoveProfile { id: app.profile_id.clone() });
        }
    });
    let labels = Labels::new(app);
    let mut changed = false;
    let profile = app.profile_mut();
    egui::Grid::new("profile_grid").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
        ui.label(&labels.name);
        changed |= ui.text_edit_singleline(&mut profile.name).changed();
        ui.end_row();

        ui.label(&labels.start);
        ui.horizontal(|ui| {
            let mut kind = match profile.start {
                StartIn::Menu => 0,
                StartIn::World { .. } => 1,
                StartIn::Join { .. } => 2,
            };
            let before = kind;
            ui.selectable_value(&mut kind, 0, &labels.menu);
            ui.selectable_value(&mut kind, 1, &labels.world);
            ui.selectable_value(&mut kind, 2, &labels.join);
            if kind != before {
                profile.start = match kind {
                    1 => StartIn::World { name: String::new(), seed: String::new(), host_port: None },
                    2 => StartIn::Join { address: String::new() },
                    _ => StartIn::Menu,
                };
                changed = true;
            }
        });
        ui.end_row();

        match &mut profile.start {
            StartIn::Menu => {}
            StartIn::World { name, seed, host_port } => {
                ui.label(&labels.world_name);
                changed |= ui.text_edit_singleline(name).changed();
                ui.end_row();
                ui.label(&labels.seed);
                changed |= ui.add(egui::TextEdit::singleline(seed).hint_text("0x5eed")).changed();
                ui.end_row();
                ui.label(&labels.host);
                ui.horizontal(|ui| {
                    let mut hosting = host_port.is_some();
                    if ui.checkbox(&mut hosting, &labels.host_open).changed() {
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
                ui.label(&labels.address);
                changed |= ui.add(egui::TextEdit::singleline(address).hint_text("10.0.0.5:47800")).changed();
                ui.end_row();
            }
        }

        ui.label(&labels.player);
        changed |= ui.text_edit_singleline(&mut profile.player_name).changed();
        ui.end_row();

        ui.label(&labels.backend);
        changed |= ui
            .add(egui::TextEdit::singleline(&mut profile.backend_url).hint_text("https://relay.example.org"))
            .changed();
        ui.end_row();

        ui.label(&labels.graphics);
        ui.horizontal(|ui| {
            for (graphics, label) in Graphics::ALL.into_iter().zip([&labels.auto, &labels.vulkan, &labels.wgpu]) {
                changed |= ui.selectable_value(&mut profile.graphics, graphics, label.as_str()).changed();
            }
        });
        ui.end_row();

        ui.label(&labels.window);
        changed |= ui.add(egui::TextEdit::singleline(&mut profile.window).hint_text("1280x720")).changed();
        ui.end_row();

        ui.label(&labels.extra);
        changed |=
            ui.add(egui::TextEdit::singleline(&mut profile.extra_args).hint_text("--gamemode creative")).changed();
        ui.end_row();

        ui.label(&labels.env);
        ui.vertical(|ui| {
            let mut remove = None;
            for (n, (key, value)) in profile.env.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    changed |= ui.add(egui::TextEdit::singleline(key).desired_width(160.0).hint_text("NAME")).changed();
                    ui.label("=");
                    changed |= ui.add(egui::TextEdit::singleline(value).desired_width(200.0)).changed();
                    if ui.small_button("✕").clicked() {
                        remove = Some(n);
                    }
                });
            }
            if let Some(n) = remove {
                profile.env.remove(n);
                changed = true;
            }
            if ui.small_button(&labels.add_env).clicked() {
                profile.env.push((String::new(), String::new()));
            }
        });
        ui.end_row();
    });
    ui.add_space(6.0);
    ui.label(RichText::new(&labels.hint).small().color(theme::MUTED));
    if changed {
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
    vulkan: String,
    wgpu: String,
    window: String,
    extra: String,
    env: String,
    add_env: String,
    hint: String,
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
            vulkan: "Vulkan".to_owned(),
            wgpu: "wgpu".to_owned(),
            window: t("profile.window"),
            extra: t("profile.extra"),
            env: t("profile.env"),
            add_env: t("profile.add_env"),
            hint: t("profile.hint"),
        }
    }
}
