//! Notifications: every note of this run, newest first, with its age; dismiss one or all.

use eframe::egui::{self, RichText};

use crate::app::{App, NoteKind};
use crate::icons;
use crate::pages::mods::empty;
use crate::theme::{color, size, space};
use crate::widgets::{self, Card, Kind, Pill, icon_button};

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    app.notes.iter_mut().for_each(|n| n.seen = true);
    let title = app.tr.get("nav.notifications").to_owned();
    let mut clear = false;
    widgets::page_header(ui, &title, |ui| {
        let pill = Pill::new(Kind::Ghost, app.tr.get("notes.clear")).icon(icons::TRASH).enabled(!app.notes.is_empty());
        clear = pill.show(ui).clicked();
    });
    ui.add_space(space::M);
    if clear {
        app.notes.clear();
    }
    if app.notes.is_empty() {
        empty(ui, icons::BELL, app.tr.get("notes.none"));
        return;
    }
    let mut remove = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (n, note) in app.notes.iter().enumerate().rev() {
            let (glyph, tint) = match note.kind {
                NoteKind::Info => (icons::CHECK_CIRCLE, color::PLAY),
                NoteKind::Error => (icons::WARNING, color::DANGER),
            };
            Card::new().padding(space::M).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(widgets::icon(glyph, 20.0, tint));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icon_button(ui, icons::X, app.tr.get("common.dismiss")).clicked() {
                            remove = Some(n);
                        }
                        let age = note.created.elapsed().as_secs();
                        let when = crate::pages::ago(&app.tr, Some(crate::app::now_seconds().saturating_sub(age)));
                        ui.label(widgets::faint(when));
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.add(
                                egui::Label::new(RichText::new(&note.text).color(color::TEXT).size(size::BODY)).wrap(),
                            );
                        });
                    });
                });
            });
            ui.add_space(space::S);
        }
    });
    if let Some(n) = remove {
        app.notes.remove(n);
    }
}
