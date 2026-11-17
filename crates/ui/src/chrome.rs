//! The window's own frame: the title bar (drag, double click, minimise, maximise, close, status chips), resize
//! handles on the edges, and the icon sidebar with the Play button.

use castanea_launcher_core::launch;
use eframe::egui::{self, Color32, CursorIcon, ResizeDirection, Role, Sense, ViewportCommand, WidgetInfo};

use crate::app::{App, Page};
use crate::icons;
use crate::probe;
use crate::shell::megabytes;
use crate::theme::{self, anim, color, font, layout, radius, size, space};
use crate::widgets::{self, button, paint};

const EDGE: f32 = 5.0;

pub fn title_bar(app: &mut App, ui: &mut egui::Ui) {
    let rect = ui.max_rect();
    probe::mark(ui.ctx(), "title_bar", rect);
    let custom = !app.settings.system_frame;
    let drag = ui.interact(rect, ui.scope_id().with("title-drag"), Sense::click_and_drag());
    if custom {
        if drag.drag_started_by(egui::PointerButton::Primary) {
            ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
        }
        if drag.double_clicked() {
            let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
            ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(!maximized));
        }
    }
    ui.horizontal_centered(|ui| {
        ui.add_space(space::L);
        let (mark, _) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), Sense::hover());
        paint::chestnut(ui.painter(), mark.center(), 22.0, color::ACCENT);
        ui.label(egui::RichText::new("castanea").font(font::semibold(size::BODY)).color(color::TEXT));
        ui.label(egui::RichText::new(app.tr.get("window.title")).size(size::BODY).color(color::TEXT_MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if custom {
                window_buttons(app, ui);
                ui.add_space(space::M);
            } else {
                ui.add_space(space::L);
            }
            ui.spacing_mut().item_spacing.x = space::S;
            status_chips(app, ui);
        });
    });
}

fn window_buttons(app: &App, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let side = layout::TITLE_BAR - 8.0;
    let close =
        button::icon_button_with(ui, icons::X, app.tr.get("window.close"), Color32::TRANSPARENT, color::TEXT, side);
    if close.hovered() {
        ui.painter().rect_filled(close.rect, theme::radius(radius::M), color::DANGER.gamma_multiply(0.85));
        ui.painter().text(
            close.rect.center(),
            egui::Align2::CENTER_CENTER,
            icons::X,
            font::icon(side * 0.5, false),
            color::ON_ACCENT,
        );
    }
    if close.clicked() {
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    let (glyph, key) =
        if maximized { (icons::CORNERS_IN, "window.restore") } else { (icons::CORNERS_OUT, "window.maximize") };
    if button::icon_button_with(ui, glyph, app.tr.get(key), Color32::TRANSPARENT, color::TEXT_MUTED, side).clicked() {
        ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    }
    let minimize = app.tr.get("window.minimize");
    if button::icon_button_with(ui, icons::MINUS, minimize, Color32::TRANSPARENT, color::TEXT_MUTED, side).clicked() {
        ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
    }
}

fn status_chips(app: &mut App, ui: &mut egui::Ui) {
    let progress = app.jobs.progress.lock().ok().and_then(|p| p.clone());
    if let Some(progress) = progress {
        if button::icon_button_with(ui, icons::X, app.tr.get("common.cancel"), color::SURFACE, color::TEXT, 26.0)
            .clicked()
        {
            app.jobs.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        let fraction = progress.total.map(|t| progress.done as f32 / t.max(1) as f32);
        let text = format!("{} · {}", progress.label, megabytes(progress.done, progress.total));
        match fraction {
            Some(f) => {
                ui.add(egui::ProgressBar::new(f).desired_width(200.0).desired_height(18.0).corner_radius(9).text(text));
            }
            None => {
                ui.label(widgets::muted(text).size(size::CAPTION));
                ui.add(egui::Spinner::new().size(14.0));
            }
        }
    } else if app.jobs.running > 0 {
        ui.add(egui::Spinner::new().size(14.0)).on_hover_text(app.tr.get("status.working"));
    }
    if app.settings.offline {
        widgets::chip(ui, &format!("{}  {}", icons::WIFI_SLASH, app.tr.get("status.offline")), color::WARN);
    }
    let running: Vec<String> = app.sessions.iter().filter(|s| s.running.is_some()).map(|s| s.name.clone()).collect();
    if !running.is_empty() {
        let text = app.tr.fill("status.running", &[("profile", &running.join(", "))]);
        if widgets::chip(ui, &format!("{}  {text}", icons::PLAY), color::PLAY).interact(Sense::click()).clicked() {
            app.go(Page::Log);
        }
    }
}

/// Starts a window resize when the pointer presses near an edge of an undecorated window.
pub fn resize_edges(app: &App, ctx: &egui::Context) {
    if app.settings.system_frame || ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
        return;
    }
    let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else { return };
    let rect = ctx.content_rect();
    let (left, right) = (pos.x - rect.left() < EDGE, rect.right() - pos.x < EDGE);
    let (top, bottom) = (pos.y - rect.top() < EDGE, rect.bottom() - pos.y < EDGE);
    let direction = match (left, right, top, bottom) {
        (true, _, true, _) => ResizeDirection::NorthWest,
        (_, true, true, _) => ResizeDirection::NorthEast,
        (true, _, _, true) => ResizeDirection::SouthWest,
        (_, true, _, true) => ResizeDirection::SouthEast,
        (true, ..) => ResizeDirection::West,
        (_, true, ..) => ResizeDirection::East,
        (_, _, true, _) => ResizeDirection::North,
        (_, _, _, true) => ResizeDirection::South,
        _ => return,
    };
    let cursor = match direction {
        ResizeDirection::NorthWest | ResizeDirection::SouthEast => CursorIcon::ResizeNwSe,
        ResizeDirection::NorthEast | ResizeDirection::SouthWest => CursorIcon::ResizeNeSw,
        ResizeDirection::West | ResizeDirection::East => CursorIcon::ResizeHorizontal,
        _ => CursorIcon::ResizeVertical,
    };
    ctx.set_cursor_icon(cursor);
    if ctx.input(|i| i.pointer.primary_pressed()) {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction));
    }
}

