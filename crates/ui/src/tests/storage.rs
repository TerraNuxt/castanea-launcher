//! Storage and reset in the window: sizes arrive from background jobs, deletions run on their own thread, nothing is
//! deleted while the game runs, and the game data question waits for the typed word.

use std::time::{Duration, Instant};

use castanea_launcher_core::storage::Row;
use eframe::egui;

use super::Harness;
use crate::app::{Confirm, NoteKind, Page};
use crate::pages::storage::{Dialog, dialog, dialog_body};
use crate::storage::{Deletion, bytes_text};

/// Draws until no background job is left.
fn settle(h: &mut Harness) {
    let start = Instant::now();
    loop {
        h.frame();
        if h.app.jobs.running == 0 && !h.app.storage.deleting {
            return;
        }
        assert!(start.elapsed() < Duration::from_secs(20), "a job never finished");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A launcher home with a cache, a log, saved settings and a world next to the harness's build and mods.
fn filled(language: &str) -> Harness {
    let mut h = Harness::new(language);
    let p = h.app.paths.clone();
    for (path, text) in [
        (p.cache.join("news/en.json"), "[]"),
        (p.cache.join("downloads/a.part"), "partial"),
        (p.game().join("logs/latest.log"), "log"),
        (p.game().join("saves/w1/level.dat"), "world"),
    ] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    h.app.save_settings();
    h
}

#[test]
fn sizes_are_measured_in_the_background_and_shown_when_they_arrive() {
    let mut h = filled("en");
    h.go(Page::Settings);
    h.app.measure_storage(&h.ctx);
    assert!(h.app.storage.info.is_empty(), "the call returned before any row was measured");
    assert!(h.app.jobs.running >= Row::ALL.len());
    settle(&mut h);
    let info = &h.app.storage.info;
    assert_eq!(info[&Row::Logs].size.bytes, 3);
    assert_eq!(info[&Row::GameData].size.bytes, 5);
    assert!(info[&Row::Mods].present && info[&Row::Game].present);
    assert!(info[&Row::Cache].size.bytes >= 9);
    h.draw();
}

#[test]
fn byte_sizes_read_short() {
    assert_eq!(bytes_text(0), "0 B");
    assert_eq!(bytes_text(1023), "1023 B");
    assert_eq!(bytes_text(1024), "1 KB");
    assert_eq!(bytes_text(1536), "2 KB");
    assert_eq!(bytes_text(5 << 20), "5.0 MB");
    assert_eq!(bytes_text(3 << 30), "3.0 GB");
}

#[test]
fn deleting_a_row_removes_its_files_and_refreshes_the_page() {
    let mut h = filled("en");
    h.go(Page::Settings);
    settle(&mut h);
    assert_eq!(h.app.mods.len(), 2);
    h.app.confirm = Some(Confirm::DeleteRow(Row::Mods));
    h.app.start_delete(&h.ctx, Deletion::Row(Row::Mods));
    assert!(h.app.storage.deleting, "it runs on its own thread");
    assert!(h.app.mods_dir().exists(), "and the call returned first");
    settle(&mut h);
    assert!(!h.app.mods_dir().exists());
    assert!(h.app.mods.is_empty());
    assert!(h.app.paths.build().exists() && h.app.paths.game().join("saves/w1/level.dat").exists());
    assert!(!h.app.storage.info[&Row::Mods].present);
    assert!(
        h.app.notes.iter().any(|n| n.kind == NoteKind::Info),
        "{:?}",
        h.app.notes.iter().map(|n| &n.text).collect::<Vec<_>>()
    );
}

#[test]
fn deleting_the_game_row_leaves_no_installed_game_and_no_new_download() {
    let mut h = filled("en");
    assert!(h.app.installed.is_some());
    h.app.start_delete(&h.ctx, Deletion::Row(Row::Game));
    settle(&mut h);
    assert!(h.app.installed.is_none());
    assert!(h.app.checked, "no automatic download right after deleting the game");
    assert!(!h.app.paths.build().exists());
}

#[test]
fn delete_everything_keeps_the_world_and_ends_with_the_goodbye_dialog() {
    let mut h = filled("en");
    h.app.start_delete(&h.ctx, Deletion::Everything);
    settle(&mut h);
    let p = &h.app.paths;
    for gone in [p.build(), p.mods(), p.settings_file(), p.cache.clone()] {
        let inside: Vec<_> = std::fs::read_dir(&gone).into_iter().flatten().flatten().map(|e| e.path()).collect();
        assert!(!gone.exists(), "{} is still there: {inside:?}", gone.display());
    }
    assert!(p.game().join("saves/w1/level.dat").exists());
    assert!(matches!(h.app.confirm, Some(Confirm::Goodbye)));
    assert!(h.app.storage.failures.is_empty());
    h.draw();
}

#[cfg(unix)]
#[test]
fn nothing_is_deleted_while_the_game_runs() {
    let mut h = filled("en");
    h.play_and_wait();
    assert!(h.app.is_running());
    assert_eq!(h.app.storage_blocked(), Some("storage.running"));
    h.app.start_delete(&h.ctx, Deletion::Everything);
    h.app.start_delete(&h.ctx, Deletion::Row(Row::Game));
    assert!(!h.app.storage.deleting, "no job was started");
    assert!(h.app.paths.build().exists() && h.app.paths.settings_file().exists());
    assert_eq!(h.app.notes.iter().filter(|n| n.kind == NoteKind::Error).count(), 2);
    h.go(Page::Settings);
    h.draw();
    h.app.stop();
    assert_eq!(h.app.storage_blocked(), None);
}

fn body_ready(h: &mut Harness, dialog: &Dialog, typed: &str) -> bool {
    let mut typed = typed.to_owned();
    let mut ready = false;
    let input =
        egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, h.size)), ..Default::default() };
    let mut output = h.ctx.run_ui(input, |ui| ready = dialog_body(&mut typed, &h.app.tr, ui, dialog));
    output.textures_delta.clear();
    ready
}

