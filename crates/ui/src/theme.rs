//! The launcher's look as tokens: colours, spacing, radius, text sizes, durations and layout sizes
//! (docs/ui-design.md). No colour or size literal lives outside this file.

use std::sync::Arc;

use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Stroke, Visuals};

/// Colours: three flat surfaces, hairlines, three text greys, one muted accent, and two signal colours for text.
pub mod color {
    use eframe::egui::Color32;

    /// The sidebar, the log and other sunken areas.
    pub const BASE: Color32 = Color32::from_rgb(0x13, 0x12, 0x11);
    /// The page background.
    pub const BG: Color32 = Color32::from_rgb(0x19, 0x18, 0x17);
    /// Raised: fields, the selected sidebar item, dialogs, toasts.
    pub const RAISED: Color32 = Color32::from_rgb(0x22, 0x21, 0x1f);
    pub const HOVER: Color32 = Color32::from_rgb(0x29, 0x27, 0x25);
    pub const PRESSED: Color32 = Color32::from_rgb(0x30, 0x2e, 0x2b);
    /// 1 px separators and outlines.
    pub const BORDER: Color32 = Color32::from_rgb(0x2e, 0x2c, 0x29);
    pub const BORDER_STRONG: Color32 = Color32::from_rgb(0x45, 0x42, 0x3e);
    pub const TEXT: Color32 = Color32::from_rgb(0xeb, 0xe7, 0xe3);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(0xa3, 0x9d, 0x97);
    /// Only for disabled controls, which need no contrast.
    pub const TEXT_DISABLED: Color32 = Color32::from_rgb(0x66, 0x61, 0x5c);
    /// Muted chestnut: the primary action, focus and selection, nothing else.
    pub const ACCENT: Color32 = Color32::from_rgb(0xa3, 0x60, 0x2f);
    pub const ACCENT_HOVER: Color32 = Color32::from_rgb(0x96, 0x58, 0x2b);
    pub const ACCENT_PRESSED: Color32 = Color32::from_rgb(0x88, 0x50, 0x27);
    pub const ON_ACCENT: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
    /// Errors, crashes and destructive buttons.
    pub const DANGER: Color32 = Color32::from_rgb(0xe0, 0x78, 0x6c);
    pub const DANGER_FILL: Color32 = Color32::from_rgb(0x8f, 0x3a, 0x32);
    pub const WARN: Color32 = Color32::from_rgb(0xd6, 0xae, 0x5e);
    pub const BACKDROP: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 140);
}

/// The spacing scale in points (4 and 8 steps).
pub mod space {
    pub const XS: f32 = 4.0;
    pub const S: f32 = 8.0;
    pub const M: f32 = 12.0;
    pub const L: f32 = 16.0;
    pub const XL: f32 = 24.0;
    pub const XXL: f32 = 32.0;
}

/// Corner radius: one for everything, a smaller one for small inner parts.
pub mod radius {
    pub const M: u8 = 5;
    pub const S: u8 = 4;
    pub const FULL: u8 = 255;
}

/// Text sizes in points: Inter for text, and three sizes for the brand font (its size is the line height).
pub mod size {
    pub const SMALL: f32 = 12.0;
    pub const BODY: f32 = 14.0;
    pub const TITLE: f32 = 18.0;
    /// The brand font (docs/ui-design.md) takes its size as the line height; its capitals are 73 % of it.
    /// The wordmark on the home page.
    pub const BRAND: f32 = 52.0;
    /// Page titles.
    pub const HEADING: f32 = 30.0;
    /// The label of the Play and Stop button.
    pub const BRAND_BUTTON: f32 = 28.0;
    /// Icon glyphs, not text.
    pub const ICON: f32 = 16.0;
}

/// Lines and strokes in points.
pub mod line {
    pub const HAIRLINE: f32 = 1.0;
    pub const FOCUS: f32 = 1.5;
    pub const PROGRESS: f32 = 2.0;
}

/// Durations in seconds.
pub mod anim {
    pub const HOVER: f32 = 0.12;
    pub const TOAST: f64 = 6.0;
    pub const TOAST_ERROR: f64 = 10.0;
    /// While a game or a job runs, the window redraws this often (log, progress, running time).
    pub const BUSY_REPAINT: f64 = 0.25;
}

