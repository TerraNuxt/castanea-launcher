//! About: version, licence and font credits, links, keyboard shortcuts, and the safety rules in short.

use eframe::egui::{self, RichText};

use crate::app::App;
use crate::icons;
use crate::theme::{color, font, layout, size, space};
use crate::widgets::{self, Card, paint};

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        Card::new().padding(space::XL).accent(color::ACCENT).show(ui, |ui| {
            ui.horizontal(|ui| {
                let (mark, _) = ui.allocate_exact_size(egui::vec2(64.0, 64.0), egui::Sense::hover());
                paint::chestnut(ui.painter(), mark.center(), 64.0, color::ACCENT);
                ui.vertical(|ui| {
                    ui.label(RichText::new("castanea launcher").font(font::bold(size::DISPLAY)).color(color::TEXT));
                    ui.label(widgets::muted(format!("{} {}", app.tr.get("about.version"), env!("CARGO_PKG_VERSION"))));
                });
            });
            ui.add_space(space::M);
            ui.add(egui::Label::new(app.tr.get("about.text")).wrap());
            ui.horizontal(|ui| {
                ui.label(widgets::icon(icons::GITHUB, 16.0, color::ACCENT));
                ui.hyperlink_to("github.com/TerraNuxt/castanea-launcher", env!("CARGO_PKG_REPOSITORY"));
            });
            ui.label(widgets::faint(app.tr.get("about.license")));
            ui.label(widgets::faint(app.tr.get("about.fonts")));
        });
        ui.add_space(space::L);
        let wide = ui.available_width() >= layout::WIDE;
        if wide {
            ui.columns(2, |cols| {
                keys(app, &mut cols[0]);
                safety(app, &mut cols[1]);
            });
        } else {
            keys(app, ui);
            ui.add_space(space::L);
            safety(app, ui);
        }
    });
}

fn keys(app: &App, ui: &mut egui::Ui) {
    Card::new().padding(space::XL).show(ui, |ui| {
        widgets::section_title(ui, icons::KEYBOARD, app.tr.get("about.keys"));
        ui.add_space(space::S);
        egui::Grid::new("keys").num_columns(2).spacing([space::XL, space::S]).show(ui, |ui| {
            for (key, text) in [
                ("Ctrl+1 … Ctrl+8", "about.key_tabs"),
                ("F5", "about.key_play"),
                ("Ctrl+F", "about.key_search"),
                ("Tab / Shift+Tab", "about.key_focus"),
                ("Enter / Space", "about.key_activate"),
                ("Esc", "about.key_escape"),
            ] {
                widgets::chip(ui, key, color::TEXT_MUTED);
                ui.label(app.tr.get(text));
                ui.end_row();
            }
        });
    });
}

fn safety(app: &App, ui: &mut egui::Ui) {
    Card::new().padding(space::XL).show(ui, |ui| {
        widgets::section_title(ui, icons::SHIELD_CHECK, app.tr.get("about.security"));
        ui.add_space(space::S);
        ui.add(egui::Label::new(RichText::new(app.tr.get("about.security_text")).color(color::TEXT_MUTED)).wrap());
    });
}
