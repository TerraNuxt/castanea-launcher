//! Game output: how the last start stands or ended (with crash reports), and its output.

use castanea_launcher_core::launch;
use eframe::egui::{self, RichText};

use crate::app::{App, open_or_note};
use crate::game::clock;
use crate::theme::{color, font, layout, radius, size, space};
use crate::widgets::{self, Button, Kind};
use crate::{probe, theme};

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    let title = app.tr.get("nav.output").to_owned();
    let has_session = app.session.is_some();
    widgets::page_header(ui, &title, |ui| {
        if !has_session {
            return;
        }
        if Button::new(Kind::Secondary, app.tr.get("log.open_folder")).show(ui).clicked() {
            let dir = app.paths.game().join(launch::run::LOG_DIR);
            open_or_note(app, &dir);
        }
        if Button::new(Kind::Secondary, app.tr.get("log.copy")).show(ui).clicked()
            && let Some(session) = &app.session
        {
            ui.ctx().copy_text(session.log.join("\n"));
        }
    });
    if !has_session {
        widgets::empty(ui, app.tr.get("log.none"));
        return;
    }
    outcome(app, ui);
    ui.add_space(space::M);
    let Some(session) = &app.session else { return };
    let rect = ui.available_rect_before_wrap();
    probe::mark(ui.ctx(), "log:view", rect);
    egui::Frame::new()
        .fill(color::BASE)
        .stroke(theme::hairline())
        .corner_radius(theme::radius(radius::M))
        .inner_margin(egui::Margin::same(space::M as i8))
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show_rows(
                ui,
                layout::LOG_ROW,
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
                        ui.label(RichText::new(line).font(font::mono(size::SMALL)).color(tint));
                    }
                },
            );
        });
}

fn outcome(app: &mut App, ui: &mut egui::Ui) {
    let Some(session) = &app.session else { return };
    let (text, tint) = if let Some(running) = &session.running {
        let time = clock(session.started.elapsed().as_secs());
        (app.tr.fill("log.running", &[("pid", &running.pid().to_string()), ("time", &time)]), color::TEXT)
    } else if let Some(outcome) = &session.outcome {
        let code = outcome.code.map_or("-".to_owned(), |c| c.to_string());
        let time = clock(outcome.seconds);
        if outcome.crashed {
            (app.tr.fill("log.crashed", &[("code", &code), ("time", &time)]), color::DANGER)
        } else {
            (app.tr.fill("log.exited", &[("code", &code), ("time", &time)]), color::TEXT)
        }
    } else {
        return;
    };
    let reports = session.outcome.as_ref().map(|o| o.crash_reports.clone()).unwrap_or_default();
    let command = session.command.clone();
    let running = session.running.is_some();
    let mut show = None;
    let top = ui.cursor().top();
    ui.horizontal(|ui| {
        ui.label(RichText::new(text).size(size::BODY).color(tint));
        if running {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if Button::new(Kind::Secondary, app.tr.get("play.stop")).show(ui).clicked() {
                    app.stop();
                }
            });
        }
    });
    widgets::one_line(ui, RichText::new(command).font(font::mono(size::SMALL)).color(color::TEXT_MUTED));
    for report in &reports {
        ui.horizontal(|ui| {
            widgets::one_line(ui, RichText::new(report.display().to_string()).font(font::mono(size::SMALL)));
            if Button::new(Kind::Quiet, app.tr.get("log.show_report")).small().show(ui).clicked() {
                show = Some(report.clone());
            }
        });
    }
    let rect = egui::Rect::from_x_y_ranges(ui.max_rect().x_range(), top..=ui.cursor().top());
    probe::mark(ui.ctx(), "log:status", rect);
    if let Some(report) = show
        && let Ok(text) = std::fs::read_to_string(&report)
        && let Some(session) = &mut app.session
    {
        session.log = text.lines().map(str::to_owned).collect();
    }
}
