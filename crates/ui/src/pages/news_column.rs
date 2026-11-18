//! The right column of the home page: the news, and a small "What's new in this version" below it. Everything shown
//! here is text from the server, drawn as text only; links open the system browser and are https only.

use castanea_launcher_core::news::Item;
use castanea_launcher_core::news::text::{self, Line, Span};
use eframe::egui::{self, RichText};

use crate::app::{App, open_url};
use crate::news::Phase;
use crate::probe;
use crate::theme::{color, font, size, space};
use crate::widgets::{self, Button, Kind};

/// Characters of a body shown before "Show more".
pub const PREVIEW: usize = 200;
/// Lines of the release notes shown in the small section.
pub const NOTE_LINES: usize = 5;
/// The height of one note line (small text and the gap under it).
const NOTE_ROW: f32 = size::SMALL * 1.3 + space::XS;
/// The height the small section takes at the bottom of the column: a hairline with its gaps, the title, the lines and
/// the link (tests check that it fits).
pub const NOTES_HEIGHT: f32 =
    space::M * 2.0 + 1.0 + (size::BODY * 1.3 + space::XS) + space::S + NOTE_LINES as f32 * NOTE_ROW + space::S + 24.0;

/// What a click in the column asks for; applied after drawing, when the items are no longer borrowed.
#[derive(Default)]
struct Clicks {
    open_url: Option<String>,
    toggle: Option<String>,
}

/// The news: a title with the offline mark, then the items or one quiet line.
pub fn news(app: &mut App, ui: &mut egui::Ui) {
    let mut clicks = Clicks::default();
    header(app, ui);
    let items = &app.news.items;
    if items.is_empty() {
        let key = match app.news.phase {
            Phase::Idle | Phase::Loading => "news.loading",
            Phase::Fresh => "news.empty",
            Phase::Offline => "news.offline_none",
            Phase::Failed => "news.unavailable",
        };
        ui.add_space(space::M);
        widgets::wrapped(ui, widgets::small(app.tr.get(key)));
    }
    for (n, item) in items.iter().enumerate() {
        let open = app.news.open.contains(&item.id);
        let shown = ui.scope(|ui| entry(&app.tr, ui, item, open, n == 0, &mut clicks));
        let visible = shown.response.rect.intersect(ui.clip_rect());
        if visible.is_positive() {
            probe::mark(ui.ctx(), format!("home:news:item:{n}"), visible);
        }
    }
    if let Some(id) = clicks.toggle
        && !app.news.open.remove(&id)
    {
        app.news.open.insert(id);
    }
    if let Some(url) = clicks.open_url {
        open_url(app, &url);
    }
}

fn header(app: &App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(widgets::strong(app.tr.get("news.title"), size::BODY));
        if app.news.stale() && !app.news.items.is_empty() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(widgets::small(app.tr.get("news.offline_saved")))
                    .on_hover_text(app.tr.get("news.offline_hint"));
            });
        }
    });
    ui.add_space(space::XS);
    widgets::hairline(ui);
}

/// One item: title, date and tag, the text (shortened until opened) and its quiet links.
fn entry(tr: &crate::i18n::Strings, ui: &mut egui::Ui, item: &Item, open: bool, first: bool, clicks: &mut Clicks) {
    if !first {
        widgets::hairline(ui);
    }
    ui.add_space(space::M);
    ui.spacing_mut().item_spacing.y = space::XS;
    widgets::wrapped(ui, widgets::strong(&item.title, size::BODY));
    let meta: Vec<&str> = [item.day(), item.tag.as_str()].into_iter().filter(|s| !s.is_empty()).collect();
    if !meta.is_empty() {
        ui.label(widgets::small(meta.join("  ·  ")));
    }
    let lines = text::lines(&item.body);
    let (lines, cut) = if open { (lines, false) } else { text::clip(&lines, PREVIEW) };
    rich(ui, &lines, clicks);
    let more = cut || (open && item.body.chars().count() > PREVIEW);
    if more || item.url.is_some() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = space::XL;
            if more
                && Button::new(Kind::Quiet, tr.get(if open { "news.less" } else { "news.more" }))
                    .small()
                    .show(ui)
                    .clicked()
            {
                clicks.toggle = Some(item.id.clone());
            }
            if let Some(url) = &item.url
                && Button::new(Kind::Quiet, tr.get("news.read_online")).small().show(ui).on_hover_text(url).clicked()
            {
                clicks.open_url = Some(url.clone());
            }
        });
    }
    ui.add_space(space::S);
}

/// Lines of spans as wrapped text: muted, bold in the main colour, links underlined.
fn rich(ui: &mut egui::Ui, lines: &[Line], clicks: &mut Clicks) {
    for line in lines {
        if line.is_empty() {
            ui.add_space(space::XS);
            continue;
        }
        // A wrapped row starts as tall as a control; text lines are shorter.
        ui.spacing_mut().interact_size.y = size::BODY * 1.3;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for span in line {
                match span {
                    Span::Text(t) => {
                        ui.label(RichText::new(t).font(font::regular(size::BODY)).color(color::TEXT_MUTED));
                    }
                    Span::Bold(t) => {
                        ui.label(RichText::new(t).font(font::semibold(size::BODY)).color(color::TEXT));
                    }
                    Span::Link { text, url } => {
                        let label = RichText::new(text).font(font::regular(size::BODY)).color(color::TEXT).underline();
                        if ui.add(egui::Link::new(label)).on_hover_text(url).clicked() {
                            clicks.open_url = Some(url.clone());
                        }
                    }
                }
            }
        });
    }
}

/// "What's new in this version": the first lines of the release notes and a link to all of them.
pub fn whats_new(app: &mut App, ui: &mut egui::Ui) {
    let Some((version, notes)) = app.whats_new() else { return };
    let top = ui.cursor().top();
    ui.spacing_mut().item_spacing.y = space::XS;
    ui.add_space(space::M);
    widgets::hairline(ui);
    ui.add_space(space::M);
    ui.label(widgets::strong(app.tr.fill("news.whats_new", &[("version", &version)]), size::BODY));
    ui.add_space(space::S);
    for line in notes.lines().map(str::trim).filter(|l| !l.is_empty()).take(NOTE_LINES) {
        widgets::one_line(ui, widgets::small(line));
    }
    ui.add_space(space::S);
    if Button::new(Kind::Quiet, app.tr.get("news.show_all")).small().show(ui).clicked() {
        app.show_notes = true;
    }
    let used = ui.min_rect();
    probe::mark(ui.ctx(), "home:notes:body", egui::Rect::from_min_max(egui::pos2(used.left(), top), used.max));
}
