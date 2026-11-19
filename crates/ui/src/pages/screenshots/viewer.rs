//! The large view of one screenshot: the picture fitted to the page, previous and next (buttons, the sides of the picture,
//! the arrow keys), Esc to go back, and the actions: open, show in the folder, copy, rename, delete.

use castanea_launcher_core::gallery::Shot;
use eframe::egui::{self, Key, Sense};

use super::clock_of;
use crate::app::App;
use crate::shell::megabytes;
use crate::theme::{self, color, line, radius, size, space};
use crate::widgets::{self, Button, Kind, button};
use crate::{probe, widgets::text_field};

/// How wide the clickable strips at the picture's left and right edge are.
const EDGE: f32 = 56.0;

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let Some(shot) = app.gallery.open_shot().cloned() else {
        return app.close_viewer();
    };
    keys(app, &ctx);
    let total = app.gallery.shots.len();
    let at = app.gallery.index_of(&shot.path).unwrap_or(0);
    ui.horizontal(|ui| {
        if Button::new(Kind::Secondary, app.tr.get("shots.back")).show(ui).clicked() {
            app.close_viewer();
        }
        ui.add_space(space::S);
        widgets::one_line(ui, widgets::strong(&shot.name, size::BODY));
    });
    ui.add_space(space::XS);
    let when = format!("{} {} · {}", day_text(&shot), clock_of(&shot), megabytes(shot.size));
    let position = app.tr.fill("shots.position", &[("n", &(at + 1).to_string()), ("total", &total.to_string())]);
    ui.label(widgets::small(format!("{when} · {position}")));
    ui.add_space(space::S);
    actions(app, ui, &shot);
    ui.add_space(space::S);
    picture(app, ui, &shot, at, total);
}

/// The date of a shot, local, as `2026-11-19`.
fn day_text(shot: &Shot) -> String {
    use castanea_launcher_core::gallery::time;
    let seconds = shot.taken_ms.div_euclid(1000);
    time::day_of(seconds + time::local_offset(seconds)).iso()
}

/// Left and right step, Home and End jump, Esc goes back (or ends a rename); a text field keeps the keys it is typed into.
fn keys(app: &mut App, ctx: &egui::Context) {
    if app.confirm.is_some() {
        return;
    }
    if app.gallery.rename.is_some() {
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            app.gallery.rename = None;
        }
        return;
    }
    let (left, right, home, end, escape, delete) = ctx.input(|i| {
        (
            i.key_pressed(Key::ArrowLeft),
            i.key_pressed(Key::ArrowRight),
            i.key_pressed(Key::Home),
            i.key_pressed(Key::End),
            i.key_pressed(Key::Escape),
            i.key_pressed(Key::Delete),
        )
    });
    // Newest first: the left arrow goes to the newer picture, the right arrow to the older one.
    if left {
        app.gallery.step(-1);
    }
    if right {
        app.gallery.step(1);
    }
    if home {
        app.gallery.step(isize::MIN / 2);
    }
    if end {
        app.gallery.step(isize::MAX / 2);
    }
    if delete && let Some(shot) = app.gallery.open_shot() {
        let path = shot.path.clone();
        app.ask_delete_shot(path);
    }
    if escape {
        app.close_viewer();
    }
}

fn actions(app: &mut App, ui: &mut egui::Ui, shot: &Shot) {
    ui.horizontal_wrapped(|ui| {
        if let Some(text) = app.gallery.rename.clone() {
            let mut text = text;
            ui.label(widgets::muted(app.tr.get("shots.rename_label")));
            let field = text_field(ui, &mut text, "", 260.0);
            if !field.has_focus() && !field.lost_focus() && app.gallery.rename.as_deref() == Some(&text) {
                field.request_focus();
            }
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            app.gallery.rename = Some(text.clone());
            let ok = Button::new(Kind::Primary, app.tr.get("common.save")).show(ui).clicked();
            if Button::new(Kind::Secondary, app.tr.get("common.cancel")).show(ui).clicked() {
                app.gallery.rename = None;
            } else if ok || enter {
                app.rename_shot(ui.ctx(), &shot.path, &text);
            }
            return;
        }
        if Button::new(Kind::Secondary, app.tr.get("shots.open")).show(ui).clicked() {
            app.open_shot_file(&shot.path);
        }
        if Button::new(Kind::Secondary, app.tr.get("shots.show")).show(ui).clicked() {
            app.show_shot_in_folder(&shot.path);
        }
        if Button::new(Kind::Secondary, app.tr.get("shots.copy")).show(ui).clicked() {
            app.copy_shot(ui.ctx(), shot.path.clone());
        }
        if Button::new(Kind::Secondary, app.tr.get("shots.rename")).show(ui).clicked() {
            let stem = std::path::Path::new(&shot.name).file_stem().map(|s| s.to_string_lossy().into_owned());
            app.gallery.rename = Some(stem.unwrap_or_default());
        }
        if Button::new(Kind::Destructive, app.tr.get("shots.delete")).show(ui).clicked() {
            app.ask_delete_shot(shot.path.clone());
        }
    });
}

