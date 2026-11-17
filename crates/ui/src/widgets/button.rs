//! Pill buttons (Play, Stop, actions) and square icon buttons, with hover and press animation and a focus ring.

use eframe::egui::{self, Color32, Response, Role, Sense, StrokeKind, WidgetInfo};

use super::paint;
use crate::theme::{self, anim, color, font, layout, radius, size, space};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Green: start the game.
    Play,
    /// Red: stop, remove.
    Danger,
    /// Chestnut orange: the main action of a page.
    Accent,
    /// Dark: everything else.
    Ghost,
}

impl Kind {
    fn base(self) -> Color32 {
        match self {
            Self::Play => color::PLAY,
            Self::Danger => color::DANGER,
            Self::Accent => color::ACCENT,
            Self::Ghost => color::SURFACE_HOVER,
        }
    }
}

/// How far the pointer hovers and presses `response`, animated (0 to 1 each).
pub fn hover_press(ui: &egui::Ui, response: &Response) -> (f32, f32) {
    let ctx = ui.ctx();
    let hover = ctx.animate_bool_with_time(response.id.with("hover"), response.hovered(), anim::HOVER);
    let press =
        ctx.animate_bool_with_time(response.id.with("press"), response.is_pointer_button_down_on(), anim::PRESS);
    (hover, press)
}

/// A ring around a widget that has keyboard focus.
pub fn focus_ring(ui: &egui::Ui, response: &Response, rect: egui::Rect, r: u8) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(2.0),
            theme::radius(r.saturating_add(2)),
            egui::Stroke::new(2.0, color::ACCENT),
            StrokeKind::Outside,
        );
    }
}

pub struct Pill<'a> {
    text: &'a str,
    icon: Option<&'static str>,
    kind: Kind,
    enabled: bool,
    width: Option<f32>,
    height: f32,
    big: bool,
}

impl<'a> Pill<'a> {
    pub fn new(kind: Kind, text: &'a str) -> Self {
        Self { text, icon: None, kind, enabled: true, width: None, height: layout::BUTTON_HEIGHT, big: false }
    }

    pub fn icon(mut self, glyph: &'static str) -> Self {
        self.icon = Some(glyph);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// A fixed width, such as the full width of a column.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Taller with larger text, for the hero.
    pub fn big(mut self) -> Self {
        self.big = true;
        self.height = 46.0;
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> Response {
        let text_size = if self.big { size::TITLE } else { size::BUTTON };
        let fg = if self.kind == Kind::Ghost { color::TEXT } else { color::ON_ACCENT };
        let painter = ui.painter();
        let galley = painter.layout_no_wrap(self.text.to_owned(), font::semibold(text_size), fg);
        let icon = self.icon.map(|g| painter.layout_no_wrap(g.to_owned(), font::icon(text_size + 3.0, true), fg));
        let gap = if self.text.is_empty() { 0.0 } else { space::S };
        let icon_width = icon.as_ref().map_or(0.0, |g| g.size().x + gap);
        let pad = if self.big { space::XL } else { space::L };
        let natural = pad * 2.0 + icon_width + galley.size().x;
        let width = self.width.unwrap_or(natural).max(self.height);
        let sense = if self.enabled { Sense::click() } else { Sense::hover() };
        let (rect, response) = ui.allocate_exact_size(egui::vec2(width, self.height), sense);
        let (hover, press) = hover_press(ui, &response);
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let shape = rect.shrink(press * 1.0);
            let base = theme::darken(theme::lighten(self.kind.base(), hover * 0.12), press * 0.15);
            let opacity = if self.enabled { 1.0 } else { 0.4 };
            if self.kind == Kind::Ghost {
                painter.rect_filled(shape, theme::radius(radius::M), base.gamma_multiply(opacity));
                painter.rect_stroke(
                    shape,
                    theme::radius(radius::M),
                    egui::Stroke::new(1.0, theme::lighten(color::BORDER, hover * 0.2)),
                    StrokeKind::Inside,
                );
            } else {
                let top = theme::lighten(base, 0.10).gamma_multiply(opacity);
                let bottom = theme::darken(base, 0.12).gamma_multiply(opacity);
                let d =
                    if self.kind == Kind::Accent { paint::Direction::Horizontal } else { paint::Direction::Vertical };
                paint::gradient_rect(painter, shape, radius::M, top, bottom, d);
            }
            let fg = fg.gamma_multiply(if self.enabled { 1.0 } else { 0.6 });
            let content_width = icon_width + galley.size().x;
            let mut x = shape.center().x - content_width / 2.0;
            if let Some(icon) = icon {
                let y = shape.center().y - icon.size().y / 2.0;
                painter.galley(egui::pos2(x, y), icon.clone(), fg);
                x += icon.size().x + gap;
            }
            painter.galley(egui::pos2(x, shape.center().y - galley.size().y / 2.0), galley, fg);
            focus_ring(ui, &response, rect, radius::M);
        }
        let label = self.text.to_owned();
        let enabled = self.enabled;
        response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, &label));
        if self.enabled { response.on_hover_cursor(egui::CursorIcon::PointingHand) } else { response }
    }
}

/// A square button with one icon; `label` is its tooltip and its name for screen readers.
pub fn icon_button(ui: &mut egui::Ui, glyph: &str, label: &str) -> Response {
    icon_button_with(ui, glyph, label, color::SURFACE_HOVER, color::TEXT, layout::BUTTON_HEIGHT)
}

/// An icon button with its own fill (transparent for the title bar) and size.
pub fn icon_button_with(
    ui: &mut egui::Ui,
    glyph: &str,
    label: &str,
    fill: Color32,
    tint: Color32,
    side: f32,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(side, side), Sense::click());
    let (hover, press) = hover_press(ui, &response);
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let hover_fill = if fill == Color32::TRANSPARENT { color::SURFACE_HOVER } else { theme::lighten(fill, 0.1) };
        let fill = theme::darken(theme::mix(fill, hover_fill, hover), press * 0.2);
        let shape = rect.shrink(press);
        painter.rect_filled(shape, theme::radius(radius::M), fill);
        painter.text(
            shape.center(),
            egui::Align2::CENTER_CENTER,
            glyph,
            font::icon(side * 0.5, false),
            theme::lighten(tint, hover * 0.2),
        );
        focus_ring(ui, &response, rect, radius::M);
    }
    let name = label.to_owned();
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, &name));
    response.on_hover_text(label).on_hover_cursor(egui::CursorIcon::PointingHand)
}
