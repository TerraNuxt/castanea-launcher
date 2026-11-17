//! Headless UI checks: every page draws at several window sizes in both languages, layout regions do not overlap,
//! every control has a name for screen readers, an idle window asks for no repaint, and every string key exists.

mod layout;

use std::fs;
use std::path::Path;

use castanea_launcher_core::install::{Origin, local};
use castanea_launcher_core::paths::Paths;
use eframe::egui;

use crate::app::{App, Confirm, ModsView, NoteKind, Page};
use crate::shell;

pub type View = (&'static str, fn(&mut App));

pub const SIZES: [[f32; 2]; 4] = [[1000.0, 640.0], [1280.0, 720.0], [1600.0, 900.0], [2200.0, 1300.0]];

pub struct Harness {
    pub app: App,
    pub ctx: egui::Context,
    pub size: egui::Vec2,
    time: f64,
    _dir: tempfile::TempDir,
}

impl Harness {
    /// A launcher home with a game build, two profiles, two mods (one with a missing dependency) and the example index.
    pub fn new(language: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::portable(dir.path());
        let game = dir.path().join("game");
        fs::create_dir_all(game.join("assets")).unwrap();
        fs::write(game.join(local::BINARY), "").unwrap();
        fs::write(game.join("version.txt"), "0.2.0").unwrap();
        let index = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/index.json");
        let mut app = App::new(paths);
        let build = local::inspect(&app.registry, &game, Origin::Local).unwrap();
        app.registry.put(build);
        app.settings.indexes.push(index.display().to_string());
        app.settings.offline = true;
        app.settings.language = language.to_owned();
        app.reload_language();
        app.profile_mut().player_name = "Anna".to_owned();
        let id = app.profiles.add("Creative test with a rather long profile name that has to be cut");
        app.profiles.get_mut(&id).unwrap().last_played = Some(crate::app::now_seconds() - 7200);
        let mods = app.mods_dir();
        fs::create_dir_all(mods.join("amber")).unwrap();
        fs::write(
            mods.join("amber/mod.json"),
            r#"{"id":"amber","name":"Amber","version":"0.1.0","depends":["frost"]}"#,
        )
        .unwrap();
        fs::create_dir_all(mods.join(".disabled/redmoon")).unwrap();
        fs::write(mods.join(".disabled/redmoon/mod.json"), r#"{"id":"redmoon","name":"Red moon","version":"0.1.0"}"#)
            .unwrap();
        app.reload_mods();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut harness = Self { app, ctx, size: egui::vec2(1280.0, 720.0), time: 0.0, _dir: dir };
        harness.load_index();
        harness
    }

    pub fn frame_with(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.time += 0.5;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut output = self.ctx.run_ui(input, |ui| shell::ui(app, ui));
        output.textures_delta.clear();
        output
    }

    pub fn frame(&mut self) -> egui::FullOutput {
        self.frame_with(Vec::new())
    }

    /// Draws until the page has faded in and the layout is stable.
    pub fn draw(&mut self) -> egui::FullOutput {
        self.frame();
        self.frame();
        self.frame()
    }

    fn load_index(&mut self) {
        for _ in 0..300 {
            self.frame();
            if self.app.index_loaded && self.app.jobs.running == 0 && !self.app.index.mods.is_empty() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("the example index did not load: {:?}", self.app.index_warnings);
    }

    pub fn go(&mut self, page: Page) {
        self.app.go(page);
    }

    /// Every view the tests draw: each page, the mod browser with a selection, and the confirm dialog.
    pub fn views() -> Vec<View> {
        vec![
            ("home", |app| app.go(Page::Home)),
            ("mods-installed", |app| {
                app.mods_view = ModsView::Installed;
                app.go(Page::Mods);
            }),
            ("mods-browse", |app| {
                app.mods_view = ModsView::Browse;
                app.browse.selected = Some("amber".to_owned());
                app.go(Page::Mods);
            }),
            ("mods-dialog", |app| {
                app.mods_view = ModsView::Installed;
                app.go(Page::Mods);
                app.confirm = Some(Confirm::EnableMod { file_name: "redmoon".to_owned() });
            }),
            ("profiles", |app| {
                app.confirm = None;
                app.go(Page::Profiles);
            }),
            ("versions", |app| app.go(Page::Versions)),
            ("log", |app| app.go(Page::Log)),
            ("notifications", |app| app.go(Page::Notifications)),
            ("settings", |app| app.go(Page::Settings)),
            ("about", |app| app.go(Page::About)),
        ]
    }
}

#[test]
fn every_page_draws_at_every_size_in_both_languages() {
    for language in ["en", "de"] {
        let mut h = Harness::new(language);
        assert_eq!(h.app.index.mods.len(), 3, "the example index loaded: {:?}", h.app.index_warnings);
        for size in SIZES {
            h.size = egui::vec2(size[0], size[1]);
            for (name, setup) in Harness::views() {
                setup(&mut h.app);
                h.draw();
                let errors: Vec<&str> =
                    h.app.notes.iter().filter(|n| n.kind == NoteKind::Error).map(|n| n.text.as_str()).collect();
                assert!(errors.is_empty(), "{language} {size:?} {name}: {errors:?}");
            }
        }
    }
}

#[test]
fn every_control_has_a_name_for_screen_readers() {
    let mut h = Harness::new("en");
    h.ctx.enable_accesskit();
    for (name, setup) in Harness::views() {
        setup(&mut h.app);
        h.draw();
        let output = h.frame();
        let update = output.platform_output.accesskit_update.expect("accesskit is on");
        let roles = [egui::Role::Button, egui::Role::CheckBox, egui::Role::Tab, egui::Role::RadioButton];
        let mut unnamed = Vec::new();
        let mut buttons = 0;
        for (id, node) in &update.nodes {
            if roles.contains(&node.role()) {
                buttons += 1;
                if node.label().is_none_or(|l| l.trim().is_empty()) && node.value().is_none() {
                    unnamed.push(format!("{id:?} {:?}", node.role()));
                }
            }
        }
        assert!(buttons > 5, "{name}: only {buttons} controls");
        assert!(unnamed.is_empty(), "{name}: controls without a name: {unnamed:?}");
    }
}

#[test]
fn an_idle_window_asks_for_no_repaint() {
    let mut h = Harness::new("en");
    for (name, setup) in Harness::views() {
        setup(&mut h.app);
        h.app.confirm = None;
        h.app.notes.clear();
        h.draw();
        h.draw();
        let output = h.frame();
        let delay = output.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
        assert!(delay > std::time::Duration::from_secs(3600), "{name}: repaints after {delay:?} while idle");
    }
}

#[test]
fn keyboard_opens_pages_and_moves_focus() {
    let mut h = Harness::new("en");
    h.go(Page::Home);
    h.draw();
    let key = |key, modifiers| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers };
    for (n, key_code) in [egui::Key::Num1, egui::Key::Num2, egui::Key::Num3, egui::Key::Num4].into_iter().enumerate() {
        h.frame_with(vec![key(key_code, egui::Modifiers::COMMAND)]);
        assert_eq!(h.app.page, Page::ALL[n]);
    }
    h.frame_with(vec![key(egui::Key::F, egui::Modifiers::COMMAND)]);
    assert_eq!((h.app.page, h.app.mods_view), (Page::Mods, ModsView::Browse));
    h.draw();
    assert!(h.ctx.memory(|m| m.focused().is_some()), "Ctrl+F focuses the search field");
    h.go(Page::Home);
    h.draw();
    h.frame_with(vec![key(egui::Key::Tab, egui::Modifiers::NONE)]);
    h.frame();
    assert!(h.ctx.memory(|m| m.focused().is_some()), "Tab focuses a control");
}

#[test]
fn every_string_key_in_the_code_exists() {
    let en: std::collections::HashMap<String, String> = serde_json::from_str(include_str!("../lang/en.json")).unwrap();
    let prefixes = [
        "nav.",
        "window.",
        "home.",
        "notes.",
        "log.",
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
