//! The home page's banner: a sky, a sun, three rows of hills with chestnut trees and a big chestnut, painted in code
//! with the colours of the current profile. A few hundred vertices, built only when a frame is drawn.

use std::f32::consts::TAU;

use eframe::egui::epaint::Mesh;
use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, pos2, vec2};

use crate::theme::{self, color, radius};
use crate::widgets::paint::{self, Direction, Scheme};

/// The height of hill row `row` at `x` (0 to 1 across the banner), as a share of the banner height from the top.
fn ridge(row: usize, x: f32, seed: f32) -> f32 {
    let base = [0.50, 0.63, 0.78][row];
    let amp = [0.11, 0.08, 0.06][row];
    let f = [1.3, 2.1, 1.7][row];
    base - amp * ((x * f + seed) * TAU).sin() - amp * 0.45 * ((x * f * 2.7 + seed * 1.9) * TAU).sin()
}

fn hill(painter: &Painter, rect: Rect, row: usize, seed: f32, top: Color32, bottom: Color32) {
    let steps = 48;
    let mut mesh = Mesh::default();
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = rect.left() + t * rect.width();
        let y = rect.top() + ridge(row, t, seed) * rect.height();
        mesh.colored_vertex(pos2(x, y), top);
        mesh.colored_vertex(pos2(x, rect.bottom()), bottom);
        if i > 0 {
            let n = (i * 2) as u32;
            mesh.add_triangle(n - 2, n - 1, n);
            mesh.add_triangle(n - 1, n + 1, n);
        }
    }
    painter.add(Shape::mesh(mesh));
}

fn tree(painter: &Painter, foot: Pos2, h: f32, crown: Color32) {
    let trunk = theme::darken(crown, 0.4);
    painter.rect_filled(Rect::from_center_size(foot - vec2(0.0, h * 0.2), vec2(h * 0.09, h * 0.42)), 1.0, trunk);
    for (dx, dy, r) in [(0.0, 0.62, 0.30), (-0.20, 0.48, 0.22), (0.21, 0.47, 0.23), (0.0, 0.80, 0.21)] {
        painter.circle_filled(foot + vec2(dx * h, -dy * h), r * h, crown);
    }
}

/// Paints the banner into `rect`; `seed` moves the hills (from the profile id).
pub fn paint(painter: &Painter, rect: Rect, scheme: Scheme, seed: f32) {
    let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    paint::gradient_rect(&painter, rect, 0, scheme.sky_top, scheme.sky_bottom, Direction::Vertical);
    let sun = pos2(rect.left() + rect.width() * 0.68, rect.top() + rect.height() * 0.42);
    let sun_r = rect.height() * 0.13;
    for (k, alpha) in [(3.2, 18u8), (2.2, 30), (1.5, 50)] {
        painter.circle_filled(sun, sun_r * k, Color32::from_rgba_unmultiplied(255, 236, 200, alpha));
    }
    painter.circle_filled(sun, sun_r, scheme.sun);
    for row in 0..3 {
        let top = scheme.hills[row];
        hill(&painter, rect, row, seed + row as f32 * 0.37, top, theme::darken(top, 0.35));
        if row == 1 {
            for (i, t) in [0.08, 0.16, 0.31, 0.47, 0.83, 0.92].into_iter().enumerate() {
                let x = rect.left() + t * rect.width();
                let y = rect.top() + ridge(1, t, seed + 0.37) * rect.height() + 3.0;
                let h = rect.height() * (0.16 + 0.05 * (i % 3) as f32);
                tree(&painter, pos2(x, y), h, theme::darken(scheme.hills[1], 0.25));
            }
        }
    }
    let nut_x = rect.left() + rect.width() * 0.86;
    let nut_y = rect.top() + ridge(2, 0.86, seed + 0.74) * rect.height();
    paint::chestnut(&painter, pos2(nut_x, nut_y - rect.height() * 0.06), rect.height() * 0.30, color::ACCENT_STRONG);
    let shade = Rect::from_min_max(pos2(rect.left(), rect.top() + rect.height() * 0.45), rect.max);
    paint::gradient_rect(&painter, shade, 0, Color32::TRANSPARENT, Color32::from_black_alpha(150), Direction::Vertical);
    paint::mask_corners(&painter, rect, radius::L as f32, color::BG);
}

/// A number from 0 to 1 for a profile id, to vary the hills.
pub fn seed(id: &str) -> f32 {
    (id.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32)) % 1000) as f32 / 1000.0
}
