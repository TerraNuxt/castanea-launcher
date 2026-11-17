//! The launcher's own bitmap font (BMFont text format): the parser, measuring, kerning and wrapping. Drawing lives in
//! `bmfont/paint.rs`, the scaled atlas levels in `bmfont/atlas.rs`. See docs/ui-design.md, "The brand font".

pub mod atlas;
pub mod paint;

use std::collections::HashMap;
use std::sync::OnceLock;

pub use paint::{BmText, bm_label};

/// The `.fnt` file and the atlas PNG of the font, from the project's own work.
pub const FNT: &str = include_str!("../assets/fonts/castanea/castanea.fnt");
pub const ATLAS_PNG: &[u8] = include_bytes!("../assets/fonts/castanea/castanea.png");

/// One glyph: its rectangle in the atlas (pixels) and how it sits on the line (all in atlas pixels).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyph {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub xoffset: i32,
    pub yoffset: i32,
    pub xadvance: i32,
}

#[derive(Clone, Debug)]
pub struct BitmapFont {
    pub face: String,
    pub size: u32,
    pub line_height: u32,
    pub base: u32,
    pub atlas: [u32; 2],
    pub page_file: String,
    glyphs: HashMap<char, Glyph>,
    kerning: HashMap<(char, char), i32>,
}

/// A glyph placed on a line, in points relative to the top left of the text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub ch: char,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    pub glyphs: Vec<Placed>,
    pub width: f32,
    pub height: f32,
    pub lines: usize,
}

/// Splits `key=value key="a b"` pairs of one BMFont line.
fn pairs(line: &str) -> HashMap<&str, &str> {
    let mut out = HashMap::new();
    let mut rest = line;
    while let Some(eq) = rest.find('=') {
        let key = rest[..eq].rsplit(char::is_whitespace).next().unwrap_or("");
        let after = &rest[eq + 1..];
        let (value, next) = match after.strip_prefix('"') {
            Some(quoted) => match quoted.find('"') {
                Some(end) => (&quoted[..end], &quoted[end + 1..]),
                None => (quoted, ""),
            },
            None => {
                let end = after.find(char::is_whitespace).unwrap_or(after.len());
                (&after[..end], &after[end..])
            }
        };
        out.insert(key, value);
        rest = next;
    }
    out
}

fn number<T: std::str::FromStr>(map: &HashMap<&str, &str>, key: &str, line: usize) -> Result<T, String> {
    let value = map.get(key).ok_or_else(|| format!("line {line}: missing {key}"))?;
    value.parse().map_err(|_| format!("line {line}: {key}={value} is not a number"))
}

