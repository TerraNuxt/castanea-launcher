//! Buttons: primary (the accent, used once per view), secondary (hairline outline), danger (only in dialogs), quiet
//! (a text link), and square icon buttons. Hover and press fade in 120 ms; a focus ring for the keyboard.

use eframe::egui::{self, Color32, Response, Role, Sense, StrokeKind, WidgetInfo};

use crate::bmfont::BmText;
use crate::theme::{self, anim, color, font, layout, line, radius, size, space};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Primary,
    Secondary,
    Danger,
    Quiet,
}

/// How far the pointer hovers and presses `response`, faded (0 to 1 each).
pub fn hover_press(ui: &egui::Ui, response: &Response) -> (f32, f32) {
    let ctx = ui.ctx();
    let hover = ctx.animate_bool_with_time(response.id.with("hover"), response.hovered(), anim::HOVER);
    let press =
        ctx.animate_bool_with_time(response.id.with("press"), response.is_pointer_button_down_on(), anim::HOVER);
    (hover, press)
}

/// A ring around a control that has keyboard focus.
pub fn focus_ring(ui: &egui::Ui, response: &Response, rect: egui::Rect, r: u8) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(layout::INSET),
            theme::radius(r.saturating_add(2)),
            egui::Stroke::new(line::FOCUS, color::ACCENT),
            StrokeKind::Outside,
        );
    }
}

pub struct Button<'a> {
    text: &'a str,
    kind: Kind,
    enabled: bool,
    size: [f32; 2],
    width: Option<f32>,
    text_size: f32,
    trailing: Option<String>,
    name: Option<String>,
    brand: bool,
}

impl<'a> Button<'a> {
    pub fn new(kind: Kind, text: &'a str) -> Self {
        Self {
            text,
            kind,
            enabled: true,
            size: [0.0, layout::CONTROL],
            width: None,
            text_size: size::BODY,
            trailing: None,
            name: None,
            brand: false,
        }
    }