/// Layout sizes in points.
pub mod layout {
    pub const SIDEBAR: f32 = 184.0;
    pub const NAV_ITEM: f32 = 32.0;
    pub const CONTROL: f32 = 32.0;
    pub const PLAY: [f32; 2] = [240.0, 48.0];
    pub const PAGE_MARGIN: f32 = 32.0;
    pub const PAGE_MARGIN_TOP: f32 = 24.0;
    /// Pages of rows (settings, about) stay this wide at most.
    pub const READABLE: f32 = 720.0;
    /// The settings label column.
    pub const LABEL: f32 = 220.0;
    pub const FIELD: f32 = 320.0;
    /// The mod browser's list column.
    pub const LIST: f32 = 320.0;
    pub const LIST_ROW: f32 = 60.0;
    /// The gap between a title line and its second line.
    pub const LINE_GAP: f32 = 2.0;
    /// How far focus rings and selected segments sit from their control.
    pub const INSET: f32 = 2.0;
    pub const KNOB_INSET: f32 = 3.0;
    pub const DOT: f32 = 3.0;
    /// Puts a row label on the same line as the text of a 32 pt control next to it.
    pub const LABEL_NUDGE: f32 = 6.0;
    /// Where the home block sits: this share of the free height goes above it.
    pub const HOME_RISE: f32 = 0.42;
    /// The content width below which the mod browser shows the list or the details, not both, and the home page puts
    /// the news below the Play block instead of beside it.
    pub const NARROW: f32 = 720.0;
    /// The news column of the home page: its share of the content width, at least and at most.
    pub const NEWS_SHARE: f32 = 0.32;
    pub const NEWS_MIN: f32 = 280.0;
    pub const NEWS_MAX: f32 = 380.0;
    /// The height of the Play block's area when the news sits below it.
    /// The Continue card of the home page: width and height.
    pub const CONTINUE: [f32; 2] = [300.0, 64.0];
    pub const HERO_NARROW: f32 = 280.0;
    pub const DIALOG: f32 = 440.0;
    /// The tallest list in a question before it scrolls.
    pub const DIALOG_LIST: f32 = 220.0;
    pub const TOAST: f32 = 360.0;
    pub const SWITCH: [f32; 2] = [32.0, 18.0];
    pub const IMAGE: f32 = 48.0;
    pub const SCREENSHOT: f32 = 220.0;
    pub const LOG_ROW: f32 = 16.0;
    /// The smallest window the layout supports: the pages scroll or compact (narrow home) below their usual size.
    pub const MIN_WINDOW: [f32; 2] = [880.0, 500.0];
    pub const START_WINDOW: [f32; 2] = [1100.0, 700.0];
}

/// Inter in two weights for text, Phosphor (regular, line icons) for icons.
pub mod font {
    use eframe::egui::{FontFamily, FontId};

    pub const SEMIBOLD: &str = "inter-semibold";
    pub const ICONS: &str = "phosphor";

    pub fn regular(size: f32) -> FontId {
        FontId::proportional(size)
    }

    pub fn semibold(size: f32) -> FontId {
        FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
    }

    pub fn icon(size: f32) -> FontId {
        FontId::new(size, FontFamily::Name(ICONS.into()))
    }

    pub fn mono(size: f32) -> FontId {
        FontId::monospace(size)
    }
}

pub fn radius(r: u8) -> CornerRadius {
    CornerRadius::same(r)
}

pub fn hairline() -> Stroke {
    Stroke::new(line::HAIRLINE, color::BORDER)
}

/// `color` mixed towards `other` by `t` (0 keeps `color`).
pub fn mix(color: Color32, other: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(
        lerp(color.r(), other.r()),
        lerp(color.g(), other.g()),
        lerp(color.b(), other.b()),
        lerp(color.a(), other.a()),
    )
}

fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let mut add = |name: &str, bytes: &'static [u8]| {
        fonts.font_data.insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    };
    add("inter-regular", include_bytes!("../assets/fonts/inter-regular.ttf"));
    add(font::SEMIBOLD, include_bytes!("../assets/fonts/inter-semibold.ttf"));
    add(font::ICONS, include_bytes!("../assets/fonts/phosphor-regular.ttf"));
    let fallback: Vec<String> = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let family = |first: &str| {
        let mut list = vec![first.to_owned(), font::ICONS.to_owned()];
        list.extend(fallback.iter().cloned());
        list
    };
    fonts.families.insert(FontFamily::Proportional, family("inter-regular"));
    fonts.families.insert(FontFamily::Name(font::SEMIBOLD.into()), family(font::SEMIBOLD));
    fonts.families.insert(FontFamily::Name(font::ICONS.into()), vec![font::ICONS.to_owned()]);
    fonts
}

