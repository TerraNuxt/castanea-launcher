//! The window's layout: sidebar, the page, keyboard shortcuts, the confirm dialog and the toasts.

use castanea_launcher_core::mods::{checks, installed};
use castanea_launcher_core::storage::Row;
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};

use crate::app::{App, Confirm, ModsView, NoteKind, Page};
use crate::storage::Deletion;
use crate::theme::{anim, color, layout, radius, size, space};
use crate::widgets::{self, Button, Kind};
use crate::{pages, probe, sidebar, theme};

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    probe::clear(&ctx);
    if app.auto_check && !app.checked {
        app.check_updates(&ctx);
    }
    app.collect_jobs(&ctx);
    app.auto_news(&ctx);
    app.auto_gallery(&ctx);
    app.auto_last_played(&ctx);
    app.refresh_card_picture();
    app.collect_thumbs(&ctx);
    app.auto_check_launcher(&ctx);
    app.poll_session(&ctx);
    if app.is_running() || app.jobs.running > 0 {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(anim::BUSY_REPAINT));
    }
    shortcuts(app, &ctx);
    egui::Panel::left("sidebar")
        .exact_size(layout::SIDEBAR)
        .resizable(false)
        .frame(egui::Frame::new().fill(color::BASE))
        .show(ui, |ui| sidebar::ui(app, ui));
    let margin = egui::Margin {
        left: layout::PAGE_MARGIN as i8,
        right: layout::PAGE_MARGIN as i8,
        top: layout::PAGE_MARGIN_TOP as i8,
        bottom: layout::PAGE_MARGIN_TOP as i8,
    };
    egui::CentralPanel::default().frame(egui::Frame::new().fill(color::BG).inner_margin(margin)).show(ui, |ui| {
        probe::mark(ui.ctx(), "content", ui.max_rect());
        match app.page {
            Page::Home => pages::home::ui(app, ui),
            Page::Mods => pages::mods::ui(app, ui),
            Page::Screenshots => pages::screenshots::ui(app, ui),
            Page::Log => pages::log::ui(app, ui),
            Page::Settings => pages::settings::ui(app, ui),
            Page::About => pages::about::ui(app, ui),
        }
    });
    confirm_dialog(app, &ctx);
    toasts(app, &ctx);
}

fn shortcuts(app: &mut App, ctx: &egui::Context) {
    let keys = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6];
    for (key, page) in keys.into_iter().zip(Page::ALL) {
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, key))) {
            app.go(page);
        }
    }
    if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::NONE, Key::F5))) {
        app.play(ctx);
    }
    if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::F))) {
        app.go(Page::Mods);
        app.mods_view = ModsView::Browse;
        app.focus_search = true;
    }
    if app.confirm.is_none()
        && app.page == Page::Mods
        && app.browse.selected.is_some()
        && ctx.input(|i| i.key_pressed(Key::Escape))
    {
        app.browse.selected = None;
    }
}

/// `1.5 MB`, `12 KB`.
pub fn megabytes(bytes: u64) -> String {
    if bytes < 1 << 20 {
        format!("{} KB", bytes.div_ceil(1024))
    } else {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    }
}

fn confirm_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(confirm) = &app.confirm else { return };
    let storage = pages::storage::dialog(app, confirm);
    let (title, body, yes, danger) = match (&storage, confirm) {
        (Some(d), _) => (d.title.clone(), d.intro.clone(), d.yes.clone(), !d.notice),
        (None, Confirm::EnableMod { file_name }) => {
            let body = app
                .mods
                .iter()
                .find(|m| &m.file_name == file_name)
                .map(|m| pages::mods::declared_text(app, m))
                .unwrap_or_default();
            (app.tr.get("mods.enable_title").to_owned(), body, app.tr.get("mods.enable").to_owned(), false)
        }
        (None, Confirm::RemoveMod { file_name }) => (
            app.tr.get("mods.remove_title").to_owned(),
            app.tr.fill("mods.remove_body", &[("name", file_name)]),
            app.tr.get("common.remove").to_owned(),
            true,
        ),
        (None, Confirm::DeleteShot(path)) => (
            app.tr.get("shots.delete_title").to_owned(),
            app.tr.fill("shots.delete_body", &[("name", &crate::gallery::file_name(path))]),
            app.tr.get("shots.to_trash").to_owned(),
            true,
        ),
        (None, Confirm::DeleteShotForever { path, reason }) => (
            app.tr.get("shots.forever_title").to_owned(),
            app.tr.fill("shots.forever_body", &[("name", &crate::gallery::file_name(path)), ("reason", reason)]),
            app.tr.get("shots.delete").to_owned(),
            true,
        ),
        _ => return,
    };
    let notice = storage.as_ref().is_some_and(|d| d.notice);
    // Enter never confirms a deletion.
    let asks_for_delete = matches!(confirm, Confirm::DeleteShot(_) | Confirm::DeleteShotForever { .. });
    let mut ready = true;
    let typed = &mut app.storage.typed;
    let mut answer = None;
    let frame = egui::Frame::new()
        .fill(color::RAISED)
        .stroke(egui::Stroke::new(theme::line::HAIRLINE, color::BORDER_STRONG))
        .corner_radius(theme::radius(radius::M))
        .inner_margin(egui::Margin::same(space::XL as i8));
    let modal =
        egui::Modal::new(egui::Id::unique("confirm")).frame(frame).backdrop_color(color::BACKDROP).show(ctx, |ui| {
            ui.set_width(layout::DIALOG);
            ui.label(widgets::strong(&title, size::TITLE));
            ui.add_space(space::M);
            widgets::wrapped(ui, widgets::muted(&body));
            if let Some(dialog) = &storage {
                ready = pages::storage::dialog_body(typed, &app.tr, ui, dialog);
            }
            ui.add_space(space::XL);
            let row = egui::vec2(ui.available_width(), layout::CONTROL);
            ui.allocate_ui_with_layout(row, egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let kind = if danger { Kind::Danger } else { Kind::Primary };
                if Button::new(kind, &yes).enabled(ready).show(ui).clicked() {
                    answer = Some(true);
                }
                if !notice && Button::new(Kind::Secondary, app.tr.get("common.cancel")).show(ui).clicked() {
                    answer = Some(false);
                }
            });
            if storage.is_none()
                && !asks_for_delete
                && ui.input(|i| i.key_pressed(Key::Enter))
                && ui.memory(|m| m.focused().is_none())
            {
                answer = Some(true);
            }
        });
    if modal.should_close() && answer.is_none() {
        answer = Some(false);
    }
    match answer {
        Some(true) => {
            if let Some(confirm) = app.confirm.take() {
                apply(app, ctx, confirm);
            }
        }
        Some(false) if notice => std::process::exit(0),
        Some(false) => app.confirm = None,
        None => {}
    }
}

