//! Headless UI checks: every tab draws without a window, and every string key the code uses exists.

use std::fs;
use std::path::Path;

use castanea_launcher_core::paths::Paths;
use eframe::egui;

use crate::app::{App, ModsView, Tab};
use crate::shell;

fn draw(app: &mut App, ctx: &egui::Context, frames: usize) {
    for _ in 0..frames {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1080.0, 700.0))),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| shell::ui(app, ui));
        output.textures_delta.clear();
    }
}

#[test]
fn every_tab_draws_headless_with_an_index_and_mods() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    let index = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/index.json");
    let mut app = App::new(paths.clone());
    app.settings.indexes.push(index.display().to_string());
    app.settings.offline = true;
    let mods = app.mods_dir();
    fs::create_dir_all(mods.join("amber")).unwrap();
    fs::write(mods.join("amber/mod.json"), r#"{"id":"amber","name":"Amber","version":"0.1.0","depends":["frost"]}"#)
        .unwrap();
    app.reload_mods();
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    for tab in Tab::ALL {
        app.tab = tab;
        draw(&mut app, &ctx, 3);
    }
    for _ in 0..200 {
        if app.jobs.running == 0 && !app.index.mods.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        draw(&mut app, &ctx, 1);
    }
    assert_eq!(app.index.mods.len(), 3, "the example index loaded: {:?}", app.index_warnings);
    app.tab = Tab::Mods;
    app.mods_view = ModsView::Browse;
    app.browse.selected = Some("amber".to_owned());
    draw(&mut app, &ctx, 3);
    app.editing_profile = true;
    app.tab = Tab::Play;
    draw(&mut app, &ctx, 3);
    assert!(app.notes.is_empty() || app.notes.iter().all(|n| matches!(n, crate::app::Note::Info(_))));
}

#[test]
fn every_string_key_in_the_code_exists() {
    let en: std::collections::HashMap<String, String> = serde_json::from_str(include_str!("../lang/en.json")).unwrap();
    let prefixes = [
        "tab.",
        "play.",
        "profile.",
        "mods.",
        "browse.",
        "versions.",
        "settings.",
        "about.",
        "common.",
        "status.",
        "problem.",
        "compat.",
    ];
    let mut missing = Vec::new();
    let mut stack = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let text = fs::read_to_string(&path).unwrap();
            for piece in text.split('"').skip(1).step_by(2) {
                let is_key = prefixes.iter().any(|p| piece.starts_with(p) && piece.len() > p.len())
                    && piece.chars().all(|c| c.is_ascii_lowercase() || c == '.' || c == '_');
                if is_key && !en.contains_key(piece) && piece != "no.such.key" {
                    missing.push(format!("{}: {piece}", path.display()));
                }
            }
        }
    }
    assert!(missing.is_empty(), "{missing:#?}");
}