impl BitmapFont {
    /// Parses the BMFont text format: `info`, `common`, `page`, `chars`/`char` and `kernings`/`kerning` lines.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut font = Self {
            face: String::new(),
            size: 0,
            line_height: 0,
            base: 0,
            atlas: [0, 0],
            page_file: String::new(),
            glyphs: HashMap::new(),
            kerning: HashMap::new(),
        };
        let (mut declared, mut common) = (None, false);
        for (n, line) in text.lines().enumerate() {
            let n = n + 1;
            let line = line.trim();
            let (tag, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
            let map = pairs(rest);
            match tag {
                "info" => {
                    font.face = map.get("face").copied().unwrap_or("").to_owned();
                    font.size = number(&map, "size", n)?;
                }
                "common" => {
                    font.line_height = number(&map, "lineHeight", n)?;
                    font.base = number(&map, "base", n)?;
                    font.atlas = [number(&map, "scaleW", n)?, number(&map, "scaleH", n)?];
                    if number::<u32>(&map, "pages", n)? != 1 {
                        return Err(format!("line {n}: only one page is supported"));
                    }
                    common = true;
                }
                "page" => font.page_file = map.get("file").copied().unwrap_or("").to_owned(),
                "chars" => declared = Some(number::<usize>(&map, "count", n)?),
                "char" => {
                    let id: u32 = number(&map, "id", n)?;
                    let ch = char::from_u32(id).ok_or_else(|| format!("line {n}: bad char id {id}"))?;
                    let glyph = Glyph {
                        x: number(&map, "x", n)?,
                        y: number(&map, "y", n)?,
                        width: number(&map, "width", n)?,
                        height: number(&map, "height", n)?,
                        xoffset: number(&map, "xoffset", n)?,
                        yoffset: number(&map, "yoffset", n)?,
                        xadvance: number(&map, "xadvance", n)?,
                    };
                    if glyph.x + glyph.width > font.atlas[0] || glyph.y + glyph.height > font.atlas[1] {
                        return Err(format!("line {n}: char {id} lies outside the atlas"));
                    }
                    font.glyphs.insert(ch, glyph);
                }
                "kerning" => {
                    let (first, second): (u32, u32) = (number(&map, "first", n)?, number(&map, "second", n)?);
                    let amount: i32 = number(&map, "amount", n)?;
                    if let (Some(a), Some(b)) = (char::from_u32(first), char::from_u32(second)) {
                        font.kerning.insert((a, b), amount);
                    }
                }
                _ => {}
            }
        }
        if !common || font.line_height == 0 {
            return Err("no common line".to_owned());
        }
        if let Some(count) = declared.filter(|c| *c != font.glyphs.len()) {
            return Err(format!("chars count={count} but {} glyphs", font.glyphs.len()));
        }
        Ok(font)
    }

    /// The font shipped with the launcher (parsed once).
    pub fn builtin() -> &'static Self {
        static FONT: OnceLock<BitmapFont> = OnceLock::new();
        FONT.get_or_init(|| Self::parse(FNT).expect("the bundled castanea.fnt parses"))
    }

    pub fn glyph(&self, ch: char) -> Option<&Glyph> {
        self.glyphs.get(&ch)
    }

    #[cfg(test)]
    pub fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }

    pub fn glyphs(&self) -> impl Iterator<Item = (char, &Glyph)> {
        self.glyphs.iter().map(|(c, g)| (*c, g))
    }

    pub fn kerning(&self, first: char, second: char) -> i32 {
        self.kerning.get(&(first, second)).copied().unwrap_or(0)
    }

    #[cfg(test)]
    pub fn kerning_count(&self) -> usize {
        self.kerning.len()
    }

    /// True when every character of `text` has a glyph (a newline counts as covered).
    pub fn covers(&self, text: &str) -> bool {
        text.chars().all(|c| c == '\n' || self.glyphs.contains_key(&c))
    }

    /// The characters of `text` the font lacks, each once, in order.
    #[cfg(test)]
    pub fn missing(&self, text: &str) -> Vec<char> {
        let mut out = Vec::new();
        for c in text.chars() {
            if c != '\n' && !self.glyphs.contains_key(&c) && !out.contains(&c) {
                out.push(c);
            }
        }
        out
    }

    /// Where the capital letters sit on the line: top and bottom in atlas pixels (the cap 'H', shadow included).
    pub fn cap_box(&self) -> (f32, f32) {
        self.glyph('H')
            .map_or((0.0, self.line_height as f32), |g| (g.yoffset as f32, (g.yoffset + g.height as i32) as f32))
    }

    /// Lays `text` out with a line height of `line_px` points, wrapping at `max_width` points when given. Characters
    /// without a glyph are skipped (callers check `covers` first and fall back to another font).
    pub fn layout(&self, text: &str, line_px: f32, max_width: Option<f32>) -> Layout {
        let scale = line_px / self.line_height as f32;
        let space = self.glyph(' ').map_or(0.0, |g| g.xadvance as f32 * scale);
        let mut out = Layout::default();
        let mut line = 0usize;
        for (n, paragraph) in text.split('\n').enumerate() {
            if n > 0 {
                line += 1;
            }
            let (mut x, mut first) = (0.0f32, true);
            for word in paragraph.split(' ') {
                let (glyphs, advance) = self.word(word, scale);
                if !first && max_width.is_some_and(|m| x + space + advance > m) {
                    line += 1;
                    (x, first) = (0.0, true);
                }
                if !first {
                    x += space;
                }
                let top = line as f32 * line_px;
                out.glyphs.extend(glyphs.into_iter().map(|g| Placed { x: g.x + x, y: g.y + top, ..g }));
                x += advance;
                first = false;
                out.width = out.width.max(x);
            }
        }
        out.lines = line + 1;
        out.height = out.lines as f32 * line_px;
        out
    }

    /// One word at x = 0 with its kerning, and its advance.
    fn word(&self, word: &str, scale: f32) -> (Vec<Placed>, f32) {
        let (mut glyphs, mut x, mut prev) = (Vec::new(), 0.0f32, None);
        for ch in word.chars() {
            let Some(glyph) = self.glyph(ch) else { continue };
            if let Some(p) = prev {
                x += self.kerning(p, ch) as f32 * scale;
            }
            if glyph.width > 0 {
                glyphs.push(Placed {
                    ch,
                    x: x + glyph.xoffset as f32 * scale,
                    y: glyph.yoffset as f32 * scale,
                    w: glyph.width as f32 * scale,
                    h: glyph.height as f32 * scale,
                });
            }
            x += glyph.xadvance as f32 * scale;
            prev = Some(ch);
        }
        (glyphs, x)
    }

    /// The width of `text` on one line, in points.
    #[cfg(test)]
    pub fn measure(&self, text: &str, line_px: f32) -> f32 {
        self.layout(text, line_px, None).width
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font() -> &'static BitmapFont {
        BitmapFont::builtin()
    }

    #[test]
    fn the_real_file_parses() {
        let f = font();
        assert_eq!(f.face, "Castanea Copper");
        assert_eq!((f.size, f.line_height, f.base), (150, 235, 180));
        assert_eq!(f.atlas, [2048, 1024]);
        assert_eq!(f.page_file, "castanea.png");
        assert_eq!(f.glyph_count(), 63);
        assert_eq!(f.kerning_count(), 0);
    }

    #[test]
    fn a_few_metrics_match_the_file() {
        let a = font().glyph('A').unwrap();
        assert_eq!((a.x, a.y, a.width, a.height, a.xoffset, a.yoffset, a.xadvance), (3, 3, 205, 171, 0, 24, 213));
        let w = font().glyph('W').unwrap();
        assert_eq!((w.width, w.xadvance), (278, 286));
        assert_eq!(font().glyph(' ').unwrap().xadvance, 90);
    }

    #[test]
    fn lower_case_shares_the_capital_shapes() {
        for (lo, up) in ('a'..='z').zip('A'..='Z') {
            assert_eq!(font().glyph(lo), font().glyph(up), "{lo}");
        }
    }

    #[test]
    fn coverage_is_letters_digits_and_space() {
        let f = font();
        assert!(f.covers("castanea 0123456789 Mods"));
        assert!(!f.covers("Über"));
        assert_eq!(f.missing("Straße, über: x?"), vec!['ß', ',', 'ü', ':', '?']);
    }

    #[test]
    fn every_glyph_lies_inside_the_atlas() {
        let f = font();
        for (c, g) in f.glyphs() {
            assert!(g.x + g.width <= f.atlas[0] && g.y + g.height <= f.atlas[1], "{c}");
        }
    }

    #[test]
    fn kerning_lines_are_read() {
        let text = "common lineHeight=10 base=8 scaleW=64 scaleH=64 pages=1\nchars count=2\n\
            char id=65 x=0 y=0 width=5 height=5 xoffset=0 yoffset=0 xadvance=6 page=0\n\
            char id=86 x=8 y=0 width=5 height=5 xoffset=0 yoffset=0 xadvance=6 page=0\n\
            kernings count=1\nkerning first=65 second=86 amount=-2\n";
        let f = BitmapFont::parse(text).unwrap();
        assert_eq!(f.kerning('A', 'V'), -2);
        assert_eq!(f.kerning('V', 'A'), 0);
        assert!((f.measure("AV", 10.0) - 10.0).abs() < 1e-4);
    }

    #[test]
    fn bad_files_are_refused() {
        assert!(BitmapFont::parse("").is_err());
        assert!(BitmapFont::parse("common lineHeight=10 base=8 scaleW=64 scaleH=64 pages=2").is_err());
        let wrong = "common lineHeight=10 base=8 scaleW=64 scaleH=64 pages=1\nchars count=3\n";
        assert!(BitmapFont::parse(wrong).is_err());
    }

    #[test]
    fn the_width_of_a_known_string() {
        // C + A + S + T + A + N + E + A: the sum of the advances, scaled by size / line height.
        let advances = [182, 213, 166, 170, 213, 180, 157, 213];
        let sum: i32 = advances.iter().sum();
        let width = font().measure("castanea", 23.5);
        assert!((width - sum as f32 * 23.5 / 235.0).abs() < 0.01, "{width}");
    }

    #[test]
    fn wrapping_breaks_at_spaces() {
        let one = font().layout("MODS ARE GOOD", 20.0, None);
        assert_eq!(one.lines, 1);
        let wrapped = font().layout("MODS ARE GOOD", 20.0, Some(one.width * 0.6));
        assert!(wrapped.lines >= 2, "{wrapped:?}");
        assert!(wrapped.glyphs.iter().all(|g| g.x >= -0.01), "no glyph is pushed left of the line start");
        assert!(wrapped.height > one.height);
    }

    #[test]
    fn the_text_sits_on_the_baseline() {
        let l = font().layout("H", 235.0, None);
        let g = l.glyphs[0];
        assert_eq!((g.y, g.h), (24.0, 171.0));
        assert_eq!(font().cap_box(), (24.0, 195.0));
    }
}
