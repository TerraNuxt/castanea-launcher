//! Line icons of the Phosphor icon font (MIT, third_party/phosphor), only where an icon carries meaning.

pub const ARROW_LEFT: &str = "\u{E058}";
pub const ARROWS_CLOCKWISE: &str = "\u{E094}";
/// Marks a page whose icon the sidebar draws itself (the Phosphor subset has no picture glyph).
pub const DRAWN: &str = "";
pub const GEAR: &str = "\u{E272}";
pub const INFO: &str = "\u{E2CE}";
pub const PLAY: &str = "\u{E3D0}";
pub const PUZZLE: &str = "\u{E596}";
pub const SEARCH: &str = "\u{E30C}";
pub const TERMINAL: &str = "\u{EAE8}";
pub const TRASH: &str = "\u{E4A6}";
pub const WARNING: &str = "\u{E4E0}";
pub const X: &str = "\u{E4F6}";

/// The picture icon of the Screenshots page, drawn: a frame, a sun and two hills. The Phosphor subset has no such glyph.
pub fn picture(painter: &eframe::egui::Painter, center: eframe::egui::Pos2, tint: eframe::egui::Color32) {
    use eframe::egui::{Rect, Stroke, StrokeKind, Vec2, pos2, vec2};
    let stroke = Stroke::new(1.5, tint);
    let frame = Rect::from_center_size(center, Vec2::new(15.0, 12.0));
    painter.rect_stroke(frame, 2.0, stroke, StrokeKind::Inside);
    painter.circle_stroke(frame.left_top() + vec2(4.5, 4.0), 1.4, Stroke::new(1.2, tint));
    let base = frame.bottom() - 1.0;
    painter.line(
        vec![
            pos2(frame.left() + 1.0, base),
            pos2(frame.left() + 5.5, base - 3.5),
            pos2(frame.left() + 8.5, base - 1.0),
        ],
        stroke,
    );
    painter.line(
        vec![
            pos2(frame.left() + 7.0, base - 0.5),
            pos2(frame.left() + 10.5, base - 4.0),
            pos2(frame.right() - 1.0, base - 1.5),
        ],
        stroke,
    );
}