#[test]
fn the_game_data_question_needs_the_word_and_lists_what_goes() {
    let mut h = filled("en");
    let question = dialog(&h.app, &Confirm::DeleteGameData).expect("a storage question");
    assert!(question.type_word);
    let lines: Vec<&String> = question.groups.iter().flat_map(|(_, l)| l).collect();
    assert!(lines.iter().any(|l| l.ends_with("saves")), "{lines:?}");
    assert!(!lines.iter().any(|l| l.ends_with("mods") || l.ends_with("build")));
    for wrong in ["", "delete", "DELET", "DELETE it"] {
        assert!(!body_ready(&mut h, &question, wrong), "{wrong:?}");
    }
    assert!(body_ready(&mut h, &question, "DELETE"));
    let plain = dialog(&h.app, &Confirm::DeleteRow(Row::Logs)).unwrap();
    assert!(!plain.type_word && body_ready(&mut h, &plain, ""));
}

#[test]
fn the_delete_everything_question_lists_exactly_the_own_rows() {
    let h = filled("en");
    let question = dialog(&h.app, &Confirm::DeleteEverything).unwrap();
    let headings: Vec<&str> = question.groups.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(headings, ["Cache", "Game", "Mods", "Logs", "Launcher settings"]);
    let lines: Vec<&String> = question.groups.iter().flat_map(|(_, l)| l).collect();
    assert!(!lines.iter().any(|l| l.contains("saves")), "the game's data is not listed");
    assert!(question.outro.contains("stay"), "{}", question.outro);
    let german = filled("de");
    let question = dialog(&german.app, &Confirm::DeleteEverything).unwrap();
    assert_eq!(question.groups[0].0, "Zwischenspeicher");
}

#[test]
fn the_game_data_row_is_only_listed_when_the_game_folder_exists() {
    let mut h = Harness::new("en");
    h.go(Page::Settings);
    std::fs::remove_dir_all(h.app.paths.game()).unwrap();
    settle(&mut h);
    h.draw();
    assert!(!castanea_launcher_core::storage::knows_game_data(&h.app.paths));
}