fn apply(app: &mut App, ctx: &egui::Context, confirm: Confirm) {
    let confirm = match confirm {
        Confirm::DeleteRow(row) => return app.start_delete(ctx, Deletion::Row(row)),
        Confirm::DeleteGameData => return app.start_delete(ctx, Deletion::Row(Row::GameData)),
        Confirm::DeleteEverything => return app.start_delete(ctx, Deletion::Everything),
        #[cfg(windows)]
        Confirm::Uninstall => return app.start_delete(ctx, Deletion::Uninstall),
        Confirm::DeleteShot(path) => return app.delete_shot(path),
        Confirm::DeleteShotForever { path, .. } => return app.delete_shot_forever(path),
        Confirm::Goodbye => std::process::exit(0),
        other => other,
    };
    let mods_dir = app.mods_dir();
    let result = match confirm {
        Confirm::EnableMod { file_name } => match app.mods.iter().find(|m| m.file_name == file_name) {
            Some(package) => {
                let mut wanted = checks::dependencies_of(&app.mods, package.id());
                wanted.push(package);
                wanted
                    .into_iter()
                    .filter(|m| !m.enabled)
                    .try_for_each(|m| installed::set_enabled(&mods_dir, m, true).map(|_| ()))
            }
            None => Ok(()),
        },
        Confirm::RemoveMod { file_name } => match app.mods.iter().find(|m| m.file_name == file_name) {
            Some(package) => installed::remove(package),
            None => Ok(()),
        },
        _ => Ok(()),
    };
    if let Err(error) = result {
        app.error(error.to_string());
    }
    app.reload_mods();
}

/// Notes for a few seconds at the bottom right, no animation; asks for one repaint when the next one expires.
fn toasts(app: &mut App, ctx: &egui::Context) {
    let life = |kind| if kind == NoteKind::Error { anim::TOAST_ERROR } else { anim::TOAST };
    app.notes.retain(|n| n.created.elapsed().as_secs_f64() < life(n.kind));
    if let Some(wait) = app.notes.iter().map(|n| life(n.kind) - n.created.elapsed().as_secs_f64()).reduce(f64::min) {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(wait.max(0.0)));
    }
    let screen = ctx.content_rect();
    let mut bottom = screen.bottom() - space::XL;
    let mut dismiss = None;
    for (n, note) in app.notes.iter().enumerate().rev().take(3) {
        let tint = if note.kind == NoteKind::Error { color::DANGER } else { color::TEXT };
        let x = screen.right() - layout::TOAST - space::XL;
        let area = egui::Area::new(egui::Id::unique(("toast", n)))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::LEFT_BOTTOM)
            .fixed_pos(egui::pos2(x, bottom))
            .sense(egui::Sense::click())
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(color::RAISED)
                    .stroke(egui::Stroke::new(theme::line::HAIRLINE, color::BORDER_STRONG))
                    .corner_radius(theme::radius(radius::M))
                    .inner_margin(egui::Margin::symmetric(space::L as i8, space::M as i8))
                    .show(ui, |ui| {
                        ui.set_width(layout::TOAST - space::L * 2.0);
                        widgets::wrapped(ui, egui::RichText::new(&note.text).size(size::BODY).color(tint));
                    })
            });
        if area.response.clicked() {
            dismiss = Some(n);
        }
        bottom = area.response.rect.top() - space::S;
    }
    if let Some(n) = dismiss {
        app.notes.remove(n);
    }
}
