//! Screenshots: the pictures the game took, newest first and grouped by day, as a grid of thumbnails that are made in the
//! background. Only the rows in view are drawn, so a folder of thousands of files costs no more than a screenful. A click
//! opens the large view (`screenshots/viewer.rs`).

mod viewer;

use castanea_launcher_core::gallery::{Shot, time};
use eframe::egui::{self, Role, Sense, WidgetInfo};

use crate::app::{App, open_or_note};
use crate::gallery::{GAP, Layout, RowKind, today};
use crate::icons;
use crate::theme::{self, color, font, radius, size, space};
use crate::widgets::{self, Button, Kind, icon_button};
use crate::{probe, widgets::button};

/// The smallest a thumbnail is drawn; the grid takes as many columns as fit.
const CELL_MIN: f32 = 200.0;
/// Rows kept ready above and below the ones in view.
const OVERSCAN: usize = 1;

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    if app.gallery.viewer.is_some() {
        return viewer::ui(app, ui);
    }
    let title = app.tr.get("nav.screenshots").to_owned();
    widgets::page_header(ui, &title, |ui| {
        if icon_button(ui, icons::ARROWS_CLOCKWISE, app.tr.get("common.refresh")).clicked() {
            app.gallery.stale = true;
        }
        if Button::new(Kind::Secondary, app.tr.get("shots.open_folder")).show(ui).clicked() {
            let dir = app.paths.screenshots();
            open_or_note(app, &dir);
        }
    });
    if !app.gallery.scanned {
        return widgets::empty(ui, app.tr.get("shots.scanning"));
    }
    if app.gallery.shots.is_empty() {
        probe::mark(ui.ctx(), "shots:empty", ui.available_rect_before_wrap());
        return widgets::empty(ui, app.tr.get("shots.empty"));
    }
    if app.gallery.unreadable > 0 {
        let text = app.tr.fill("shots.unreadable_files", &[("n", &app.gallery.unreadable.to_string())]);
        ui.label(widgets::small(text));
        ui.add_space(space::S);
    }
    grid(app, ui);
}

/// "Today", "Yesterday", or the date.
fn day_label(app: &App, day: time::Day) -> String {
    let now = today();
    if day == now {
        app.tr.get("shots.today").to_owned()
    } else if day.days() == now.days() - 1 {
        app.tr.get("shots.yesterday").to_owned()
    } else {
        day.iso()
    }
}

/// `14:32`, local time.
pub fn clock_of(shot: &Shot) -> String {
    let seconds = shot.taken_ms.div_euclid(1000);
    let local = (seconds + time::local_offset(seconds)).rem_euclid(86_400);
    format!("{:02}:{:02}", local / 3600, local / 60 % 60)
}

fn grid(app: &mut App, ui: &mut egui::Ui) {
    let width = ui.available_width();
    let cols = (((width + GAP) / (CELL_MIN + GAP)).floor() as usize).max(1);
    let cell_w = (width - GAP * (cols - 1) as f32) / cols as f32;
    let cell_h = (cell_w * 9.0 / 16.0).round();
    let stale = app
        .gallery
        .layout
        .as_ref()
        .is_none_or(|l| l.version != app.gallery.version || l.cols != cols || (l.cell_h - cell_h).abs() > 0.5);
    if stale {
        app.gallery.layout = Some(Layout::build(&app.gallery.groups, cols, cell_h, app.gallery.version));
    }
    let Some(layout) = app.gallery.layout.clone() else { return };
    let mut wanted: Vec<Shot> = Vec::new();
    let mut ahead: Vec<Shot> = Vec::new();
    let mut opened = None;
    let area = ui.available_rect_before_wrap();
    probe::mark(ui.ctx(), "shots:grid", area);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show_viewport(ui, |ui, viewport| {
        ui.set_height(layout.total);
        let origin = ui.max_rect().min;
        let rows = layout.visible(viewport.min.y, viewport.max.y, OVERSCAN);
        let in_view = layout.visible(viewport.min.y, viewport.max.y, 0);
        for (at, row) in layout.rows[rows.clone()].iter().enumerate().map(|(n, r)| (rows.start + n, r)) {
            let top = origin.y + layout.tops[at];
            match row {
                RowKind::Header(group) => {
                    let g = &app.gallery.groups[*group];
                    let label = day_label(app, g.day);
                    let n = g.range.len();
                    let count = if n == 1 {
                        app.tr.get("shots.count_one").to_owned()
                    } else {
                        app.tr.fill("shots.count", &[("n", &n.to_string())])
                    };
                    let rect = egui::Rect::from_min_size(egui::pos2(origin.x, top), egui::vec2(width, 44.0));
                    header(ui, rect, &label, &count);
                }
                RowKind::Cells(range) => {
                    for (c, index) in range.clone().enumerate() {
                        let x = origin.x + c as f32 * (cell_w + GAP);
                        let rect = egui::Rect::from_min_size(egui::pos2(x, top), egui::vec2(cell_w, cell_h));
                        let shot = app.gallery.shots[index].clone();
                        if !in_view.contains(&at) {
                            ahead.push(shot);
                            continue;
                        }
                        if cell(app, ui, rect, &shot, index).clicked() {
                            opened = Some(shot.path.clone());
                        }
                        wanted.push(shot);
                    }
                }
            }
        }
    });
    // The ones in view first, then the rows kept ready beside them.
    wanted.extend(ahead);
    app.want_thumbs(ui.ctx(), &wanted);
    if let Some(path) = opened {
        app.open_viewer(path);
    }
}

