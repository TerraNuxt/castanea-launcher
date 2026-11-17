//! The launcher's look as tokens: colours, spacing, radii, text sizes, durations and layout sizes (docs/ui-design.md).

use std::sync::Arc;

use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, Visuals};

/// Colours; every page and component takes its colours from here.
pub mod color {
    use eframe::egui::Color32;

    pub const BG: Color32 = Color32::from_rgb(0x13, 0x11, 0x10);
    pub const SIDEBAR: Color32 = Color32::from_rgb(0x0c, 0x0b, 0x0a);
    pub const SURFACE: Color32 = Color32::from_rgb(0x1d, 0x1a, 0x18);
    pub const SURFACE_HOVER: Color32 = Color32::from_rgb(0x26, 0x23, 0x20);
    pub const SURFACE_ACTIVE: Color32 = Color32::from_rgb(0x30, 0x2c, 0x28);
    pub const SUNKEN: Color32 = Color32::from_rgb(0x15, 0x13, 0x12);
    pub const BORDER: Color32 = Color32::from_rgb(0x2f, 0x2a, 0x26);
    pub const TEXT: Color32 = Color32::from_rgb(0xf3, 0xed, 0xe7);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(0xa6, 0x9c, 0x93);
    pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x6e, 0x65, 0x5e);
    pub const ACCENT: Color32 = Color32::from_rgb(0xe0, 0x7a, 0x2f);
    pub const ACCENT_STRONG: Color32 = Color32::from_rgb(0xb8, 0x5a, 0x1c);
    pub const PLAY: Color32 = Color32::from_rgb(0x2f, 0xaa, 0x58);
    pub const DANGER: Color32 = Color32::from_rgb(0xd6, 0x45, 0x3b);
    pub const WARN: Color32 = Color32::from_rgb(0xe5, 0xad, 0x48);
    pub const INFO: Color32 = Color32::from_rgb(0x4b, 0x8f, 0xe0);
    pub const VIOLET: Color32 = Color32::from_rgb(0x8a, 0x6a, 0xd8);
    pub const TEAL: Color32 = Color32::from_rgb(0x2a, 0xa5, 0x96);
    pub const ON_ACCENT: Color32 = Color32::WHITE;
    pub const BACKDROP: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 150);
}

/// The spacing scale in points.
pub mod space {
    pub const XS: f32 = 4.0;
    pub const S: f32 = 8.0;
    pub const M: f32 = 12.0;
    pub const L: f32 = 16.0;
    pub const XL: f32 = 24.0;
    pub const XXL: f32 = 32.0;
}

/// Corner radii.
pub mod radius {
    pub const S: u8 = 6;
    pub const M: u8 = 10;
    pub const L: u8 = 14;
    pub const PILL: u8 = 255;
}

/// Text sizes in points.
pub mod size {
    pub const CAPTION: f32 = 12.0;
    pub const SMALL: f32 = 13.0;
    pub const BODY: f32 = 14.5;
    pub const BUTTON: f32 = 15.0;
    pub const TITLE: f32 = 17.0;
    pub const HEADING: f32 = 22.0;
    pub const DISPLAY: f32 = 28.0;
    pub const ICON: f32 = 20.0;
}

/// Animation durations in seconds.
pub mod anim {
    pub const HOVER: f32 = 0.12;
    pub const PRESS: f32 = 0.08;
    pub const PAGE: f32 = 0.16;
    pub const TOAST: f64 = 5.0;
    pub const TOAST_FADE: f64 = 0.25;
}

/// Layout sizes in points.
pub mod layout {
    pub const SIDEBAR: f32 = 72.0;
    pub const SIDEBAR_ITEM: f32 = 44.0;
    pub const TITLE_BAR: f32 = 38.0;
    pub const RIGHT_COLUMN: f32 = 320.0;
    /// The content width from which a page shows two columns.
    pub const WIDE: f32 = 960.0;
    /// The content width below which the mod browser shows list or details, not both.
    pub const NARROW: f32 = 760.0;
    pub const MIN_WINDOW: [f32; 2] = [1000.0, 640.0];
    pub const START_WINDOW: [f32; 2] = [1280.0, 760.0];
    pub const BUTTON_HEIGHT: f32 = 36.0;
}

/// Font families: Inter for text in three weights, Phosphor for icons (outline and filled).
pub mod font {
    use eframe::egui::{FontFamily, FontId};

    pub const SEMIBOLD: &str = "inter-semibold";
    pub const BOLD: &str = "inter-bold";
    pub const ICONS: &str = "phosphor";
    pub const ICONS_FILL: &str = "phosphor-fill";

    pub fn regular(size: f32) -> FontId {
        FontId::proportional(size)
    }

    pub fn semibold(size: f32) -> FontId {
        FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
    }

    pub fn bold(size: f32) -> FontId {
        FontId::new(size, FontFamily::Name(BOLD.into()))
    }

    pub fn icon(size: f32, filled: bool) -> FontId {
        FontId::new(size, FontFamily::Name(if filled { ICONS_FILL } else { ICONS }.into()))
    }
}

