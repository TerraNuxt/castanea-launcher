//! Headless UI checks: every page draws at three window sizes in both languages, layout regions do not overlap,
//! every control has a name for screen readers, an idle window asks for no repaint, the update states read right,
//! and every string key exists and is used.

mod brand;
mod launcher;
mod layout;
mod name;
mod news;
#[cfg(unix)]
mod on_start;
mod storage;

use std::fs;
use std::path::Path;

use castanea_launcher_core::install::{Build, local, update};
use castanea_launcher_core::paths::Paths;
use eframe::egui;

use crate::app::{App, Confirm, ModsView, NoteKind, Page};
use crate::game::Update;
use crate::jobs::{self, Progress};
use crate::news::Phase;
use crate::shell;

pub type View = (&'static str, fn(&mut App));

pub const SIZES: [[f32; 2]; 3] = [[1000.0, 640.0], [1280.0, 720.0], [1600.0, 900.0]];
/// The home page's sizes: the small window below the minimum (news below the Play block), the common ones and a big one.
pub const HOME_SIZES: [[f32; 2]; 5] =
    [[900.0, 600.0], [1000.0, 640.0], [1280.0, 720.0], [1600.0, 900.0], [1920.0, 1080.0]];

pub struct Harness {
    pub app: App,
    pub ctx: egui::Context,
    pub size: egui::Vec2,
    time: f64,
    _dir: tempfile::TempDir,
}

/// An installed release in `paths`, as an update would leave it.
pub fn install_build(paths: &Paths, script: &str) {
    let root = update::current_dir(paths);
    fs::create_dir_all(root.join("assets")).unwrap();
    let binary = root.join(local::BINARY);
    fs::write(&binary, format!("#!/bin/sh\n{script}\n")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let build = Build { version: "0.2.0".into(), root, binary, ..Build::default() };
    build.write().unwrap();
}

impl Harness {
    /// A launcher home with an installed build, two mods (one with a missing dependency) and the example index,
    /// offline.
    pub fn new(language: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::portable(dir.path());
        install_build(&paths, "echo started; exec sleep 30");
        let mods = paths.mods();
        fs::create_dir_all(mods.join("amber")).unwrap();
        fs::write(
            mods.join("amber/mod.json"),
            r#"{"id":"amber","name":"Amber","version":"0.1.0","depends":["frost"]}"#,
        )
        .unwrap();
        fs::create_dir_all(mods.join(".disabled/redmoon")).unwrap();
        fs::write(mods.join(".disabled/redmoon/mod.json"), r#"{"id":"redmoon","name":"Red moon","version":"0.1.0"}"#)
            .unwrap();
        let index = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/index.json");
        let mut app = App::new(paths, None);
        app.settings.indexes.push(index.display().to_string());
        app.settings.offline = true;
        app.settings.language = language.to_owned();
        app.reload_language();
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

    /// Draws until the layout is stable.
    pub fn draw(&mut self) -> egui::FullOutput {
        self.frame();
        self.frame();
        self.frame()
    }

    fn load_index(&mut self) {
        self.app.go(Page::Mods);
        self.app.mods_view = ModsView::Browse;
        for _ in 0..300 {
            self.frame();
            if self.app.index_loaded && self.app.jobs.running == 0 && !self.app.index.mods.is_empty() {
                self.app.go(Page::Home);
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("the example index did not load: {:?}", self.app.index_warnings);
    }

    pub fn go(&mut self, page: Page) {
        self.app.go(page);
    }

    #[cfg(unix)]
    /// Presses Play and draws until the backend job is done and the game runs (or the start failed).
    pub fn play_and_wait(&mut self) {
        self.app.play(&self.ctx);
        for _ in 0..500 {
            self.frame();
            if !self.app.starting {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("the start never finished");
    }

    /// Every view the tests draw: each page, the update states of the home page, the mod browser with a selection,
    /// the confirm dialog and the open Advanced settings.
    pub fn views() -> Vec<View> {
        vec![
            ("home", |app| {
                app.update = Update::Offline;
                app.go(Page::Home);
            }),
            ("home-news", |app| {
                app.update = Update::UpToDate;
                app.show_demo_news(Phase::Fresh);
                app.installed.as_mut().unwrap().notes = "• Smelteries\n• Steam machines\n• Faster loading".into();
                app.go(Page::Home);
            }),
            ("home-news-offline", |app| {
                app.show_demo_news(Phase::Offline);
                app.news.open.insert("steam-age".to_owned());
                app.go(Page::Home);
            }),
            ("home-news-empty", |app| {
                app.news.items.clear();
                app.news.phase = Phase::Failed;
                app.installed.as_mut().unwrap().notes.clear();
                app.go(Page::Home);
            }),
            ("home-update", |app| {
                app.update = Update::Downloading { version: "0.3.0".into() };
                jobs::set(
                    &app.update_progress,
                    Some(Progress { label: "0.3.0".into(), done: 43, total: Some(100), speed: Some(5_200_000) }),
                );
                app.go(Page::Home);
            }),
            ("home-failed", |app| {
                jobs::set(&app.update_progress, None);
                app.update = Update::Failed("HTTP 404".into());
                app.go(Page::Home);
            }),
            ("mods-installed", |app| {
                app.update = Update::Offline;
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
            ("storage-row-dialog", |app| {
                app.go(Page::Settings);
                app.confirm = Some(Confirm::DeleteRow(castanea_launcher_core::storage::Row::Mods));
            }),
            ("storage-game-data-dialog", |app| {
                app.go(Page::Settings);
                app.confirm = Some(Confirm::DeleteGameData);
            }),
            ("storage-everything-dialog", |app| {
                app.go(Page::Settings);
                app.confirm = Some(Confirm::DeleteEverything);
            }),
            ("log", |app| {
                app.confirm = None;
                app.go(Page::Log);
            }),
            ("settings", |app| {
                app.advanced_open = true;
                app.go(Page::Settings);
            }),
            ("about", |app| app.go(Page::About)),
        ]
    }
}

fn errors(app: &App) -> Vec<&str> {
    app.notes.iter().filter(|n| n.kind == NoteKind::Error).map(|n| n.text.as_str()).collect()
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
                assert!(errors(&h.app).is_empty(), "{language} {size:?} {name}: {:?}", errors(&h.app));
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
        let roles =
            [egui::Role::Button, egui::Role::CheckBox, egui::Role::Tab, egui::Role::RadioButton, egui::Role::Link];
        let mut unnamed = Vec::new();
        let mut controls = 0;
        for (id, node) in &update.nodes {
            if roles.contains(&node.role()) {
                controls += 1;
                if node.label().is_none_or(|l| l.trim().is_empty()) && node.value().is_none() {
                    unnamed.push(format!("{id:?} {:?}", node.role()));
                }
            }
        }
        assert!(controls >= 5, "{name}: only {controls} controls");
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
        // The sizes of the storage rows are measured on first sight; wait for those jobs.
        for _ in 0..400 {
            h.frame();
            if h.app.jobs.running == 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
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
    let keys = [egui::Key::Num1, egui::Key::Num2, egui::Key::Num3, egui::Key::Num4, egui::Key::Num5];
    for (n, key_code) in keys.into_iter().enumerate() {
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
fn the_status_line_reads_every_update_state_in_both_languages() {
    for language in ["en", "de"] {
        let mut h = Harness::new(language);
        let states = [
            Update::Idle,
            Update::Checking,
            Update::UpToDate,
            Update::Downloading { version: "0.3.0".into() },
            Update::Ready { version: "0.3.0".into() },
            Update::Applying { version: "0.3.0".into() },
            Update::NoRelease,
            Update::Unsupported("no build".into()),
            Update::Offline,
            Update::Failed("x".into()),
        ];
        for state in states {
            h.app.update = state.clone();
            let (text, _) = crate::pages::home::status_line(&h.app);
            assert!(text.contains("0.2.0"), "{language} {state:?}: {text}");
            assert!(!text.contains("home."), "{language} {state:?}: a raw key in {text}");
            assert_eq!(h.app.can_play(), !matches!(state, Update::Applying { .. }), "{state:?}");
        }
        h.app.installed = None;
        h.app.update = Update::Offline;
        assert!(!h.app.can_play(), "nothing to play");
        let (text, _) = crate::pages::home::status_line(&h.app);
        assert_eq!(text, h.app.tr.get("home.offline_none"));
    }
}

#[test]
fn offline_mode_skips_the_update_check() {
    let mut h = Harness::new("en");
    h.app.checked = false;
    h.app.update = Update::Idle;
    h.draw();
    assert!(h.app.checked);
    assert_eq!(h.app.update, Update::Offline);
    assert_eq!(h.app.jobs.running, 0, "no request went out");
}

#[test]
fn a_local_build_folder_replaces_the_release_and_turns_updates_off() {
    let mut h = Harness::new("en");
    let folder = h._dir.path().join("checkout");
    fs::create_dir_all(folder.join("assets")).unwrap();
    fs::create_dir_all(folder.join("target/release")).unwrap();
    fs::write(folder.join("target/release").join(local::BINARY), "bin").unwrap();
    fs::write(folder.join("version.txt"), "0.9.0-dev").unwrap();
    h.app.settings.dev_build = Some(folder);
    h.app.refresh_installed();
    h.app.update = Update::Idle;
    h.app.check_updates(&h.ctx.clone());
    assert_eq!(h.app.update, Update::Local);
    assert_eq!(h.app.game_version().as_deref(), Some("0.9.0-dev"));
    let (text, _) = crate::pages::home::status_line(&h.app);
    assert!(text.contains("0.9.0-dev") && text.contains("Local"), "{text}");
}

#[test]
fn an_old_home_with_profiles_is_migrated_with_a_note() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    fs::create_dir_all(&paths.config).unwrap();
    fs::write(
        paths.config.join("profiles.json"),
        r#"{"profiles":[{"id":"default","name":"Survival","player_name":"Anna"}]}"#,
    )
    .unwrap();
    fs::create_dir_all(paths.data.join("instances/default/saves/lake")).unwrap();
    let app = App::new(paths.clone(), None);
    assert_eq!(app.settings.game.player_name, "Anna");
    assert!(paths.game().join("saves/lake").is_dir());
    assert!(app.notes.iter().any(|n| n.kind == NoteKind::Info && n.text.contains("Survival")));
    assert!(errors(&app).is_empty(), "{:?}", errors(&app));
}

#[cfg(unix)]
#[test]
fn play_becomes_stop_with_the_running_time_and_the_output_shows_the_game() {
    let mut h = Harness::new("en");
    h.ctx.enable_accesskit();
    h.app.update = Update::Offline;
    h.go(Page::Home);
    h.play_and_wait();
    assert!(h.app.is_running(), "{:?}", errors(&h.app));
    h.draw();
    let output = h.frame();
    let nodes = output.platform_output.accesskit_update.unwrap().nodes;
    let stop = h.app.tr.fill("play.stop_running", &[("time", "0:00")]);
    assert!(nodes.iter().any(|(_, n)| n.label() == Some(stop.as_str())), "a Stop button named {stop}");
    h.go(Page::Log);
    for _ in 0..100 {
        h.frame();
        if h.app.session.as_ref().is_some_and(|s| s.log.iter().any(|l| l == "started")) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(h.app.session.as_ref().unwrap().log.contains(&"started".to_owned()));
    h.app.stop();
    h.draw();
    assert!(!h.app.is_running());
    assert!(h.app.session.as_ref().unwrap().outcome.as_ref().unwrap().crashed, "killed counts as a crash");
}

/// Every `"group.key"` literal in the sources.
fn keys_in_code() -> Vec<(String, String)> {
    let prefixes = [
        "nav.",
        "common.",
        "status.",
        "play.",
        "home.",
        "notes.",
        "log.",
        "mods.",
        "problem.",
        "compat.",
        "browse.",
        "settings.",
        "about.",
        "launcher.",
        "backend.",
        "news.",
        "hint.",
        "storage.",
    ];
    let mut out = Vec::new();
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
                if is_key {
                    out.push((path.display().to_string(), piece.to_owned()));
                }
            }
        }
    }
    out
}

#[test]
fn every_string_key_exists_and_every_key_is_used() {
    let en: std::collections::HashMap<String, String> = serde_json::from_str(include_str!("../lang/en.json")).unwrap();
    let used = keys_in_code();
    let missing: Vec<String> = used
        .iter()
        .filter(|(_, k)| !en.contains_key(k) && k != "no.such.key")
        .map(|(f, k)| format!("{f}: {k}"))
        .collect();
    assert!(missing.is_empty(), "{missing:#?}");
    let mut unused: Vec<&String> = en.keys().filter(|k| !used.iter().any(|(_, u)| u == *k)).collect();
    unused.sort();
    assert!(unused.is_empty(), "keys no code uses: {unused:?}");
}

#[test]
fn whats_new_shows_the_notes_of_the_new_or_the_installed_build_and_the_download_speed() {
    let mut h = Harness::new("en");
    assert!(h.app.whats_new().is_none(), "no notes, no link");
    h.app.installed.as_mut().unwrap().notes = "• Faster".into();
    assert_eq!(h.app.whats_new(), Some(("0.2.0".into(), "• Faster".into())));
    h.app.pending_notes = Some(("0.3.0".into(), "• Newer".into()));
    h.app.update = Update::Downloading { version: "0.3.0".into() };
    assert_eq!(h.app.whats_new(), Some(("0.3.0".into(), "• Newer".into())));
    let progress = Progress { label: "0.3.0".into(), done: 43, total: Some(100), speed: Some(5_200_000) };
    jobs::set(&h.app.update_progress, Some(progress));
    let (text, _) = crate::pages::home::status_line(&h.app);
    assert!(text.contains("43 %") && text.contains("5.2 MB/s"), "{text}");
    h.app.show_notes = true;
    h.go(Page::Home);
    h.draw();
    assert!(h.app.show_notes, "the dialog stays open until it is closed");
}
