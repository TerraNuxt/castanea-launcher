//! The rounded dark card: an optional accent glow on top, outline (running game, selection), hover for clickable cards.

use eframe::egui::epaint::{CornerRadiusF32, RoundedRect, tessellator::path};
use eframe::egui::{self, Color32, CornerRadius, Response, Sense, StrokeKind};

use super::paint;
use crate::theme::{self, color, font, radius, size, space};

const GLOW: f32 = 90.0;

pub struct Card {
    accent: Option<Color32>,
    outline: Option<Color32>,
    clickable: bool,
    padding: f32,
    fill: Color32,
}

impl Default for Card {
    fn default() -> Self {
        Self::new()
    }
}

impl Card {
    pub fn new() -> Self {
        Self { accent: None, outline: None, clickable: false, padding: space::L, fill: color::SURFACE }
    }

    /// A glow of `accent` from the top, for the cards of the right column.
    pub fn accent(mut self, accent: Color32) -> Self {
        self.accent = Some(accent);
        self
    }

    pub fn outline(mut self, outline: Color32) -> Self {
        self.outline = Some(outline);
        self
    }

    pub fn outline_if(self, on: bool, outline: Color32) -> Self {
        if on { self.outline(outline) } else { self }
    }

    /// The whole card reacts to the pointer; use the returned response's `clicked`.
    pub fn clickable(mut self) -> Self {
        self.clickable = true;
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    pub fn fill(mut self, fill: Color32) -> Self {
        self.fill = fill;
        self
    }

    /// Draws the card at the full available width with `add` inside.
    pub fn show<R>(self, ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
        let background = ui.painter().add(egui::Shape::Noop);
        let margin = egui::Margin {
            left: self.padding as i8,
            right: self.padding as i8,
            top: self.padding as i8,
            bottom: self.padding as i8,
        };
        let inner = egui::Frame::new().inner_margin(margin).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            add(ui)
        });
        let rect = inner.response.rect;
        let response = if self.clickable {
            ui.interact(rect, inner.response.id.with("card"), Sense::click())
        } else {
            inner.response.clone()
        };
        let hover = if self.clickable { super::button::hover_press(ui, &response).0 } else { 0.0 };
        let fill = theme::mix(self.fill, color::SURFACE_HOVER, hover);
        let mut shapes = vec![egui::Shape::rect_filled(rect, theme::radius(radius::L), fill)];
        if let Some(accent) = self.accent {
            // A glow of the accent that fades out downwards; the outline below covers its edge.
            let top = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), rect.height().min(GLOW)));
            let r = CornerRadius { nw: radius::L, ne: radius::L, sw: 0, se: 0 };
            let mut points = Vec::new();
            path::rounded_rectangle(&mut points, RoundedRect::new(top, CornerRadiusF32::from(r)));
            let (from, to) = (theme::mix(fill, accent, 0.24), fill);
            shapes.push(paint::fan(top.center(), &points, |p| theme::mix(from, to, (p.y - top.top()) / top.height())));
        }
        let stroke = match (self.outline, self.accent) {
            (Some(outline), _) => egui::Stroke::new(1.5, outline),
            (None, Some(accent)) => egui::Stroke::new(1.0, theme::mix(color::BORDER, accent, 0.3)),
            (None, None) => egui::Stroke::new(1.0, theme::mix(color::BORDER, color::SURFACE_ACTIVE, hover)),
        };
        shapes.push(egui::Shape::Rect(egui::epaint::RectShape::stroke(
            rect,
            theme::radius(radius::L),
            stroke,
            StrokeKind::Inside,
        )));
        ui.painter().set(background, egui::Shape::Vec(shapes));
        if self.clickable {
            super::button::focus_ring(ui, &response, rect, radius::L);
        }
        egui::InnerResponse::new(inner.inner, response)
    }
}

/// The title row of a right column card: an icon in a tinted circle, the title, and a muted note on the right.
pub fn card_title(ui: &mut egui::Ui, glyph: &str, title: &str, accent: Color32, note: Option<&str>) -> Response {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), Sense::hover());
        ui.painter().circle_filled(rect.center(), 14.0, accent.gamma_multiply(0.22));
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, glyph, font::icon(16.0, true), accent);
        ui.label(egui::RichText::new(title).font(font::semibold(size::TITLE)).color(color::TEXT));
        if let Some(note) = note {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(note).size(size::CAPTION).color(color::TEXT_MUTED));
            });
        }
    })
    .response
}