/// The picture in the rest of the page with a step zone on each side.
fn picture(app: &mut App, ui: &mut egui::Ui, shot: &Shot, at: usize, total: usize) {
    let ctx = ui.ctx().clone();
    app.want_view(&ctx, &shot.path);
    // A thumbnail stands in until the full picture is there.
    app.want_thumbs(&ctx, std::slice::from_ref(shot));
    let area = ui.available_rect_before_wrap();
    probe::mark(&ctx, "shots:view", area);
    ui.painter().rect_filled(area, theme::radius(radius::M), color::BASE);
    ui.painter().rect_stroke(
        area,
        theme::radius(radius::M),
        egui::Stroke::new(line::HAIRLINE, color::BORDER),
        egui::StrokeKind::Inside,
    );
    let full = app.gallery.view.as_ref().filter(|(p, _)| *p == shot.path).map(|(_, t)| t.clone());
    let texture = full.or_else(|| app.gallery.thumbs.get(&shot.path).cloned());
    let failed = app.gallery.view_failed.as_deref() == Some(shot.path.as_path());
    let inner = area.shrink(space::S);
    if let Some(texture) = &texture {
        let size = texture.size_vec2();
        let scale = (inner.width() / size.x).min(inner.height() / size.y);
        let fitted = egui::Rect::from_center_size(inner.center(), size * scale);
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        ui.painter().image(texture.id(), fitted, uv, egui::Color32::WHITE);
    }
    let note = if failed {
        Some(app.tr.get("shots.cannot_view"))
    } else if texture.is_none() || app.gallery.view.as_ref().is_none_or(|(p, _)| *p != shot.path) {
        Some(app.tr.get("shots.loading"))
    } else {
        None
    };
    if let Some(note) = note {
        let tint = if failed { color::DANGER } else { color::TEXT_MUTED };
        let anchor =
            if texture.is_some() && !failed { area.center_bottom() - egui::vec2(0.0, space::L) } else { area.center() };
        ui.painter().text(anchor, egui::Align2::CENTER_CENTER, note, theme::font::regular(size::BODY), tint);
    }
    // Newest first: the left strip goes to the newer picture, the right one to the older one.
    let newer = at > 0;
    let older = at + 1 < total;
    let left = egui::Rect::from_min_size(area.min, egui::vec2(EDGE, area.height()));
    let right = egui::Rect::from_min_size(egui::pos2(area.right() - EDGE, area.top()), egui::vec2(EDGE, area.height()));
    if step_zone(ui, left, -1.0, newer, app.tr.get("shots.previous")) {
        app.gallery.step(-1);
    }
    if step_zone(ui, right, 1.0, older, app.tr.get("shots.next")) {
        app.gallery.step(1);
    }
    ui.allocate_rect(area, Sense::hover());
}

/// A strip at the picture's edge with a chevron; `dir` is -1 for the left strip, 1 for the right one.
fn step_zone(ui: &mut egui::Ui, rect: egui::Rect, dir: f32, enabled: bool, name: &str) -> bool {
    let id = ui.make_persistent_id(("step", dir as i32));
    let response = ui.interact(rect, id, if enabled { Sense::click() } else { Sense::hover() });
    let (hover, _) = button::hover_press(ui, &response);
    if enabled {
        let fill = theme::mix(egui::Color32::TRANSPARENT, color::HOVER, hover * 0.6);
        ui.painter().rect_filled(rect, theme::radius(radius::M), fill);
        let tint = theme::mix(color::TEXT_MUTED, color::TEXT, hover);
        let c = rect.center();
        let (r, d) = (space::S, dir);
        let points =
            vec![c + egui::vec2(-d * r / 2.0, -r), c + egui::vec2(d * r / 2.0, 0.0), c + egui::vec2(-d * r / 2.0, r)];
        ui.painter().line(points, egui::Stroke::new(line::FOCUS, tint));
        button::focus_ring(ui, &response, rect, radius::M);
    }
    let label = name.to_owned();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, &label));
    enabled && response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}