    /// Sets the label in the brand font (docs/ui-design.md) when it can spell it, else in Inter.
    pub fn brand(mut self) -> Self {
        self.brand = true;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// The large Play button of the home page.
    pub fn large(mut self) -> Self {
        self.size = layout::PLAY;
        self.width = Some(layout::PLAY[0]);
        self.text_size = size::TITLE;
        self
    }

    /// Small text, for quiet links under a status line.
    pub fn small(mut self) -> Self {
        self.text_size = size::SMALL;
        self
    }

    /// A second part in a fixed slot after the text, such as a running time that changes each second.
    pub fn trailing(mut self, text: String) -> Self {
        self.trailing = Some(text);
        self
    }

    /// The name for screen readers, when it says more than the visible text.
    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> Response {
        let quiet = self.kind == Kind::Quiet;
        let text_font = if quiet { font::regular(self.text_size) } else { font::semibold(self.text_size) };
        let galley = ui.painter().layout_no_wrap(self.text.to_owned(), text_font, Color32::PLACEHOLDER);
        let trailing = self.trailing.as_ref().map(|t| {
            let digits: String = t.chars().map(|c| if c.is_ascii_digit() { '0' } else { c }).collect();
            let slot =
                ui.painter().layout_no_wrap(digits, font::regular(self.text_size), Color32::PLACEHOLDER).size().x;
            (ui.painter().layout_no_wrap(t.clone(), font::regular(self.text_size), Color32::PLACEHOLDER), slot)
        });
        let brand = (self.brand && !quiet).then(|| BmText::new(self.text, size::BRAND_BUTTON).fallback(color::TEXT));
        let brand = brand.filter(BmText::is_bitmap);
        let label_width = brand.as_ref().map_or(galley.size().x, |b| b.measure(ui.ctx()).x);
        let content = label_width + trailing.as_ref().map_or(0.0, |(_, slot)| space::M + slot);
        let pad = if quiet { 0.0 } else { space::L };
        let height = if quiet { galley.size().y + space::XS } else { self.size[1] };
        let width = self.width.unwrap_or(content + pad * 2.0);
        let sense = if self.enabled { Sense::click() } else { Sense::hover() };
        let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), sense);
        let (hover, press) = hover_press(ui, &response);
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let (mut fill, mut stroke, fg) = self.colors(hover, press);
            if brand.is_some() && self.enabled && self.kind == Kind::Primary {
                // Copper letters need a dark plate; the accent is kept as the outline.
                fill = theme::mix(theme::mix(color::BASE, color::RAISED, hover), color::HOVER, press);
                stroke = Some(color::ACCENT);
            }
            if fill != Color32::TRANSPARENT {
                painter.rect_filled(rect, theme::radius(radius::M), fill);
            }
            if let Some(stroke) = stroke {
                painter.rect_stroke(
                    rect,
                    theme::radius(radius::M),
                    egui::Stroke::new(line::HAIRLINE, stroke),
                    StrokeKind::Inside,
                );
            }
            let mut x = rect.center().x - content / 2.0;
            if let Some(brand) = &brand {
                let tint = if self.enabled { Color32::WHITE } else { theme::mix(Color32::WHITE, color::RAISED, 0.7) };
                let height = brand.measure(ui.ctx()).y;
                brand.clone().tint(tint).paint(painter, egui::pos2(x, rect.center().y - height / 2.0));
            } else {
                painter.galley(egui::pos2(x, rect.center().y - galley.size().y / 2.0), galley.clone(), fg);
            }
            if let Some((time, slot)) = trailing {
                x += label_width + space::M;
                let at = egui::pos2(x + slot - time.size().x, rect.center().y - time.size().y / 2.0);
                painter.galley(at, time, theme::mix(fg, Color32::TRANSPARENT, 0.15));
            }
            focus_ring(ui, &response, rect, radius::M);
        }
        let name = self.name.unwrap_or_else(|| match &self.trailing {
            Some(t) => format!("{} {t}", self.text),
            None => self.text.to_owned(),
        });
        let (enabled, role) = (self.enabled, if quiet { Role::Link } else { Role::Button });
        response.widget_info(|| WidgetInfo::labeled(role, enabled, &name));
        if enabled { response.on_hover_cursor(egui::CursorIcon::PointingHand) } else { response }
    }

    fn colors(&self, hover: f32, press: f32) -> (Color32, Option<Color32>, Color32) {
        if !self.enabled {
            let stroke = matches!(self.kind, Kind::Secondary).then_some(color::BORDER);
            let fill = if self.kind == Kind::Quiet { Color32::TRANSPARENT } else { color::RAISED };
            return (fill, stroke, color::TEXT_DISABLED);
        }
        let step = |base: Color32, hovered: Color32, pressed: Color32| {
            theme::mix(theme::mix(base, hovered, hover), pressed, press)
        };
        match self.kind {
            Kind::Primary => (step(color::ACCENT, color::ACCENT_HOVER, color::ACCENT_PRESSED), None, color::ON_ACCENT),
            Kind::Danger => (color::DANGER_FILL, None, color::ON_ACCENT),
            Kind::Secondary => {
                (step(Color32::TRANSPARENT, color::HOVER, color::PRESSED), Some(color::BORDER_STRONG), color::TEXT)
            }
            Kind::Quiet => (Color32::TRANSPARENT, None, theme::mix(color::TEXT_MUTED, color::TEXT, hover)),
        }
    }
}

/// A square button with one line icon; `label` is its tooltip and its name for screen readers.
pub fn icon_button(ui: &mut egui::Ui, glyph: &str, label: &str) -> Response {
    let side = layout::CONTROL;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(side, side), Sense::click());
    let (hover, press) = hover_press(ui, &response);
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let fill = theme::mix(theme::mix(Color32::TRANSPARENT, color::HOVER, hover), color::PRESSED, press);
        painter.rect_filled(rect, theme::radius(radius::M), fill);
        let tint = theme::mix(color::TEXT_MUTED, color::TEXT, hover);
        painter.text(rect.center(), egui::Align2::CENTER_CENTER, glyph, font::icon(size::ICON), tint);
        focus_ring(ui, &response, rect, radius::M);
    }
    let name = label.to_owned();
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, &name));
    response.on_hover_text(label).on_hover_cursor(egui::CursorIcon::PointingHand)
}
