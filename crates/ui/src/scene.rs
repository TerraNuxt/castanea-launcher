//! The home page scene: `assets/scene.svg` read at start and painted with egui's vector shapes, so it is crisp at any
//! size and costs nothing while the window is idle. Only the tiny SVG subset the file uses is understood: convex
//! `<polygon>`, `<rect>`, `<circle>` and `<ellipse>` with `fill="#rrggbb"` and `opacity`.

use std::sync::OnceLock;

use eframe::egui::{self, Color32, Pos2, Rect, Vec2};

const SOURCE: &str = include_str!("../assets/scene.svg");

#[derive(Clone, Debug, PartialEq)]
pub enum Prim {
    Polygon(Vec<Pos2>),
    Rect(Rect),
    Ellipse { center: Pos2, radius: Vec2 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub prim: Prim,
    /// Straight (not premultiplied) colour with the opacity as alpha.
    pub color: Color32,
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub size: Vec2,
    pub shapes: Vec<Shape>,
}

/// The scene of the launcher, read once.
pub fn builtin() -> &'static Scene {
    static SCENE: OnceLock<Scene> = OnceLock::new();
    SCENE.get_or_init(|| parse(SOURCE).unwrap_or_else(|error| Scene { size: egui::vec2(1.0, 1.0), shapes: log(error) }))
}

fn log(error: String) -> Vec<Shape> {
    eprintln!("castanea-launcher: the home scene is broken: {error}");
    Vec::new()
}

/// The attribute `name` of the tag text `tag` (`<circle cx="1" …/>`).
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let start = tag.find(&key)? + key.len();
    Some(&tag[start..start + tag[start..].find('"')?])
}

fn number(tag: &str, name: &str) -> Result<f32, String> {
    let text = attr(tag, name).ok_or_else(|| format!("{name} is missing in {tag}"))?;
    text.trim()
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("{name}=\"{text}\" is not a number"))
}

fn color(tag: &str) -> Result<Color32, String> {
    let fill = attr(tag, "fill").ok_or_else(|| format!("no fill in {tag}"))?;
    let hex =
        fill.strip_prefix('#').filter(|h| h.len() == 6).ok_or_else(|| format!("fill=\"{fill}\" is not #rrggbb"))?;
    let part = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| format!("fill=\"{fill}\" is not #rrggbb"));
    let opacity = attr(tag, "opacity").map_or(Ok(1.0), |o| o.parse::<f32>().map_err(|_| format!("opacity=\"{o}\"")))?;
    Ok(Color32::from_rgba_unmultiplied(part(0)?, part(2)?, part(4)?, (opacity.clamp(0.0, 1.0) * 255.0).round() as u8))
}

fn points(tag: &str) -> Result<Vec<Pos2>, String> {
    let text = attr(tag, "points").ok_or_else(|| format!("no points in {tag}"))?;
    let numbers: Result<Vec<f32>, _> =
        text.split([' ', ',']).filter(|p| !p.is_empty()).map(str::parse::<f32>).collect();
    let numbers = numbers.map_err(|_| format!("points=\"{text}\" are not numbers"))?;
    if numbers.len() < 6 || numbers.len() % 2 != 0 {
        return Err(format!("points=\"{text}\": a polygon needs three or more points"));
    }
    Ok(numbers.chunks(2).map(|p| egui::pos2(p[0], p[1])).collect())
}

/// Whether the corners turn the same way all round; egui fills only convex polygons without artefacts.
pub fn convex(points: &[Pos2]) -> bool {
    let turn = |i: usize| {
        let (a, b, c) = (points[i], points[(i + 1) % points.len()], points[(i + 2) % points.len()]);
        (b - a).x * (c - b).y - (b - a).y * (c - b).x
    };
    let signs: Vec<f32> = (0..points.len()).map(turn).filter(|t| t.abs() > 1e-3).collect();
    signs.iter().all(|t| *t > 0.0) || signs.iter().all(|t| *t < 0.0)
}

pub fn parse(svg: &str) -> Result<Scene, String> {
    let mut size = None;
    let mut shapes = Vec::new();
    let mut rest = svg;
    while let Some(open) = rest.find('<') {
        rest = &rest[open..];
        if let Some(comment) = rest.strip_prefix("<!--") {
            rest = &comment[comment.find("-->").ok_or("a comment never ends")? + 3..];
            continue;
        }
        let end = rest.find('>').ok_or("a tag never ends")?;
        let tag = &rest[..end];
        rest = &rest[end + 1..];
        let name = tag[1..].split(char::is_whitespace).next().unwrap_or("");
        let prim = match name {
            "svg" => {
                let view: Vec<f32> =
                    attr(tag, "viewBox").unwrap_or("").split(' ').filter_map(|v| v.parse().ok()).collect();
                if view.len() != 4 || view[0] != 0.0 || view[1] != 0.0 {
                    return Err("viewBox must be `0 0 W H`".to_owned());
                }
                size = Some(egui::vec2(view[2], view[3]));
                continue;
            }
            "polygon" => {
                let corners = points(tag)?;
                if !convex(&corners) {
                    return Err(format!("a polygon is not convex: {tag}"));
                }
                Prim::Polygon(corners)
            }
            "rect" => {
                let min = egui::pos2(number(tag, "x")?, number(tag, "y")?);
                Prim::Rect(Rect::from_min_size(min, egui::vec2(number(tag, "width")?, number(tag, "height")?)))
            }
            "circle" => {
                let r = number(tag, "r")?;
                Prim::Ellipse { center: egui::pos2(number(tag, "cx")?, number(tag, "cy")?), radius: egui::vec2(r, r) }
            }
            "ellipse" => Prim::Ellipse {
                center: egui::pos2(number(tag, "cx")?, number(tag, "cy")?),
                radius: egui::vec2(number(tag, "rx")?, number(tag, "ry")?),
            },
            "title" | "/title" | "/svg" | "?xml" => continue,
            other => return Err(format!("<{other}> is not supported")),
        };
        shapes.push(Shape { prim, color: color(tag)? });
    }
    let size = size.filter(|s| s.x > 0.0 && s.y > 0.0).ok_or("no <svg viewBox>")?;
    Ok(Scene { size, shapes })
}

