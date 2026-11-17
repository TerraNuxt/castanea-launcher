//! The window's layout: title bar, sidebar, the page in the middle (fading in on change), keyboard shortcuts, the
//! confirm dialog and the toasts.

use castanea_launcher_core::mods::{checks, installed};
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers, RichText};

use crate::app::{App, Confirm, ModsView, NoteKind, Page};
use crate::theme::{self, anim, color, font, layout, radius, size, space};
use crate::widgets::{self, Kind, Pill};
use crate::{chrome, icons, pages, probe};

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    probe::clear(&ctx);
    app.collect_jobs();
    app.poll_sessions();
    if app.any_running() || app.jobs.running > 0 {
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
    shortcuts(app, &ctx);
    chrome::resize_edges(app, &ctx);
    egui::Panel::top("title")
        .exact_size(layout::TITLE_BAR)
        .frame(egui::Frame::new().fill(color::SIDEBAR))
        .show(ui, |ui| chrome::title_bar(app, ui));
    egui::Panel::left("sidebar")
        .exact_size(layout::SIDEBAR)
        .resizable(false)
        .frame(egui::Frame::new().fill(color::SIDEBAR))
        .show(ui, |ui| chrome::sidebar(app, ui));
    let margin = egui::Margin { left: 28, right: 28, top: 22, bottom: 20 };
    egui::CentralPanel::default().frame(egui::Frame::new().fill(color::BG).inner_margin(margin)).show(ui, |ui| {
        probe::mark(ui.ctx(), "content", ui.max_rect());
        let now = ui.input(|i| i.time);
        let start = *app.page_changed.get_or_insert(now);
        let t = ((now - start) as f32 / anim::PAGE).clamp(0.0, 1.0);
        if t < 1.0 {
            ui.ctx().request_repaint();
        }
        ui.multiply_opacity(egui::emath::easing::cubic_out(t));
        match app.page {
            Page::Home => pages::home::ui(app, ui),
            Page::Mods => pages::mods::ui(app, ui),
            Page::Profiles => pages::profiles::ui(app, ui),
            Page::Versions => pages::versions::ui(app, ui),
            Page::Log => pages::log::ui(app, ui),
            Page::Notifications => pages::notifications::ui(app, ui),
            Page::Settings => pages::settings::ui(app, ui),
            Page::About => pages::about::ui(app, ui),
        }
    });
    confirm_dialog(app, &ctx);
    toasts(app, &ctx);
}

fn shortcuts(app: &mut App, ctx: &egui::Context) {
    let keys = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8];
    for (key, page) in keys.into_iter().zip(Page::ALL) {
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, key))) {
            app.go(page);
        }
    }
    if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::NONE, Key::F5))) {
        app.play();
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

/// `1.5 MB`, `12 KB`; with a total, `done / total`.
pub fn megabytes(done: u64, total: Option<u64>) -> String {
    let size = |b: u64| {
        if b < 1 << 20 { format!("{} KB", b.div_ceil(1024)) } else { format!("{:.1} MB", b as f64 / 1_048_576.0) }
    };
    match total {
        Some(total) => format!("{} / {}", size(done), size(total)),
        None => size(done),
    }
}

fn confirm_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(confirm) = &app.confirm else { return };
    let (title, body, yes, danger) = match confirm {
        Confirm::EnableMod { file_name } => {
            let body = app
                .mods
                .iter()
                .find(|m| &m.file_name == file_name)
                .map(|m| pages::mods::declared_text(app, m))
                .unwrap_or_default();
            (app.tr.get("mods.enable_title").to_owned(), body, app.tr.get("mods.enable").to_owned(), false)
        }
        Confirm::RemoveMod { file_name } => (
            app.tr.get("mods.remove_title").to_owned(),
            app.tr.fill("mods.remove_body", &[("name", file_name)]),
            app.tr.get("common.remove").to_owned(),
            true,
        ),
        Confirm::RemoveBuild { id } => (
            app.tr.get("versions.remove_title").to_owned(),
            app.tr.fill("versions.remove_body", &[("version", id)]),
            app.tr.get("common.remove").to_owned(),
            true,
        ),
        Confirm::RemoveProfile { id } => (
            app.tr.get("profile.remove_title").to_owned(),
            app.tr.fill("profile.remove_body", &[("profile", id)]),
            app.tr.get("common.remove").to_owned(),
            true,
        ),
    };
    let mut answer = None;
    let frame = egui::Frame::new()
        .fill(color::SURFACE)
        .stroke(egui::Stroke::new(1.0, color::BORDER))
        .corner_radius(theme::radius(radius::L))
        .inner_margin(egui::Margin::same(space::XL as i8));
    let modal =
        egui::Modal::new(egui::Id::unique("confirm")).frame(frame).backdrop_color(color::BACKDROP).show(ctx, |ui| {
            ui.set_width(440.0);
            ui.horizontal(|ui| {
                let (glyph, tint) =
                    if danger { (icons::WARNING, color::DANGER) } else { (icons::SHIELD_CHECK, color::ACCENT) };
                ui.label(widgets::icon(glyph, 24.0, tint));
                ui.label(RichText::new(&title).font(font::bold(size::TITLE + 2.0)).color(color::TEXT));
            });
            ui.add_space(space::S);
            ui.label(RichText::new(&body).color(color::TEXT_MUTED));
            ui.add_space(space::L);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let kind = if danger { Kind::Danger } else { Kind::Accent };
                if Pill::new(kind, &yes).show(ui).clicked() {
                    answer = Some(true);
                }
                if Pill::new(Kind::Ghost, app.tr.get("common.cancel")).show(ui).clicked() {
                    answer = Some(false);
                }
            });
            if ui.input(|i| i.key_pressed(Key::Enter)) && ui.memory(|m| m.focused().is_none()) {
                answer = Some(true);
            }
        });
    if modal.should_close() && answer.is_none() {
        answer = Some(false);
    }
    match answer {
        Some(true) => {
            if let Some(confirm) = app.confirm.take() {
                apply(app, confirm);
            }
        }
        Some(false) => app.confirm = None,
        None => {}
    }
}