pub fn apply(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    let mut visuals = Visuals::dark();
    visuals.panel_fill = color::BG;
    visuals.window_fill = color::RAISED;
    visuals.window_stroke = hairline();
    visuals.window_corner_radius = radius(radius::M);
    visuals.window_shadow = egui::Shadow::NONE;
    visuals.popup_shadow = egui::Shadow::NONE;
    visuals.menu_corner_radius = radius(radius::M);
    visuals.extreme_bg_color = color::BASE;
    visuals.text_edit_bg_color = Some(color::BASE);
    visuals.faint_bg_color = color::RAISED;
    visuals.code_bg_color = color::BASE;
    visuals.selection.bg_fill = color::ACCENT;
    visuals.selection.stroke = Stroke::new(line::HAIRLINE, color::ACCENT);
    visuals.hyperlink_color = color::TEXT;
    visuals.warn_fg_color = color::WARN;
    visuals.error_fg_color = color::DANGER;
    visuals.override_text_color = None;
    visuals.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.5 };
    let widgets = &mut visuals.widgets;
    widgets.noninteractive.fg_stroke = Stroke::new(line::HAIRLINE, color::TEXT);
    widgets.noninteractive.bg_stroke = hairline();
    widgets.noninteractive.bg_fill = color::BG;
    widgets.inactive.fg_stroke = Stroke::new(line::HAIRLINE, color::TEXT);
    widgets.inactive.bg_fill = color::RAISED;
    widgets.inactive.weak_bg_fill = color::RAISED;
    widgets.inactive.bg_stroke = hairline();
    widgets.hovered.bg_fill = color::HOVER;
    widgets.hovered.weak_bg_fill = color::HOVER;
    widgets.hovered.bg_stroke = Stroke::new(line::HAIRLINE, color::BORDER_STRONG);
    widgets.hovered.fg_stroke = Stroke::new(line::HAIRLINE, color::TEXT);
    widgets.hovered.expansion = 0.0;
    widgets.active.bg_fill = color::PRESSED;
    widgets.active.weak_bg_fill = color::PRESSED;
    widgets.active.bg_stroke = Stroke::new(line::HAIRLINE, color::BORDER_STRONG);
    widgets.active.fg_stroke = Stroke::new(line::HAIRLINE, color::TEXT);
    widgets.active.expansion = 0.0;
    widgets.open.weak_bg_fill = color::HOVER;
    widgets.open.bg_stroke = Stroke::new(line::HAIRLINE, color::BORDER_STRONG);
    for widget in [
        &mut widgets.noninteractive,
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
        &mut widgets.open,
    ] {
        widget.corner_radius = radius(radius::M);
    }
    ctx.set_visuals_of(egui::Theme::Dark, visuals);
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.all_styles_mut(|style| {
        use egui::TextStyle;
        style.spacing.item_spacing = egui::vec2(space::S, space::S);
        style.spacing.button_padding = egui::vec2(space::M, space::XS);
        style.spacing.interact_size.y = layout::CONTROL;
        style.spacing.combo_height = 320.0;
        style.spacing.window_margin = egui::Margin::same(space::L as i8);
        style.spacing.menu_margin = egui::Margin::same(space::XS as i8);
        style.spacing.scroll = egui::style::ScrollStyle::thin();
        style.animation_time = anim::HOVER;
        style.text_styles.insert(TextStyle::Heading, font::semibold(size::TITLE));
        style.text_styles.insert(TextStyle::Body, font::regular(size::BODY));
        style.text_styles.insert(TextStyle::Button, font::regular(size::BODY));
        style.text_styles.insert(TextStyle::Small, font::regular(size::SMALL));
        style.text_styles.insert(TextStyle::Monospace, font::mono(size::SMALL));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn text_meets_wcag_aa_on_every_surface() {
        use color::*;
        for surface in [BASE, BG, RAISED, HOVER] {
            for text in [TEXT, TEXT_MUTED, DANGER, WARN] {
                let ratio = contrast(text, surface);
                assert!(ratio >= 4.5, "{text:?} on {surface:?}: {ratio:.2}");
            }
        }
        for fill in [ACCENT, ACCENT_HOVER, ACCENT_PRESSED, DANGER_FILL] {
            assert!(contrast(ON_ACCENT, fill) >= 4.5, "text on {fill:?}");
        }
        for surface in [BASE, BG, RAISED] {
            assert!(contrast(ACCENT, surface) >= 3.0, "focus ring and selection on {surface:?}");
        }
    }

    #[test]
    fn mix_ends() {
        assert_eq!(mix(Color32::BLACK, Color32::WHITE, 0.5), Color32::from_rgb(128, 128, 128));
        assert_eq!(mix(color::ACCENT, color::BG, 0.0), color::ACCENT);
    }

    #[test]
    fn fonts_have_text_and_icon_families() {
        let fonts = fonts();
        for name in [font::SEMIBOLD, font::ICONS] {
            assert!(fonts.families.contains_key(&FontFamily::Name(name.into())), "{name}");
        }
        assert_eq!(fonts.families[&FontFamily::Proportional][0], "inter-regular");
    }
}
