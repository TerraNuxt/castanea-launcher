//! Game output: one chip per session (running or ended), the outcome with crash reports, and the log of the chosen one.

use castanea_launcher_core::launch;
use eframe::egui::{self, RichText};

use crate::app::{App, open_or_note};
use crate::pages::mods::empty;
use crate::theme::{self, color, radius, size, space};
use crate::widgets::{self, Card, Kind, Pill};
use crate::{icons, probe};

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    let title = app.tr.get("play.log").to_owned();
    let chosen = app
        .log_profile
        .clone()
        .filter(|p| app.session(p).is_some())
        .or_else(|| app.sessions.last().map(|s| s.profile.clone()));
    widgets::page_header(ui, &title, |ui| {
        let Some(chosen) = &chosen else { return };
        let dir = app.profiles.get(chosen).map(|p| p.game_dir(&app.paths).join(launch::run::LOG_DIR));
        if let Some(dir) = dir
            && Pill::new(Kind::Ghost, app.tr.get("play.open_log")).icon(icons::FOLDER_OPEN).show(ui).clicked()
        {
            open_or_note(app, &dir);
        }
        if Pill::new(Kind::Ghost, app.tr.get("play.copy_log")).icon(icons::COPY).show(ui).clicked()
            && let Some(session) = app.session(chosen)
        {
            ui.ctx().copy_text(session.log.join("\n"));
        }
    });
    ui.add_space(space::M);
    let Some(chosen) = chosen else {
        empty(ui, icons::TERMINAL, app.tr.get("log.none"));
        return;
    };
    let mut pick = None;
    ui.horizontal_wrapped(|ui| {
        for session in &app.sessions {
            let tint = match (&session.running, &session.outcome) {
                (Some(_), _) => color::PLAY,
                (None, Some(o)) if o.crashed => color::DANGER,
                _ => color::TEXT_MUTED,
            };
            let selected = session.profile == chosen;
            let text =
                format!("{}  {}", if session.running.is_some() { icons::PLAY } else { icons::SQUARE }, session.name);
            let chip =
                widgets::chip(ui, &text, if selected { color::ACCENT } else { tint }).interact(egui::Sense::click());
            let name = session.name.clone();
            chip.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, selected, &name));
            if chip.clicked() {
                pick = Some(session.profile.clone());
            }
        }
    });
    if let Some(profile) = pick {
        app.log_profile = Some(profile);
    }
    ui.add_space(space::S);
    outcome(app, ui, &chosen);
    ui.add_space(space::S);
    let Some(session) = app.session(&chosen) else { return };
    let rect = ui.available_rect_before_wrap();
    probe::mark(ui.ctx(), "log:view", rect);
    egui::Frame::new()
        .fill(color::SUNKEN)
        .stroke(egui::Stroke::new(1.0, color::BORDER))
        .corner_radius(theme::radius(radius::L))
        .inner_margin(egui::Margin::same(space::M as i8))
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show_rows(
                ui,
                17.0,
                session.log.len(),
                |ui, range| {
                    for line in &session.log[range] {
                        let lower = line.to_lowercase();
                        let tint = if lower.contains("panic") || lower.contains("error") {
                            color::DANGER
                        } else if lower.contains("warn") {
                            color::WARN
                        } else {
                            color::TEXT_MUTED
                        };
                        ui.label(RichText::new(line).monospace().color(tint));
                    }
                },
            );
        });
}

fn outcome(app: &mut App, ui: &mut egui::Ui, profile: &str) {
    let Some(session) = app.session(profile) else { return };
    let mut show = None;
    let (glyph, tint, text) = if let Some(running) = &session.running {
        (icons::PLAY, color::PLAY, app.tr.fill("play.running", &[("pid", &running.pid().to_string())]))
    } else if let Some(outcome) = &session.outcome {
        let code = outcome.code.map_or("-".to_owned(), |c| c.to_string());
        let seconds = outcome.seconds.to_string();
        if outcome.crashed {
            (icons::BUG, color::DANGER, app.tr.fill("play.crashed", &[("code", &code), ("seconds", &seconds)]))
        } else {
            (icons::CHECK_CIRCLE, color::PLAY, app.tr.fill("play.exited", &[("code", &code), ("seconds", &seconds)]))
        }
    } else {
        return;
    };
    let reports = session.outcome.as_ref().map(|o| o.crash_reports.clone()).unwrap_or_default();
    let command = session.command.clone();
    let running = session.running.is_some();
    Card::new().padding(space::M).outline(tint.gamma_multiply(0.6)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(widgets::icon(glyph, 20.0, tint));
            ui.label(RichText::new(text).color(tint).size(size::TITLE));
            if running {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if Pill::new(Kind::Danger, app.tr.get("play.stop")).icon(icons::STOP).show(ui).clicked() {
                        app.stop(profile);
                    }
                });
            }
        });
        widgets::one_line(ui, RichText::new(command).monospace().size(size::CAPTION).color(color::TEXT_FAINT));
        for report in &reports {
            ui.horizontal(|ui| {
                widgets::one_line(ui, RichText::new(report.display().to_string()).monospace().size(size::SMALL));
                if Pill::new(Kind::Ghost, app.tr.get("play.show_report")).show(ui).clicked() {
                    show = Some(report.clone());
                }
            });
        }
    });
    if let Some(report) = show
        && let Ok(text) = std::fs::read_to_string(&report)
        && let Some(session) = app.sessions.iter_mut().find(|s| s.profile == profile)
    {
        session.log = text.lines().map(str::to_owned).collect();
    }
}