impl Scene {
    /// Where the scene lies in `area`: at least as wide as the area (and at least 45 % as tall), its bottom edge on the
    /// area's bottom. When it is wider than the area, a little more is cut on the right, where there is only sky and
    /// the smeltery, than on the left, where the sun is.
    pub fn placement(&self, area: Rect) -> (Pos2, f32) {
        let scale = (area.width() / self.size.x).max(area.height() * 0.45 / self.size.y);
        let spare = area.width() - self.size.x * scale;
        let origin = egui::pos2(area.left() + spare * 0.35, area.bottom() - self.size.y * scale);
        (origin, scale)
    }

    pub fn paint(&self, painter: &egui::Painter, area: Rect) {
        let painter = painter.with_clip_rect(area.intersect(painter.clip_rect()));
        let (origin, scale) = self.placement(area);
        let at = |p: Pos2| origin + p.to_vec2() * scale;
        for shape in &self.shapes {
            let fill = shape.color;
            painter.add(match &shape.prim {
                Prim::Polygon(corners) => {
                    egui::Shape::convex_polygon(corners.iter().map(|p| at(*p)).collect(), fill, egui::Stroke::NONE)
                }
                Prim::Rect(rect) => egui::Shape::rect_filled(Rect::from_two_pos(at(rect.min), at(rect.max)), 0.0, fill),
                Prim::Ellipse { center, radius } if radius.x == radius.y => {
                    egui::Shape::circle_filled(at(*center), radius.x * scale, fill)
                }
                Prim::Ellipse { center, radius } => {
                    egui::Shape::Ellipse(egui::epaint::EllipseShape::filled(at(*center), *radius * scale, fill))
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::color as theme_color;

    fn over(top: Color32, below: Color32) -> Color32 {
        let a = top.a() as f32 / 255.0;
        let mix = |t: u8, b: u8| (t as f32 * a + b as f32 * (1.0 - a)).round() as u8;
        Color32::from_rgb(mix(top.r(), below.r()), mix(top.g(), below.g()), mix(top.b(), below.b()))
    }

    fn luminance(c: Color32) -> f32 {
        let lin = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn the_scene_file_parses_with_convex_polygons_only() {
        let scene = parse(SOURCE).unwrap();
        assert_eq!(scene.size, egui::vec2(960.0, 480.0));
        assert!(scene.shapes.len() > 40, "{} shapes", scene.shapes.len());
        let inside = Rect::from_min_size(Pos2::ZERO, scene.size).expand(1.0);
        for shape in &scene.shapes {
            if let Prim::Polygon(corners) = &shape.prim {
                assert!(convex(corners), "{corners:?}");
                assert!(corners.iter().all(|p| inside.contains(*p)), "{corners:?} leaves the picture");
            }
        }
    }

    #[test]
    fn the_scene_stays_quiet_and_keeps_text_readable() {
        // Whatever lies under the text, the muted text colour keeps AA contrast and the scene never gets loud.
        for shape in &parse(SOURCE).unwrap().shapes {
            let seen = over(shape.color, theme_color::BG);
            assert!(contrast(theme_color::TEXT_MUTED, seen) >= 4.5, "{shape:?}: text on it is {seen:?}");
            assert!(contrast(theme_color::TEXT, seen) >= 7.0, "{shape:?}");
            assert!(
                contrast(seen, theme_color::BG) <= 1.5 || shape.color.a() < 128,
                "{shape:?} is too bright: {seen:?}"
            );
        }
    }

    #[test]
    fn broken_files_are_errors() {
        let ok = r##"<svg viewBox="0 0 10 10"><rect x="0" y="0" width="5" height="5" fill="#101010"/></svg>"##;
        assert_eq!(parse(ok).unwrap().shapes.len(), 1);
        for bad in [
            "",
            r##"<svg viewBox="0 0 10 10"><polygon points="0,0 5,5 0,5 5,0" fill="#101010"/></svg>"##,
            r##"<svg viewBox="0 0 10 10"><rect x="a" y="0" width="5" height="5" fill="#101010"/></svg>"##,
            r#"<svg viewBox="0 0 10 10"><rect x="0" y="0" width="5" height="5" fill="red"/></svg>"#,
            r#"<svg viewBox="0 0 10 10"><path d="M0 0"/></svg>"#,
            r##"<rect x="0" y="0" width="5" height="5" fill="#101010"/>"##,
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn placement_keeps_the_scene_on_the_bottom_edge_at_every_size() {
        let scene = parse(SOURCE).unwrap();
        for (w, h) in [(650.0, 330.0), (680.0, 560.0), (1300.0, 900.0)] {
            let area = Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(w, h));
            let (origin, scale) = scene.placement(area);
            let bottom = origin.y + scene.size.y * scale;
            assert!((bottom - area.bottom()).abs() < 0.01);
            assert!(scene.size.x * scale >= area.width() - 0.01, "covers the width");
            assert!(scene.size.y * scale >= area.height() * 0.45 - 0.01);
            assert!(origin.x <= area.left() + 0.01 && origin.x + scene.size.x * scale >= area.right() - 0.01);
        }
    }
}
