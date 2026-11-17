//! Home: the name, one large Play button (Stop with the running time while the game runs), one quiet status line, and
//! a link to the game output once the game has run. Nothing else.

use eframe::egui::{self, Sense};

use crate::app::{App, Page};
use crate::game::{Update, clock};
use crate::jobs;
use crate::theme::{color, layout, line, radius, size, space};
use crate::widgets::{self, Button, Kind};
use crate::{probe, theme};

/// The block's height for centring: name, gaps, button, progress, status and link lines.
const BLOCK: f32 = size::DISPLAY * 1.25 + space::XXL + layout::PLAY[1] + space::L * 2.0 + size::BODY * 3.0;

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    let top = ((ui.available_height() - BLOCK) * layout::HOME_RISE).max(0.0);
    ui.add_space(top);
    ui.vertical_centered(|ui| {
        let name = ui.label(widgets::strong("castanea", size::DISPLAY));
        probe::mark(ui.ctx(), "home:wordmark", name.rect);
        ui.add_space(space::XXL);
        play_button(app, ui);
        progress_bar(app, ui);
        ui.add_space(space::M);
        let (text, tint) = status_line(app);
        let status = ui.label(egui::RichText::new(text).size(size::SMALL).color(tint));
        let status = match &app.update {
            Update::Failed(error) => status.on_hover_text(error),
            _ => status,
        };
        probe::mark(ui.ctx(), "home:status", status.rect);
        links(app, ui);
    });
}

fn play_button(app: &mut App, ui: &mut egui::Ui) {
    let response = match app.running_seconds() {
        Some(seconds) => {
            let stop = app.tr.get("play.stop").to_owned();
            let name = app.tr.fill("play.stop_running", &[("time", &clock(seconds))]);
            let response = Button::new(Kind::Secondary, &stop).large().trailing(clock(seconds)).name(name).show(ui);
            if response.clicked() {
                app.stop();
            }
            response
        }
        None => {
            let play = app.tr.get("play.play").to_owned();
            let response = Button::new(Kind::Primary, &play).large().enabled(app.can_play()).show(ui);
            if response.clicked() {
                app.play();
            }
            response.on_hover_text("F5")
        }
    };
    probe::mark(ui.ctx(), "home:play", response.rect);
}

/// A 2 px bar under the button while a new build downloads; the same height stays reserved otherwise.
fn progress_bar(app: &App, ui: &mut egui::Ui) {
    ui.add_space(space::S);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(layout::PLAY[0], line::PROGRESS), Sense::hover());
    let Update::Downloading { .. } = app.update else { return };
    let fraction = jobs::get(&app.update_progress).and_then(|p| p.fraction()).unwrap_or(0.0);
    ui.painter().rect_filled(rect, theme::radius(radius::S), color::RAISED);
    let done = rect.with_max_x(rect.left() + rect.width() * fraction);
    ui.painter().rect_filled(done, theme::radius(radius::S), color::ACCENT);
}

/// The one line under the button: which build, and where its update stands.
pub fn status_line(app: &App) -> (String, egui::Color32) {
    let tr = &app.tr;
    let build = match (&app.installed, app.dev_folder()) {
        (Some(build), Some(_)) => Some(tr.fill("home.local_build", &[("version", &build.version)])),
        (Some(build), None) => Some(tr.fill("home.version", &[("version", &build.version)])),
        (None, _) => None,
    };
    let percent = || {
        let fraction = jobs::get(&app.update_progress).and_then(|p| p.fraction()).unwrap_or(0.0);
        format!("{:.0}", fraction * 100.0)
    };
    let state = match &app.update {
        Update::Idle | Update::Local => None,
        Update::Checking => Some(tr.get("home.checking").to_owned()),
        Update::UpToDate => Some(tr.get("home.up_to_date").to_owned()),
        Update::Downloading { version } => {
            Some(tr.fill("home.downloading", &[("version", version), ("percent", &percent())]))
        }
        Update::Ready { version } => Some(tr.fill("home.ready", &[("version", version)])),
        Update::Applying { version } => Some(tr.fill("home.applying", &[("version", version)])),
        Update::NoRelease if build.is_none() => Some(tr.get("home.no_release").to_owned()),
        Update::NoRelease => None,
        Update::Offline if build.is_none() => Some(tr.get("home.offline_none").to_owned()),
        Update::Offline => Some(tr.get("home.offline").to_owned()),
        Update::Failed(_) => Some(tr.get("home.check_failed").to_owned()),
    };
    let tint = if matches!(app.update, Update::Failed(_)) { color::DANGER } else { color::TEXT_MUTED };
    let text = match (build, state) {
        (Some(build), Some(state)) => format!("{build}  ·  {state}"),
        (Some(build), None) => build,
        (None, Some(state)) => state,
        (None, None) => tr.get("home.not_installed").to_owned(),
    };
    (text, tint)
}

/// Quiet links under the status: the game output once the game ran, and Try again after a failed check.
fn links(app: &mut App, ui: &mut egui::Ui) {
    let output = app.session.is_some();
    let retry = matches!(app.update, Update::Failed(_));
    if !output && !retry {
        return;
    }
    ui.add_space(space::S);
    let labels: Vec<String> = [(output, "home.output"), (retry, "home.retry")]
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, key)| app.tr.get(key).to_owned())
        .collect();
    let width: f32 = labels
        .iter()
        .map(|l| ui.painter().layout_no_wrap(l.clone(), theme::font::regular(size::SMALL), color::TEXT).size().x)
        .sum::<f32>()
        + space::XL * (labels.len() - 1) as f32;
    let row = ui.allocate_ui_with_layout(
        egui::vec2(width, size::BODY + space::XS),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = space::XL;
            if output && Button::new(Kind::Quiet, app.tr.get("home.output")).small().show(ui).clicked() {
                app.go(Page::Log);
            }
            if retry && Button::new(Kind::Quiet, app.tr.get("home.retry")).small().show(ui).clicked() {
                app.update = Update::Idle;
                app.check_updates(ui.ctx());
            }
        },
    );
    probe::mark(ui.ctx(), "home:links", row.response.rect);
}
