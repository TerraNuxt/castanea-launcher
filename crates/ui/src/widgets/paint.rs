//! Painted pictures made in code: gradients, the chestnut mark, profile tiles and the colour sets they use.

use std::f32::consts::{PI, TAU};

use eframe::egui::epaint::{CornerRadiusF32, Mesh, RoundedRect, tessellator::path};
use eframe::egui::{self, Color32, CornerRadius, Painter, Pos2, Rect, Shape, pos2, vec2};

use crate::theme::{self, color, font};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Vertical,
    Horizontal,
}

/// A rounded rectangle filled with a gradient from `from` (top or left) to `to`.
pub fn gradient_rect(
    painter: &Painter,
    rect: Rect,
    r: impl Into<CornerRadius>,
    from: Color32,
    to: Color32,
    d: Direction,
) {
    let mut shapes = Vec::new();
    gradient_shapes(&mut shapes, rect, r, from, to, d);
    painter.extend(shapes);
}

/// The shapes of [`gradient_rect`]: an anti-aliased plain fill for the edge, a gradient mesh just inside it.
pub fn gradient_shapes(
    out: &mut Vec<Shape>,
    rect: Rect,
    r: impl Into<CornerRadius>,
    from: Color32,
    to: Color32,
    d: Direction,
) {
    let r = r.into();
    out.push(Shape::rect_filled(rect, r, theme::mix(from, to, 0.5)));
    let inner = rect.shrink(0.6);
    let mut points = Vec::new();
    path::rounded_rectangle(&mut points, RoundedRect::new(inner, CornerRadiusF32::from(r)));
    let at = |p: Pos2| match d {
        Direction::Vertical => (p.y - inner.top()) / inner.height().max(1.0),
        Direction::Horizontal => (p.x - inner.left()) / inner.width().max(1.0),
    };
    out.push(fan(inner.center(), &points, |p| theme::mix(from, to, at(p))));
}

/// A triangle fan from `center` over a convex outline, coloured per vertex.
pub fn fan(center: Pos2, outline: &[Pos2], color_at: impl Fn(Pos2) -> Color32) -> Shape {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(center, color_at(center));
    for &p in outline {
        mesh.colored_vertex(p, color_at(p));
    }
    let n = outline.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    Shape::mesh(mesh)
}

/// Covers what lies outside the rounded corners of `rect` with `bg`, to round off a picture painted as a plain rect.
pub fn mask_corners(painter: &Painter, rect: Rect, r: f32, bg: Color32) {
    let corners = [
        (rect.left_top(), vec2(1.0, 1.0), PI),
        (rect.right_top(), vec2(-1.0, 1.0), 1.5 * PI),
        (rect.right_bottom(), vec2(-1.0, -1.0), 0.0),
        (rect.left_bottom(), vec2(1.0, -1.0), 0.5 * PI),
    ];
    for (corner, inward, start) in corners {
        let center = corner + inward * r;
        let mut mesh = Mesh::default();
        mesh.colored_vertex(corner, bg);
        let steps = 10;
        for i in 0..=steps {
            let a = start + (i as f32 / steps as f32) * (PI / 2.0);
            mesh.colored_vertex(center + vec2(a.cos(), a.sin()) * (r + 0.5), bg);
        }
        for i in 0..steps {
            mesh.add_triangle(0, 1 + i, 2 + i);
        }
        painter.add(Shape::mesh(mesh));
    }
}

