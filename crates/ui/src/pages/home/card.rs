//! The Continue card under the Play block (docs/quick-start.md): the last world or server played, its picture, when and
//! with which version; one click starts the game straight in.

use std::sync::Arc;

use castanea_launcher_core::gallery::time;
use castanea_launcher_core::history::{self, Ago, Played};
use eframe::egui::{self, Role, Sense, WidgetInfo};

use crate::app::App;
use crate::theme::{self, color, font, layout, line, radius, size, space};
use crate::widgets::button;
use crate::{icons, probe};

/// The picture's size inside the card; the card's padding is the same on all sides.
const PICTURE: [f32; 2] = [96.0, 54.0];

/// The height the card takes with the gap above it, 0 when it is not shown.
pub fn height(app: &App) -> f32 {
    if app.card_visible() { layout::CONTINUE[1] + space::XL } else { 0.0 }
}

/// How long ago, in words of the language of the UI; a month or more is the date.
pub fn when_text(app: &App, when_ms: i64) -> String {
    let tr = &app.tr;
    match history::ago(history::now_ms(), when_ms) {
        Ago::JustNow => tr.get("home.ago_now").to_owned(),
        Ago::Minutes(n) => tr.fill("home.ago_minutes", &[("n", &n.to_string())]),
        Ago::Hours(n) => tr.fill("home.ago_hours", &[("n", &n.to_string())]),
        Ago::Days(n) => tr.fill("home.ago_days", &[("n", &n.to_string())]),
        Ago::Date => {
            let seconds = when_ms.div_euclid(1000);
            time::day_of(seconds + time::local_offset(seconds)).iso()
        }
    }
}

/// `World · 2 h ago · 0.2.0`.
pub fn detail(app: &App, played: &Played) -> String {
    let mut parts = vec![app.tr.get(App::card_kind_key(&played.start)).to_owned(), when_text(app, played.when_ms)];
    parts.extend(played.version.clone());
    parts.join(" · ")
}

/// One line of text cut with an ellipsis at `width`.
fn fitted(
    painter: &egui::Painter,
    text: &str,
    font: egui::FontId,
    tint: egui::Color32,
    width: f32,
) -> Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, tint);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width);
    painter.layout_job(job)
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let Some(played) = app.quick.played.clone().filter(|_| app.card_visible()) else { return };
    let (width, card_h) = (layout::CONTINUE[0], layout::CONTINUE[1]);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, card_h), Sense::hover());
    let enabled = app.can_play();
    let response = ui.interact(
        rect,
        ui.make_persistent_id("continue-card"),
        if enabled { Sense::click() } else { Sense::hover() },
    );
    let (hover, press) = button::hover_press(ui, &response);
    let pad = (card_h - PICTURE[1]) / 2.0;
    let picture = egui::Rect::from_min_size(rect.min + egui::vec2(pad, pad), egui::vec2(PICTURE[0], PICTURE[1]));
    let shot = app.quick.picture.clone();
    if let Some(shot) = &shot {
        app.want_thumbs(ui.ctx(), std::slice::from_ref(shot));
    }
    let fill = theme::mix(theme::mix(color::RAISED, color::HOVER, hover), color::PRESSED, press);
    let painter = ui.painter();
    painter.rect_filled(rect, theme::radius(radius::M), fill);
    painter.rect_filled(picture, theme::radius(radius::S), color::BASE);
    match shot.as_ref().and_then(|s| app.gallery.thumbs.get(&s.path)) {
        Some(texture) => {
            let size = texture.size_vec2();
            let scale = (picture.width() / size.x).max(picture.height() / size.y);
            let crop = egui::vec2(picture.width() / (size.x * scale), picture.height() / (size.y * scale));
            let uv = egui::Rect::from_center_size(egui::pos2(0.5, 0.5), crop);
            painter.image(texture.id(), picture, uv, egui::Color32::WHITE);
        }
        None => {
            painter.text(
                picture.center(),
                egui::Align2::CENTER_CENTER,
                icons::PLAY,
                font::icon(size::ICON),
                color::TEXT_MUTED,
            );
        }
    }
    let x = picture.right() + space::M;
    let text_w = rect.right() - x - space::M;
    let accent = if enabled { color::ACCENT } else { color::TEXT_DISABLED };
    let lines = [
        fitted(painter, app.tr.get("home.continue"), font::semibold(size::SMALL), accent, text_w),
        fitted(painter, &played.label, font::semibold(size::BODY), color::TEXT, text_w),
        fitted(painter, &detail(app, &played), font::regular(size::SMALL), color::TEXT_MUTED, text_w),
    ];
    let total: f32 = lines.iter().map(|g| g.size().y).sum::<f32>() + layout::LINE_GAP * 2.0;
    let mut y = rect.center().y - total / 2.0;
    for galley in lines {
        let height = galley.size().y;
        painter.galley(egui::pos2(x, y), galley, color::TEXT);
        y += height + layout::LINE_GAP;
    }
    let outline = theme::mix(color::BORDER, color::BORDER_STRONG, hover);
    painter.rect_stroke(
        rect,
        theme::radius(radius::M),
        egui::Stroke::new(line::HAIRLINE, outline),
        egui::StrokeKind::Inside,
    );
    button::focus_ring(ui, &response, rect, radius::M);
    let name = app.tr.fill("home.continue_name", &[("name", &played.label), ("detail", &detail(app, &played))]);
    response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, &name));
    probe::mark(ui.ctx(), "home:continue", rect);
    if response.clicked() {
        app.continue_play(ui.ctx());
    }
    let _ = response.on_hover_cursor(egui::CursorIcon::PointingHand);
}