fn header(ui: &mut egui::Ui, rect: egui::Rect, label: &str, count: &str) {
    let painter = ui.painter();
    let baseline = egui::pos2(rect.left(), rect.bottom() - space::S);
    let title = painter.layout_no_wrap(label.to_owned(), font::semibold(size::BODY), color::TEXT);
    let wide = title.size().x;
    painter.galley(egui::pos2(baseline.x, baseline.y - title.size().y), title, color::TEXT);
    painter.text(
        egui::pos2(baseline.x + wide + space::M, baseline.y),
        egui::Align2::LEFT_BOTTOM,
        count,
        font::regular(size::SMALL),
        color::TEXT_MUTED,
    );
}

/// One thumbnail: the picture fitted into its cell, or a placeholder while it loads or when the file cannot be read.
fn cell(app: &mut App, ui: &mut egui::Ui, rect: egui::Rect, shot: &Shot, index: usize) -> egui::Response {
    let response = ui.interact(rect, ui.make_persistent_id(("shot", index)), Sense::click());
    let (hover, _) = button::hover_press(ui, &response);
    let failed = app.gallery.failed.contains(&shot.path);
    let painter = ui.painter();
    painter.rect_filled(rect, theme::radius(radius::M), color::BASE);
    if let Some(texture) = app.gallery.thumbs.get(&shot.path) {
        let size = texture.size_vec2();
        let scale = (rect.width() / size.x).min(rect.height() / size.y);
        let fitted = egui::Rect::from_center_size(rect.center(), size * scale);
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        painter.image(texture.id(), fitted, uv, egui::Color32::WHITE);
    } else if failed {
        painter.text(
            rect.center() - egui::vec2(0.0, space::S),
            egui::Align2::CENTER_CENTER,
            icons::WARNING,
            font::icon(size::ICON),
            color::TEXT_MUTED,
        );
        painter.text(
            rect.center() + egui::vec2(0.0, space::M),
            egui::Align2::CENTER_CENTER,
            app.tr.get("shots.cannot_read"),
            font::regular(size::SMALL),
            color::TEXT_MUTED,
        );
    }
    let outline = theme::mix(color::BORDER, color::BORDER_STRONG, hover);
    painter.rect_stroke(
        rect,
        theme::radius(radius::M),
        egui::Stroke::new(theme::line::HAIRLINE, outline),
        egui::StrokeKind::Inside,
    );
    let clock = clock_of(shot);
    let pill = painter.layout_no_wrap(clock.clone(), font::regular(size::SMALL), color::TEXT);
    let pill_rect = egui::Rect::from_min_size(
        rect.left_bottom() + egui::vec2(space::S, -space::S - pill.size().y - space::XS),
        pill.size() + egui::vec2(space::S, space::XS),
    );
    if app.gallery.thumbs.get(&shot.path).is_some() {
        let painter = ui.painter();
        painter.rect_filled(pill_rect, theme::radius(radius::S), egui::Color32::from_black_alpha(150));
        painter.galley(pill_rect.min + egui::vec2(space::XS, space::XS / 2.0), pill, color::TEXT);
    }
    button::focus_ring(ui, &response, rect, radius::M);
    let name = format!("{} {}", shot.name, clock);
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, &name));
    response.on_hover_text(&shot.name).on_hover_cursor(egui::CursorIcon::PointingHand)
}
