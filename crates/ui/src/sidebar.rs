//! The sidebar: Play, Mods, Output and Settings at the top, About at the bottom, and one quiet line for a download or
//! offline mode above it.

use eframe::egui::{self, Color32, CursorIcon, Role, Sense, WidgetInfo};

use crate::app::{App, Page};
use crate::game::Update;
use crate::jobs;
use crate::probe;
use crate::theme::{self, color, font, layout, radius, size, space};
use crate::widgets::{self, button};

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    let full = ui.max_rect();
    probe::mark(ui.ctx(), "sidebar", full);
    ui.painter().vline(full.right(), full.y_range(), theme::hairline());
    let inner = full.shrink2(egui::vec2(space::M, layout::PAGE_MARGIN_TOP));
    ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
        ui.spacing_mut().item_spacing.y = space::XS;
        for page in &Page::ALL[..Page::TOP] {
            item(app, ui, *page);
        }
    });
    let footer = status(app);
    let rows = if footer.is_some() { 2.0 } else { 1.0 };
    let bottom = inner.with_min_y(inner.bottom() - rows * (layout::NAV_ITEM + space::XS));
    ui.scope_builder(egui::UiBuilder::new().max_rect(bottom), |ui| {
        ui.spacing_mut().item_spacing.y = space::XS;
        if let Some(text) = footer {
            ui.allocate_ui(egui::vec2(inner.width(), layout::NAV_ITEM), |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(space::S);
                    widgets::one_line(ui, widgets::small(text));
                });
            });
        }
        item(app, ui, Page::About);
    });
}

/// A download in progress or offline mode, when the home page does not already say so.
fn status(app: &App) -> Option<String> {
    if let Some(progress) = jobs::get(&app.jobs.progress) {
        let percent = progress.fraction().map_or(String::new(), |f| format!(" {:.0} %", f * 100.0));
        return Some(format!("{}{percent}", progress.label));
    }
    if app.page == Page::Home {
        return None;
    }
    if let Update::Downloading { version } = &app.update {
        let fraction = jobs::get(&app.update_progress).and_then(|p| p.fraction()).unwrap_or(0.0);
        let percent = format!("{:.0}", fraction * 100.0);
        return Some(app.tr.fill("status.downloading", &[("version", version), ("percent", &percent)]));
    }
    app.settings.offline.then(|| app.tr.get("status.offline").to_owned())
}

fn item(app: &mut App, ui: &mut egui::Ui, page: Page) {
    let active = app.page == page;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), layout::NAV_ITEM), Sense::click());
    let (hover, press) = button::hover_press(ui, &response);
    let painter = ui.painter();
    let fill = if active {
        color::RAISED
    } else {
        theme::mix(theme::mix(Color32::TRANSPARENT, color::HOVER, hover), color::PRESSED, press)
    };
    painter.rect_filled(rect, theme::radius(radius::M), fill);
    let tint = if active { color::TEXT } else { theme::mix(color::TEXT_MUTED, color::TEXT, hover) };
    let icon_at = egui::pos2(rect.left() + space::S + size::ICON / 2.0, rect.center().y);
    if page.icon() == crate::icons::DRAWN {
        crate::icons::picture(painter, icon_at, tint);
    } else {
        painter.text(icon_at, egui::Align2::CENTER_CENTER, page.icon(), font::icon(size::ICON), tint);
    }
    let label = app.tr.get(page.key()).to_owned();
    let text_at = egui::pos2(rect.left() + space::S * 2.0 + size::ICON, rect.center().y);
    let text_font = if active { font::semibold(size::BODY) } else { font::regular(size::BODY) };
    painter.text(text_at, egui::Align2::LEFT_CENTER, &label, text_font, tint);
    if page == Page::Log && app.is_running() {
        let dot = egui::pos2(rect.right() - space::M, rect.center().y);
        painter.circle_filled(dot, layout::DOT, color::ACCENT);
    }
    button::focus_ring(ui, &response, rect, radius::M);
    let n = Page::ALL.iter().position(|p| *p == page).unwrap_or(0) + 1;
    response.widget_info(|| WidgetInfo::selected(Role::Tab, true, active, &label));
    let response = response.on_hover_text(format!("Ctrl+{n}")).on_hover_cursor(CursorIcon::PointingHand);
    probe::mark(ui.ctx(), format!("nav:{}", page.key()), rect);
    if response.clicked() {
        app.go(page);
    }
}
