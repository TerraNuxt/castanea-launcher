//! The brand font in the UI: the launcher's own titles are spelled by it in both languages, headings keep their real
//! text for screen readers, glyphs land on whole physical pixels, and text it cannot spell falls back to Inter.

use eframe::egui::{self, Shape};

use super::Harness;
use crate::app::Page;
use crate::bmfont::{BitmapFont, BmText};
use crate::theme::size;

fn lang(code: &str) -> std::collections::HashMap<String, String> {
    let text = match code {
        "en" => include_str!("../../lang/en.json"),
        _ => include_str!("../../lang/de.json"),
    };
    serde_json::from_str(text).unwrap()
}

#[test]
fn every_brand_string_is_spelled_by_the_font_in_both_languages() {
    let font = BitmapFont::builtin();
    for code in ["en", "de"] {
        let tr = lang(code);
        for key in ["nav.mods", "nav.output", "nav.settings", "play.play", "play.stop"] {
            let text = &tr[key];
            assert!(font.covers(text), "{code} {key}: {text:?} lacks {:?}", font.missing(text));
        }
    }
    assert!(font.covers("castanea") && font.covers("castanea launcher"));
}

#[test]
fn text_the_font_cannot_spell_falls_back_whole() {
    assert!(BmText::new("Mods", size::HEADING).is_bitmap());
    assert!(!BmText::new("Über uns", size::HEADING).is_bitmap());
    assert!(!BmText::new("Play!", size::HEADING).is_bitmap());
}

#[test]
fn headings_keep_their_text_for_screen_readers() {
    for code in ["en", "de"] {
        let mut h = Harness::new(code);
        h.ctx.enable_accesskit();
        let tr = lang(code);
        let pages = [
            (Page::Home, "castanea".to_owned()),
            (Page::Mods, tr["nav.mods"].clone()),
            (Page::Log, tr["nav.output"].clone()),
            (Page::Settings, tr["nav.settings"].clone()),
            (Page::About, "castanea launcher".to_owned()),
        ];
        for (page, title) in pages {
            h.go(page);
            h.draw();
            let nodes = h.frame().platform_output.accesskit_update.unwrap().nodes;
            let found = nodes.iter().any(|(_, n)| n.role() == egui::Role::Heading && n.label() == Some(title.as_str()));
            assert!(found, "{code} {page:?}: no heading named {title:?}");
        }
    }
}

#[test]
fn a_fallback_heading_still_carries_the_real_text() {
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    ctx.enable_accesskit();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 200.0))),
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| {
        BmText::new("Über uns", size::HEADING).show(ui);
    });
    output.textures_delta.clear();
    let nodes = output.platform_output.accesskit_update.unwrap().nodes;
    assert!(nodes.iter().any(|(_, n)| n.label() == Some("Über uns")));
}

/// The vertices of every mesh a frame drew, in points.
fn meshes(output: &egui::FullOutput) -> Vec<Vec<egui::epaint::Vertex>> {
    output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            Shape::Mesh(mesh) => Some(mesh.vertices.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn glyphs_sit_on_whole_physical_pixels_at_any_scale() {
    for ppp in [1.0f32, 1.25, 1.5, 2.0] {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        ctx.set_pixels_per_point(ppp);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 300.0))),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            ui.add_space(3.3);
            BmText::new("castanea 2026", size::BRAND).show(ui);
        });
        output.textures_delta.clear();
        let all = meshes(&output);
        assert_eq!(all.len(), 1, "one mesh for one string");
        let vertices = &all[0];
        assert_eq!(vertices.len(), 4 * "castanea2026".len(), "four vertices per glyph, no spaces");
        for quad in vertices.chunks(4) {
            let (x, y) = (quad[0].pos.x * ppp, quad[0].pos.y * ppp);
            assert!((x - x.round()).abs() < 1e-3 && (y - y.round()).abs() < 1e-3, "ppp {ppp}: corner at {x},{y}");
        }
    }
}

#[test]
fn small_text_uses_a_smaller_level_than_large_text() {
    let ctx = egui::Context::default();
    let draw = |px: f32| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 300.0))),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            BmText::new("MODS", px).show(ui);
        });
        output.textures_delta.clear();
        let quad = &meshes(&output)[0][..4];
        let (u0, u1) = (quad[0].uv, quad[1].uv);
        // The UV span in texels says how far the level was shrunk against the screen size.
        (u1.x - u0.x, quad[1].pos.x - quad[0].pos.x)
    };
    let (small_uv, small_px) = draw(16.0);
    let (large_uv, large_px) = draw(120.0);
    assert!(small_px < large_px);
    assert!(small_uv > 0.0 && large_uv > 0.0);
}

#[test]
fn the_wordmark_is_a_bitmap_text_of_the_expected_size() {
    let font = BitmapFont::builtin();
    let line = size::BRAND;
    let width = font.measure("castanea", line);
    let ctx = egui::Context::default();
    assert!((BmText::new("castanea", line).measure(&ctx).x - width).abs() < 1e-3);
    let (top, bottom) = font.cap_box();
    let height = BmText::new("castanea", line).measure(&ctx).y;
    assert!((height - (bottom - top) * line / font.line_height as f32).abs() < 1e-3);
}

#[test]
fn wrapping_makes_a_taller_narrower_box() {
    let ctx = egui::Context::default();
    let one = BmText::new("MODS AND MORE MODS", size::HEADING).measure(&ctx);
    let wrapped = BmText::new("MODS AND MORE MODS", size::HEADING).wrap(one.x * 0.5).measure(&ctx);
    assert!(wrapped.y > one.y && wrapped.x < one.x, "{one:?} {wrapped:?}");
}
