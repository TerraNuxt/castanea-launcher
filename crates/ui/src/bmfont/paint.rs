//! Drawing bitmap-font text with egui: texture levels, pixel snapping, the tint, the Inter fallback for text the font
//! cannot spell, and the screen reader name.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, Mesh, Pos2, Rect, Response, Role, Sense, Shape, TextureHandle, TextureOptions, Vec2, WidgetInfo,
};

use super::atlas::{self, Cell, Source};
use super::{BitmapFont, Layout};
use crate::theme::{color, font};

/// One level's texture and where each glyph sits in it.
struct LevelTexture {
    texture: TextureHandle,
    scale: f32,
    size: [usize; 2],
    cells: std::collections::HashMap<char, Cell>,
}

/// The textures built so far, one slot per level, kept in the context.
#[derive(Clone)]
struct Levels(Vec<Option<Arc<LevelTexture>>>);

fn level_texture(ctx: &egui::Context, k: usize) -> Arc<LevelTexture> {
    if let Some(found) = ctx.data(|d| d.get_temp::<Levels>(egui::Id::NULL)).and_then(|l| l.0[k].clone()) {
        return found;
    }
    let level = Source::builtin().level(BitmapFont::builtin(), k);
    let image = egui::ColorImage::from_rgba_premultiplied(level.size, &level.rgba);
    let texture = ctx.load_texture(format!("bmfont-{k}"), image, TextureOptions::LINEAR);
    let built = Arc::new(LevelTexture { texture, scale: level.scale, size: level.size, cells: level.cells });
    ctx.data_mut(|d| {
        let mut levels = d.get_temp::<Levels>(egui::Id::NULL).unwrap_or_else(|| Levels(vec![None; atlas::LEVELS]));
        levels.0[k] = Some(built.clone());
        d.insert_temp(egui::Id::NULL, levels);
    });
    built
}

/// Text in the brand font. Letters are the copper of the atlas multiplied by `tint` (white keeps them as drawn); text
/// the font cannot spell (punctuation, umlauts) is set whole in Inter semibold in `fallback` instead.
#[derive(Clone)]
pub struct BmText<'a> {
    text: &'a str,
    size: f32,
    tint: Color32,
    fallback: Color32,
    wrap: Option<f32>,
}

/// How a string will be drawn.
enum Prepared {
    Bitmap(Layout),
    Fallback(Arc<egui::Galley>),
}

impl<'a> BmText<'a> {
    /// `size` is the line height in points (the capitals are 73 % of it).
    pub fn new(text: &'a str, size: f32) -> Self {
        Self { text, size, tint: Color32::WHITE, fallback: color::TEXT, wrap: None }
    }

    pub fn tint(mut self, tint: Color32) -> Self {
        self.tint = tint;
        self
    }

    pub fn fallback(mut self, fallback: Color32) -> Self {
        self.fallback = fallback;
        self
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn wrap(mut self, width: f32) -> Self {
        self.wrap = Some(width);
        self
    }

    /// True when the whole text is drawn with the bitmap font.
    pub fn is_bitmap(&self) -> bool {
        BitmapFont::builtin().covers(self.text)
    }

    fn prepare(&self, ctx: &egui::Context) -> Prepared {
        let bm = BitmapFont::builtin();
        if bm.covers(self.text) {
            return Prepared::Bitmap(bm.layout(self.text, self.size, self.wrap));
        }
        let job = egui::text::LayoutJob::simple(
            self.text.to_owned(),
            font::semibold(self.size),
            self.fallback,
            self.wrap.unwrap_or(f32::INFINITY),
        );
        Prepared::Fallback(ctx.fonts_mut(|f| f.layout_job(job)))
    }

    /// The box the text takes: the capitals' height for one line, so it lines up with other controls.
    pub fn measure(&self, ctx: &egui::Context) -> Vec2 {
        let bm = BitmapFont::builtin();
        match self.prepare(ctx) {
            Prepared::Bitmap(layout) => {
                let (top, bottom) = bm.cap_box();
                let cap = (bottom - top) * self.size / bm.line_height as f32;
                Vec2::new(layout.width, layout.height - self.size + cap)
            }
            Prepared::Fallback(galley) => galley.size(),
        }
    }

    /// Draws the text with the top left of its box at `at`; returns the box.
    pub fn paint(&self, painter: &egui::Painter, at: Pos2) -> Rect {
        let ctx = painter.ctx();
        let size = self.measure(ctx);
        let rect = Rect::from_min_size(at, size);
        match self.prepare(ctx) {
            Prepared::Fallback(galley) => {
                painter.galley(at, galley, self.fallback);
            }
            Prepared::Bitmap(layout) => {
                let bm = BitmapFont::builtin();
                let (cap_top, _) = bm.cap_box();
                let origin = Pos2::new(at.x, at.y - cap_top * self.size / bm.line_height as f32);
                paint_layout(painter, &layout, origin, self.size, self.tint);
            }
        }
        rect
    }

    /// Lays the text out in `ui`, draws it and names it for screen readers as a heading.
    pub fn show(self, ui: &mut egui::Ui) -> Response {
        self.show_as(ui, Role::Heading)
    }

    pub fn show_as(self, ui: &mut egui::Ui, role: Role) -> Response {
        let size = self.measure(ui.ctx());
        let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
        if ui.is_rect_visible(rect) {
            self.paint(ui.painter(), rect.min);
        }
        let name = self.text.to_owned();
        response.widget_info(|| WidgetInfo::labeled(role, true, &name));
        response
    }
}

/// A title in the brand font as a page heading.
pub fn bm_label(ui: &mut egui::Ui, text: &str, size: f32) -> Response {
    BmText::new(text, size).show(ui)
}

/// Draws a laid-out string whose line box starts at `origin` (points), on physical pixel boundaries.
fn paint_layout(painter: &egui::Painter, layout: &Layout, origin: Pos2, size: f32, tint: Color32) {
    let ctx = painter.ctx();
    let bm = BitmapFont::builtin();
    let ppp = ctx.pixels_per_point();
    let screen_scale = size * ppp / bm.line_height as f32;
    let level = level_texture(ctx, atlas::pick(screen_scale));
    let mut mesh = Mesh::with_texture(level.texture.id());
    let snap = |v: f32| (v * ppp).round() / ppp;
    for g in &layout.glyphs {
        let Some(cell) = level.cells.get(&g.ch) else { continue };
        let Some(glyph) = bm.glyph(g.ch) else { continue };
        // The cell is the glyph rounded up to whole level pixels; the quad grows by the same share.
        let (fx, fy) =
            (cell.w as f32 / level.scale / glyph.width as f32, cell.h as f32 / level.scale / glyph.height as f32);
        let min = Pos2::new(snap(origin.x + g.x), snap(origin.y + g.y));
        let rect = Rect::from_min_size(min, Vec2::new(g.w * fx, g.h * fy));
        let [tw, th] = level.size.map(|v| v as f32);
        let uv = Rect::from_min_max(
            Pos2::new(cell.x as f32 / tw, cell.y as f32 / th),
            Pos2::new((cell.x + cell.w) as f32 / tw, (cell.y + cell.h) as f32 / th),
        );
        mesh.add_rect_with_uv(rect, uv, tint);
    }
    painter.add(Shape::mesh(mesh));
}