fn apply(app: &mut App, confirm: Confirm) {
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
        Confirm::RemoveBuild { id } => {
            let paths = app.paths.clone();
            let result = app.registry.remove(&paths, &id).map(|_| ());
            app.save_registry();
            result
        }
        Confirm::RemoveProfile { id } => {
            let result = app.profiles.remove(&id).map(|_| ());
            app.save_profiles();
            let first = app.profiles.profiles[0].id.clone();
            app.set_profile(first);
            result
        }
    };
    if let Err(error) = result {
        app.error(error.to_string());
    }
    app.reload_mods();
}

/// New notes for a few seconds at the bottom right; asks for one repaint when the next one expires.
fn toasts(app: &mut App, ctx: &egui::Context) {
    let mut shown = Vec::new();
    let mut next_change: Option<f64> = None;
    for (n, note) in app.notes.iter().enumerate().rev() {
        let age = note.created.elapsed().as_secs_f64();
        if note.seen || age >= anim::TOAST || shown.len() >= 3 {
            continue;
        }
        let fade_in = (age / anim::TOAST_FADE).min(1.0) as f32;
        let fade_out = ((anim::TOAST - age) / anim::TOAST_FADE).min(1.0) as f32;
        let wait = if age < anim::TOAST_FADE { 0.0 } else { (anim::TOAST - anim::TOAST_FADE - age).max(0.0) };
        next_change = Some(next_change.map_or(wait, |w: f64| w.min(wait)));
        shown.push((n, fade_in.min(fade_out), fade_in));
    }
    if let Some(wait) = next_change {
        if wait <= 0.0 {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(wait));
        }
    }
    let screen = ctx.content_rect();
    let mut bottom = screen.bottom() - space::XL;
    let mut dismiss = None;
    for (n, opacity, slide) in shown {
        let note = &app.notes[n];
        let (glyph, tint) = match note.kind {
            NoteKind::Info => (icons::CHECK_CIRCLE, color::PLAY),
            NoteKind::Error => (icons::WARNING, color::DANGER),
        };
        let width = 360.0;
        let x = screen.right() - width - space::XL + (1.0 - slide) * 40.0;
        let area = egui::Area::new(egui::Id::unique(("toast", n)))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::LEFT_BOTTOM)
            .fixed_pos(egui::pos2(x, bottom))
            .show(ctx, |ui| {
                ui.multiply_opacity(opacity);
                egui::Frame::new()
                    .fill(color::SURFACE_HOVER)
                    .stroke(egui::Stroke::new(1.0, tint.gamma_multiply(0.6)))
                    .corner_radius(theme::radius(radius::L))
                    .inner_margin(egui::Margin::same(space::M as i8))
                    .shadow(egui::Shadow {
                        offset: [0, 6],
                        blur: 18,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(120),
                    })
                    .show(ui, |ui| {
                        ui.set_width(width - space::M * 2.0);
                        ui.horizontal(|ui| {
                            ui.label(widgets::icon(glyph, 20.0, tint));
                            ui.add(egui::Label::new(RichText::new(&note.text).color(color::TEXT)).wrap());
                        });
                    })
            });
        if area.response.clicked() {
            dismiss = Some(n);
        }
        bottom = area.response.rect.top() - space::S;
    }
    if let Some(n) = dismiss {
        app.notes[n].seen = true;
    }
}
