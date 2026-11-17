//! The atlas at the sizes the UI needs. The PNG is 235 px per line; text is drawn at 14 to 60 px, so drawing it
//! straight from the GPU with linear filtering would shimmer. Instead each glyph is area-averaged (an exact box
//! filter on premultiplied pixels) into a pyramid of levels, 1/sqrt(2) apart, packed with a transparent border so no
//! neighbour bleeds in. The painter picks the level that is just sharper than the screen size, so the GPU only has to
//! shrink by less than 1.42 and linear filtering is enough. Levels are built when first used, one texture each.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::BitmapFont;

/// How many levels there are: scale 1, 0.71, 0.5 ... down to about 0.044.
pub const LEVELS: usize = 9;
/// The transparent border around each packed glyph, in level pixels.
const PAD: usize = 2;

/// The atlas as premultiplied RGBA.
pub struct Source {
    pub size: [usize; 2],
    pub rgba: Vec<u8>,
}

/// Where one glyph sits in a level (pixels of the level's image, without the border).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

pub struct Level {
    pub scale: f32,
    pub size: [usize; 2],
    /// Premultiplied RGBA.
    pub rgba: Vec<u8>,
    pub cells: HashMap<char, Cell>,
}

impl Source {
    /// The bundled atlas, decoded once.
    pub fn builtin() -> &'static Self {
        static SOURCE: OnceLock<Source> = OnceLock::new();
        SOURCE.get_or_init(|| Self::decode(super::ATLAS_PNG).expect("the bundled castanea.png decodes"))
    }

    pub fn decode(png: &[u8]) -> Result<Self, String> {
        let image = image::load_from_memory_with_format(png, image::ImageFormat::Png).map_err(|e| e.to_string())?;
        let image = image.to_rgba8();
        let size = [image.width() as usize, image.height() as usize];
        let mut rgba = image.into_raw();
        for px in rgba.chunks_mut(4) {
            let a = px[3] as u32;
            for c in &mut px[..3] {
                *c = ((*c as u32 * a + 127) / 255) as u8;
            }
        }
        Ok(Self { size, rgba })
    }

    /// Level `k` for the glyphs of `font`.
    pub fn level(&self, font: &BitmapFont, k: usize) -> Level {
        let scale = level_scale(k);
        let mut glyphs: Vec<(char, &super::Glyph)> =
            font.glyphs().filter(|(_, g)| g.width > 0 && g.height > 0).collect();
        glyphs.sort_by_key(|(c, _)| *c);
        let sizes: Vec<(usize, usize)> = glyphs
            .iter()
            .map(|(_, g)| ((g.width as f32 * scale).ceil() as usize, (g.height as f32 * scale).ceil() as usize))
            .collect();
        let area: usize = sizes.iter().map(|(w, h)| (w + PAD * 2) * (h + PAD * 2)).sum();
        let width =
            ((area as f32 * 1.3).sqrt().ceil() as usize).max(sizes.iter().map(|s| s.0).max().unwrap_or(1) + PAD * 2);
        let width = width.next_power_of_two().max(32);
        let (mut x, mut y, mut shelf) = (0usize, 0usize, 0usize);
        let mut cells = HashMap::new();
        let mut places = Vec::new();
        for ((ch, _), (w, h)) in glyphs.iter().zip(&sizes) {
            if x + w + PAD * 2 > width {
                (x, y, shelf) = (0, y + shelf, 0);
            }
            let cell = Cell { x: x + PAD, y: y + PAD, w: *w, h: *h };
            cells.insert(*ch, cell);
            places.push(cell);
            x += w + PAD * 2;
            shelf = shelf.max(h + PAD * 2);
        }
        let height = (y + shelf).max(1);
        let mut rgba = vec![0u8; width * height * 4];
        for ((_, g), cell) in glyphs.iter().zip(&places) {
            self.box_filter(g, scale, cell, width, &mut rgba);
        }
        Level { scale, size: [width, height], rgba, cells }
    }

    /// Averages the glyph's pixels over the area each destination pixel covers; outside the glyph is transparent.
    fn box_filter(&self, g: &super::Glyph, scale: f32, cell: &Cell, stride: usize, out: &mut [u8]) {
        let step = 1.0 / scale;
        let norm = scale * scale;
        let source = |x: usize, y: usize| -> [f32; 4] {
            if x >= g.width as usize || y >= g.height as usize {
                return [0.0; 4];
            }
            let i = ((g.y as usize + y) * self.size[0] + g.x as usize + x) * 4;
            [self.rgba[i] as f32, self.rgba[i + 1] as f32, self.rgba[i + 2] as f32, self.rgba[i + 3] as f32]
        };
        for dy in 0..cell.h {
            let (y0, y1) = (dy as f32 * step, (dy + 1) as f32 * step);
            for dx in 0..cell.w {
                let (x0, x1) = (dx as f32 * step, (dx + 1) as f32 * step);
                let mut sum = [0.0f32; 4];
                for sy in y0.floor() as usize..y1.ceil() as usize {
                    let wy = (y1.min(sy as f32 + 1.0) - y0.max(sy as f32)).max(0.0);
                    for sx in x0.floor() as usize..x1.ceil() as usize {
                        let wx = (x1.min(sx as f32 + 1.0) - x0.max(sx as f32)).max(0.0);
                        let p = source(sx, sy);
                        for c in 0..4 {
                            sum[c] += p[c] * wx * wy;
                        }
                    }
                }
                let o = ((cell.y + dy) * stride + cell.x + dx) * 4;
                for c in 0..4 {
                    out[o + c] = (sum[c] * norm).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
}

/// The size of level `k` relative to the atlas.
pub fn level_scale(k: usize) -> f32 {
    std::f32::consts::FRAC_1_SQRT_2.powi(k as i32)
}

/// The deepest level that still has at least `screen_scale` (physical pixels per atlas pixel).
pub fn pick(screen_scale: f32) -> usize {
    (0..LEVELS).rev().find(|k| level_scale(*k) >= screen_scale * 0.999).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_atlas_decodes_at_its_declared_size() {
        let font = BitmapFont::builtin();
        let source = Source::builtin();
        assert_eq!(source.size, [font.atlas[0] as usize, font.atlas[1] as usize]);
    }

    #[test]
    fn the_atlas_is_colour_with_transparency() {
        let source = Source::builtin();
        let alphas = source.rgba.chunks(4).map(|p| p[3]).collect::<Vec<_>>();
        assert_eq!(alphas[0], 0, "the corner is transparent");
        assert!(alphas.contains(&255), "there are opaque pixels");
        let (mut warm, mut total) = (0u64, 0u64);
        for p in source.rgba.chunks(4).filter(|p| p[3] == 255) {
            total += 1;
            warm += (p[0] > p[2]) as u64;
        }
        assert!(warm * 10 > total * 9, "copper: red above blue in nearly every opaque pixel");
    }

    #[test]
    fn picking_a_level() {
        assert_eq!(pick(1.0), 0);
        assert_eq!(pick(0.5), 2);
        assert_eq!(pick(0.14), 5);
        assert_eq!(pick(0.001), LEVELS - 1);
        for s in [0.05f32, 0.1, 0.2, 0.4, 0.8] {
            let k = pick(s);
            assert!(level_scale(k) >= s && level_scale(k) / s < 1.5, "{s} -> level {k}");
        }
    }

    #[test]
    fn a_level_keeps_every_glyph_apart_and_its_coverage() {
        let font = BitmapFont::builtin();
        let source = Source::builtin();
        for k in [0, 3, 6] {
            let level = source.level(font, k);
            assert_eq!(level.rgba.len(), level.size[0] * level.size[1] * 4);
            let cells: Vec<_> = level.cells.values().collect();
            for (i, a) in cells.iter().enumerate() {
                assert!(a.x + a.w + PAD <= level.size[0] && a.y + a.h + PAD <= level.size[1]);
                for b in &cells[i + 1..] {
                    let apart = a.x + a.w + PAD <= b.x
                        || b.x + b.w + PAD <= a.x
                        || a.y + a.h + PAD <= b.y
                        || b.y + b.h + PAD <= a.y;
                    assert!(apart, "level {k}: {a:?} touches {b:?}");
                }
            }
            // Total alpha, scaled back to atlas pixels, matches the source: nothing is lost or invented.
            let g = font.glyph('O').unwrap();
            let cell = level.cells[&'O'];
            let mut got = 0.0f64;
            for y in 0..cell.h {
                for x in 0..cell.w {
                    got += level.rgba[((cell.y + y) * level.size[0] + cell.x + x) * 4 + 3] as f64;
                }
            }
            let mut want = 0.0f64;
            for y in 0..g.height as usize {
                for x in 0..g.width as usize {
                    want += source.rgba[((g.y as usize + y) * source.size[0] + g.x as usize + x) * 4 + 3] as f64;
                }
            }
            let ratio = got / (want * (level.scale as f64).powi(2));
            assert!((ratio - 1.0).abs() < 0.02, "level {k}: coverage ratio {ratio}");
        }
    }
}