pub fn sidebar(app: &mut App, ui: &mut egui::Ui) {
    probe::mark(ui.ctx(), "sidebar", ui.max_rect());
    ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing.y = space::S;
        ui.add_space(space::M);
        for page in &Page::ALL[..Page::TOP] {
            sidebar_item(app, ui, *page);
        }
    });
    let bottom =
        ui.max_rect().with_min_y(ui.max_rect().bottom() - 4.0 * (layout::SIDEBAR_ITEM + space::S) - space::L * 2.0);
    ui.scope_builder(egui::UiBuilder::new().max_rect(bottom), |ui| {
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing.y = space::S;
            for page in &Page::ALL[Page::TOP..] {
                sidebar_item(app, ui, *page);
            }
            ui.add_space(space::XS);
            play_button(app, ui);
        });
    });
}

fn sidebar_item(app: &mut App, ui: &mut egui::Ui, page: Page) {
    let active = app.page == page;
    let side = layout::SIDEBAR_ITEM;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(side, side), Sense::click());
    let (hover, press) = button::hover_press(ui, &response);
    let t = ui.ctx().animate_bool_with_time(response.id.with("active"), active, anim::HOVER);
    let painter = ui.painter();
    let fill = theme::mix(theme::mix(Color32::TRANSPARENT, color::SURFACE_HOVER, hover), color::SURFACE_ACTIVE, t);
    painter.rect_filled(rect.shrink(press), theme::radius(radius::M), fill);
    if t > 0.0 {
        let bar = egui::Rect::from_center_size(
            egui::pos2(rect.left() - (layout::SIDEBAR - side) / 2.0 + 2.0, rect.center().y),
            egui::vec2(4.0, 22.0 * t),
        );
        painter.rect_filled(bar, theme::radius(2), color::ACCENT);
    }
    let tint = theme::mix(theme::mix(color::TEXT_MUTED, color::TEXT, hover), color::ACCENT, t);
    painter.text(rect.center(), egui::Align2::CENTER_CENTER, page.icon(), font::icon(size::ICON + 2.0, active), tint);
    if page == Page::Notifications {
        let unread = app.unread();
        if unread > 0 {
            let at = rect.right_top() + egui::vec2(-10.0, 10.0);
            painter.circle_filled(at, 8.0, color::DANGER);
            painter.text(
                at,
                egui::Align2::CENTER_CENTER,
                unread.min(9).to_string(),
                font::semibold(10.0),
                color::ON_ACCENT,
            );
        }
    }
    if page == Page::Log && app.any_running() {
        painter.circle_filled(rect.right_top() + egui::vec2(-10.0, 10.0), 4.5, color::PLAY);
    }
    button::focus_ring(ui, &response, rect, radius::M);
    let n = Page::ALL.iter().position(|p| *p == page).unwrap_or(0) + 1;
    let label = app.tr.get(page.key()).to_owned();
    response.widget_info(|| WidgetInfo::selected(Role::Tab, true, active, &label));
    let response = response.on_hover_text(format!("{label}  (Ctrl+{n})")).on_hover_cursor(CursorIcon::PointingHand);
    probe::mark(ui.ctx(), format!("nav:{}", page.key()), rect);
    if response.clicked() {
        app.go(page);
    }
}

fn play_button(app: &mut App, ui: &mut egui::Ui) {
    let id = app.profile_id.clone();
    let running = app.is_running(&id);
    let ready = launch::pick_build(&app.registry, app.profile()).is_ok();
    let side = layout::SIDEBAR_ITEM;
    let sense = if ready || running { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(side, side), sense);
    let (hover, press) = button::hover_press(ui, &response);
    let base = if running { color::DANGER } else { color::PLAY };
    let base = theme::darken(theme::lighten(base, hover * 0.12), press * 0.15);
    let base = if ready || running { base } else { base.gamma_multiply(0.4) };
    let shape = rect.shrink(press);
    paint::gradient_rect(
        ui.painter(),
        shape,
        radius::M,
        theme::lighten(base, 0.1),
        theme::darken(base, 0.15),
        paint::Direction::Vertical,
    );
    let glyph = if running { icons::STOP } else { icons::PLAY };
    ui.painter().text(
        shape.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        font::icon(size::ICON, true),
        color::ON_ACCENT,
    );
    button::focus_ring(ui, &response, rect, radius::M);
    let name = app.profile().name.clone();
    let label = if running {
        app.tr.fill("nav.stop", &[("profile", &name)])
    } else if ready {
        app.tr.fill("nav.play", &[("profile", &name)])
    } else {
        app.tr.get("play.no_build").to_owned()
    };
    let enabled = ready || running;
    response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, &label));
    probe::mark(ui.ctx(), "nav:play", rect);
    let response = response.on_hover_text(format!("{label}  (F5)"));
    if response.clicked() {
        if running { app.stop(&id) } else { app.play() }
    }
}