pub fn radius(r: u8) -> CornerRadius {
    CornerRadius::same(r)
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

/// A lighter shade for hover.
pub fn lighten(color: Color32, t: f32) -> Color32 {
    mix(color, Color32::WHITE, t)
}

/// A darker shade for press and gradients.
pub fn darken(color: Color32, t: f32) -> Color32 {
    mix(color, Color32::BLACK, t)
}

fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let mut add = |name: &str, bytes: &'static [u8]| {
        fonts.font_data.insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    };
    add("inter-regular", include_bytes!("../assets/fonts/inter-regular.ttf"));
    add(font::SEMIBOLD, include_bytes!("../assets/fonts/inter-semibold.ttf"));
    add(font::BOLD, include_bytes!("../assets/fonts/inter-bold.ttf"));
    add(font::ICONS, include_bytes!("../assets/fonts/phosphor-regular.ttf"));
    add(font::ICONS_FILL, include_bytes!("../assets/fonts/phosphor-fill.ttf"));
    let fallback: Vec<String> = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let family = |first: &str, icons: &str| {
        let mut list = vec![first.to_owned(), icons.to_owned()];
        list.extend(fallback.iter().cloned());
        list
    };
    fonts.families.insert(FontFamily::Proportional, family("inter-regular", font::ICONS));
    fonts.families.insert(FontFamily::Name(font::SEMIBOLD.into()), family(font::SEMIBOLD, font::ICONS));
    fonts.families.insert(FontFamily::Name(font::BOLD.into()), family(font::BOLD, font::ICONS));
    fonts.families.insert(FontFamily::Name(font::ICONS.into()), vec![font::ICONS.to_owned()]);
    fonts.families.insert(FontFamily::Name(font::ICONS_FILL.into()), vec![font::ICONS_FILL.to_owned()]);
    fonts
}

pub fn apply(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    let mut visuals = Visuals::dark();
    visuals.panel_fill = color::BG;
    visuals.window_fill = color::SURFACE;
    visuals.window_stroke = Stroke::new(1.0, color::BORDER);
    visuals.window_corner_radius = radius(radius::L);
    visuals.menu_corner_radius = radius(radius::M);
    visuals.extreme_bg_color = color::SUNKEN;
    visuals.text_edit_bg_color = Some(color::SUNKEN);
    visuals.faint_bg_color = color::SURFACE;
    visuals.code_bg_color = color::SUNKEN;
    visuals.selection.bg_fill = color::ACCENT_STRONG;
    visuals.selection.stroke = Stroke::new(1.0, color::TEXT);
    visuals.hyperlink_color = color::ACCENT;
    visuals.warn_fg_color = color::WARN;
    visuals.error_fg_color = color::DANGER;
    visuals.override_text_color = None;
    let widgets = &mut visuals.widgets;
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, color::TEXT);
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, color::BORDER);
    widgets.noninteractive.bg_fill = color::SURFACE;
    widgets.inactive.fg_stroke = Stroke::new(1.0, color::TEXT);
    widgets.inactive.bg_fill = color::SURFACE_HOVER;
    widgets.inactive.weak_bg_fill = color::SURFACE_HOVER;
    widgets.inactive.bg_stroke = Stroke::NONE;
    widgets.hovered.bg_fill = color::SURFACE_ACTIVE;
    widgets.hovered.weak_bg_fill = color::SURFACE_ACTIVE;
    widgets.hovered.bg_stroke = Stroke::new(1.0, color::ACCENT);
    widgets.hovered.fg_stroke = Stroke::new(1.5, color::TEXT);
    widgets.active.bg_fill = color::ACCENT_STRONG;
    widgets.active.weak_bg_fill = color::ACCENT_STRONG;
    widgets.active.bg_stroke = Stroke::new(1.0, color::ACCENT);
    widgets.open.weak_bg_fill = color::SURFACE_ACTIVE;
    for widget in [
        &mut widgets.noninteractive,
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
        &mut widgets.open,
    ] {
        widget.corner_radius = radius(radius::S);
    }
    ctx.set_visuals_of(egui::Theme::Dark, visuals);
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.all_styles_mut(|style| {
        use egui::TextStyle;
        style.spacing.item_spacing = egui::vec2(space::S, space::S);
        style.spacing.button_padding = egui::vec2(space::M, 6.0);
        style.spacing.interact_size.y = 30.0;
        style.spacing.combo_height = 320.0;
        style.spacing.window_margin = egui::Margin::same(space::L as i8);
        style.spacing.menu_margin = egui::Margin::same(space::S as i8);
        style.animation_time = anim::HOVER;
        style.text_styles.insert(TextStyle::Heading, font::bold(size::HEADING));
        style.text_styles.insert(TextStyle::Body, font::regular(size::BODY));
        style.text_styles.insert(TextStyle::Button, font::regular(size::BODY));
        style.text_styles.insert(TextStyle::Small, font::regular(size::CAPTION));
        style.text_styles.insert(TextStyle::Monospace, FontId::monospace(size::SMALL));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_and_shades() {
        assert_eq!(mix(Color32::BLACK, Color32::WHITE, 0.5), Color32::from_rgb(128, 128, 128));
        assert_eq!(lighten(color::ACCENT, 0.0), color::ACCENT);
        assert_eq!(darken(color::ACCENT, 1.0), Color32::BLACK);
    }

    #[test]
    fn fonts_have_text_and_icon_families() {
        let fonts = fonts();
        for name in [font::SEMIBOLD, font::BOLD, font::ICONS, font::ICONS_FILL] {
            assert!(fonts.families.contains_key(&FontFamily::Name(name.into())), "{name}");
        }
        assert_eq!(fonts.families[&FontFamily::Proportional][0], "inter-regular");
    }
}