/// The colours of one banner and tile set.
#[derive(Clone, Copy, Debug)]
pub struct Scheme {
    pub sky_top: Color32,
    pub sky_bottom: Color32,
    pub sun: Color32,
    pub hills: [Color32; 3],
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

pub const SCHEMES: [Scheme; 5] = [
    // Dusk over chestnut woods.
    Scheme {
        sky_top: rgb(0x3a, 0x1f, 0x2e),
        sky_bottom: rgb(0xf0, 0x9a, 0x4e),
        sun: rgb(0xff, 0xd8, 0x8a),
        hills: [rgb(0x8a, 0x4a, 0x2e), rgb(0x5a, 0x2f, 0x1f), rgb(0x2e, 0x1a, 0x12)],
    },
    // Green morning.
    Scheme {
        sky_top: rgb(0x1d, 0x3b, 0x4a),
        sky_bottom: rgb(0x9c, 0xcf, 0xb0),
        sun: rgb(0xfa, 0xf2, 0xc8),
        hills: [rgb(0x4f, 0x8a, 0x5a), rgb(0x2f, 0x5e, 0x3d), rgb(0x1a, 0x33, 0x24)],
    },
    // Violet evening.
    Scheme {
        sky_top: rgb(0x22, 0x1c, 0x45),
        sky_bottom: rgb(0xc0, 0x7a, 0xb8),
        sun: rgb(0xff, 0xd1, 0xe0),
        hills: [rgb(0x6b, 0x4a, 0x8a), rgb(0x45, 0x2e, 0x63), rgb(0x24, 0x18, 0x38)],
    },
    // Autumn red.
    Scheme {
        sky_top: rgb(0x40, 0x16, 0x14),
        sky_bottom: rgb(0xe8, 0x6a, 0x3c),
        sun: rgb(0xff, 0xe0, 0x9a),
        hills: [rgb(0x9a, 0x3a, 0x22), rgb(0x66, 0x24, 0x18), rgb(0x33, 0x12, 0x0e)],
    },
    // Blue winter day.
    Scheme {
        sky_top: rgb(0x12, 0x2a, 0x4a),
        sky_bottom: rgb(0x8f, 0xb8, 0xe0),
        sun: rgb(0xf4, 0xf8, 0xff),
        hills: [rgb(0x5a, 0x78, 0x9a), rgb(0x34, 0x4c, 0x6a), rgb(0x1a, 0x26, 0x38)],
    },
];

/// A stable colour set for a profile id.
pub fn scheme_for(id: &str) -> Scheme {
    let hash = id.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
    SCHEMES[hash as usize % SCHEMES.len()]
}

/// The chestnut mark: a brown nut with a pale base and a pointed tip, `size` high, centred on `center`.
pub fn chestnut(painter: &Painter, center: Pos2, size: f32, body: Color32) {
    let r = size * 0.42;
    let c = center + vec2(0.0, size * 0.06);
    let outline: Vec<Pos2> = (0..48)
        .map(|i| {
            let a = i as f32 / 48.0 * TAU;
            let (s, k) = (a.sin(), a.cos());
            let tip = (-s).max(0.0).powi(6) * 0.55;
            pos2(c.x + r * k * 1.08 * (1.0 - tip * 0.6), c.y + r * s * (if s < 0.0 { 1.0 + tip } else { 0.92 }))
        })
        .collect();
    let light = theme::lighten(body, 0.25);
    let dark = theme::darken(body, 0.35);
    let top = c.y - r * 1.5;
    let bottom = c.y + r;
    painter.add(fan(c, &outline, |p| theme::mix(light, dark, (p.y - top) / (bottom - top))));
    let base: Vec<Pos2> = outline.iter().copied().filter(|p| p.y > c.y + r * 0.42).collect();
    if base.len() > 2 {
        let pale = rgb(0xe6, 0xc9, 0xa0);
        let base_center = pos2(c.x, (c.y + r * 0.42 + bottom) / 2.0);
        painter.add(fan(base_center, &base, |_| pale));
    }
    let shine = c + vec2(-r * 0.38, -r * 0.35);
    painter.circle_filled(shine, r * 0.16, Color32::from_white_alpha(70));
}

/// A rounded tile with a gradient from the profile's colours and the first letter of its name.
pub fn tile(painter: &Painter, rect: Rect, id: &str, name: &str) {
    let scheme = scheme_for(id);
    gradient_rect(
        painter,
        rect,
        theme::radius(theme::radius::M),
        scheme.sky_bottom,
        scheme.hills[2],
        Direction::Vertical,
    );
    let letter: String = name.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        letter,
        font::bold(rect.height() * 0.48),
        color::ON_ACCENT,
    );
}
