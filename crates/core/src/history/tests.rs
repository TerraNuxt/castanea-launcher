use std::fs;
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use super::*;

const MIN: i64 = 60_000;
const T0: i64 = 1_700_000_000_000;

/// A world folder whose newest file was changed at `at` milliseconds.
fn world(saves: &Path, name: &str, at: i64) {
    let dir = saves.join(name);
    fs::create_dir_all(dir.join("region")).unwrap();
    for file in [dir.join(WORLD_META), dir.join("region/r.0.0.dat")] {
        fs::write(&file, "x").unwrap();
        let time = UNIX_EPOCH + Duration::from_millis(u64::try_from(at).unwrap());
        fs::File::options().write(true).open(&file).unwrap().set_modified(time).unwrap();
    }
}

fn entry(started: i64, ended: Option<i64>, start: Start) -> Entry {
    Entry { started, ended, version: "0.2.0".into(), start }
}

fn setup() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("game");
    fs::create_dir_all(game.join(SAVES)).unwrap();
    (dir, game)
}

#[test]
fn nothing_played_is_nothing() {
    let (_dir, game) = setup();
    assert_eq!(last_played(&History::default(), &game, T0), None);
    assert_eq!(last_played(&History::default(), &game.join("missing"), T0), None);
}

#[test]
fn without_a_record_the_world_the_game_saw_played_last_is_used() {
    let (_dir, game) = setup();
    let saves = game.join(SAVES);
    world(&saves, "old", T0 - 9 * MIN);
    world(&saves, "recent", T0 - MIN);
    world(&saves, "middle", T0 - 5 * MIN);
    let played = last_played(&History::default(), &game, T0).unwrap();
    assert_eq!(
        (played.start, played.label.as_str(), played.when_ms),
        (Start::World("recent".into()), "recent", T0 - MIN)
    );
    assert_eq!((played.version, played.window), (None, None));
}

#[test]
fn only_real_worlds_count() {
    let (_dir, game) = setup();
    let saves = game.join(SAVES);
    world(&saves, "real", T0 - 100 * MIN);
    world(&saves, "bad name", T0);
    world(&saves, "-dash", T0);
    fs::create_dir_all(saves.join("no_meta")).unwrap();
    fs::write(saves.join("no_meta/file"), "x").unwrap();
    fs::write(saves.join("a_file"), "x").unwrap();
    assert_eq!(worlds(&saves).iter().map(|w| w.0.as_str()).collect::<Vec<_>>(), ["real"]);
}

#[test]
fn a_world_start_is_the_last_played_and_its_picture_is_found() {
    let (_dir, game) = setup();
    let saves = game.join(SAVES);
    world(&saves, "lake", T0 + 10 * MIN);
    world(&saves, "other", T0 + 50 * MIN);
    let picture = saves.join("lake").join(PREVIEW);
    fs::write(&picture, "png").unwrap();
    let time = UNIX_EPOCH + Duration::from_millis(u64::try_from(T0 + 3 * MIN).unwrap());
    fs::File::options().write(true).open(&picture).unwrap().set_modified(time).unwrap();
    let history = History { entries: vec![entry(T0, Some(T0 + 12 * MIN), Start::World("lake".into()))] };
    let played = last_played(&history, &game, T0 + 20 * MIN).unwrap();
    assert_eq!(played.start, Start::World("lake".into()));
    assert_eq!(played.when_ms, T0 + 10 * MIN, "the world's last change while it was played");
    assert_eq!(played.version.as_deref(), Some("0.2.0"));
    assert_eq!(played.window, Some((T0, T0 + 12 * MIN)));
    assert_eq!(played.preview, Some(saves.join("lake").join(PREVIEW)));
}

#[test]
fn a_world_that_was_deleted_is_skipped_for_the_one_before() {
    let (_dir, game) = setup();
    let saves = game.join(SAVES);
    world(&saves, "kept", T0 + MIN);
    let history = History {
        entries: vec![
            entry(T0, Some(T0 + 5 * MIN), Start::World("kept".into())),
            entry(T0 + 10 * MIN, Some(T0 + 11 * MIN), Start::World("gone".into())),
        ],
    };
    let played = last_played(&history, &game, T0 + 20 * MIN).unwrap();
    assert_eq!(played.start, Start::World("kept".into()));
}

#[test]
fn a_menu_start_is_resolved_to_the_world_changed_while_it_ran() {
    let (_dir, game) = setup();
    let saves = game.join(SAVES);
    world(&saves, "before", T0 - 30 * MIN);
    world(&saves, "played", T0 + 7 * MIN);
    world(&saves, "after", T0 + 40 * MIN);
    let history = History { entries: vec![entry(T0, Some(T0 + 20 * MIN), Start::Menu)] };
    let played = last_played(&history, &game, T0 + 60 * MIN).unwrap();
    assert_eq!(played.start, Start::World("played".into()));
    assert_eq!(played.when_ms, T0 + 7 * MIN);
    assert_eq!(played.window, Some((T0, T0 + 20 * MIN)));
}

#[test]
fn a_menu_start_in_which_nothing_was_played_falls_back_to_the_start_before_it() {
    let (_dir, game) = setup();
    let saves = game.join(SAVES);
    world(&saves, "lake", T0 + 2 * MIN);
    let history = History {
        entries: vec![
            entry(T0, Some(T0 + 5 * MIN), Start::Menu),
            entry(T0 + 100 * MIN, Some(T0 + 101 * MIN), Start::Menu),
        ],
    };
    let played = last_played(&history, &game, T0 + 200 * MIN).unwrap();
    assert_eq!(played.start, Start::World("lake".into()));
    assert_eq!(played.window, Some((T0, T0 + 5 * MIN)), "the older session that did play it");
}

#[test]
fn a_start_without_an_end_runs_until_the_next_start() {
    let (_dir, game) = setup();
    let saves = game.join(SAVES);
    world(&saves, "first", T0 + 10 * MIN);
    world(&saves, "second", T0 + 100 * MIN);
    // The launcher was closed during the first run, so it has no end; the second start closes its window.
    let history = History {
        entries: vec![entry(T0, None, Start::Menu), entry(T0 + 90 * MIN, Some(T0 + 110 * MIN), Start::Menu)],
    };
    assert_eq!(last_played(&history, &game, T0 + 200 * MIN).unwrap().start, Start::World("second".into()));
    let one = History { entries: vec![entry(T0, None, Start::Menu)] };
    assert_eq!(last_played(&one, &game, T0 + 20 * MIN).unwrap().start, Start::World("first".into()));
}

#[test]
fn a_server_is_remembered_by_its_saved_name_or_its_address() {
    let (_dir, game) = setup();
    fs::write(game.join("servers.txt"), "Friends\t10.0.0.5:47800\nBroken line\nLAN\t192.168.1.2:47800\n").unwrap();
    let history = History { entries: vec![entry(T0, Some(T0 + 30 * MIN), Start::Join("10.0.0.5:47800".into()))] };
    let played = last_played(&history, &game, T0 + 60 * MIN).unwrap();
    assert_eq!((played.start.clone(), played.label.as_str()), (Start::Join("10.0.0.5:47800".into()), "Friends"));
    assert_eq!(played.when_ms, T0 + 30 * MIN);
    assert_eq!(played.preview, None);
    let unsaved = History { entries: vec![entry(T0, None, Start::Join("AB12CD".into()))] };
    let played = last_played(&unsaved, &game, T0 + 60 * MIN).unwrap();
    assert_eq!((played.label.as_str(), played.when_ms), ("AB12CD", T0));
    assert_eq!(server_name(&game, "nope"), None);
    assert_eq!(server_name(&game.join("none"), "x"), None);
}

#[test]
fn the_newest_start_wins_whatever_it_was() {
    let (_dir, game) = setup();
    let saves = game.join(SAVES);
    world(&saves, "lake", T0 + 500 * MIN);
    let history = History {
        entries: vec![
            entry(T0, Some(T0 + MIN), Start::World("lake".into())),
            entry(T0 + 10 * MIN, Some(T0 + 20 * MIN), Start::Join("host:1".into())),
        ],
    };
    assert_eq!(last_played(&history, &game, T0 + 600 * MIN).unwrap().start, Start::Join("host:1".into()));
}

#[test]
fn the_activity_time_is_the_newest_file_to_the_games_depth_and_never_through_a_link() {
    let (_dir, game) = setup();
    let saves = game.join(SAVES);
    world(&saves, "w", T0);
    let deep = saves.join("w/a/b/c/d");
    fs::create_dir_all(&deep).unwrap();
    fs::write(deep.join("too_deep.dat"), "x").unwrap();
    assert_eq!(activity_ms(&saves.join("w")), Some(T0), "a file four folders down is not looked at");
    let ok = saves.join("w/a/b/c");
    fs::write(ok.join("fine.dat"), "x").unwrap();
    assert!(activity_ms(&saves.join("w")).unwrap() > T0, "three folders down is");
    #[cfg(unix)]
    {
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("new.dat"), "x").unwrap();
        std::os::unix::fs::symlink(outside.path(), saves.join("w/link")).unwrap();
        let before = activity_ms(&saves.join("w")).unwrap();
        assert!(before < now_ms() + 5_000);
    }
    assert_eq!(activity_ms(&saves.join("missing")), None);
}

#[test]
fn the_record_keeps_a_hundred_starts_ends_them_and_survives_a_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    let mut history = History::load(&paths);
    assert!(history.entries.is_empty(), "no file: empty");
    for n in 0..(KEEP as i64 + 20) {
        history.record_start(entry(T0 + n, None, Start::World(format!("w{n}"))));
    }
    assert_eq!(history.entries.len(), KEEP);
    assert_eq!(history.entries[0].started, T0 + 20, "the oldest are dropped");
    history.record_end(T0 + 50, T0 + 99);
    history.record_end(T0 + 51, T0);
    history.record_end(12345, T0);
    assert_eq!(history.entries.iter().find(|e| e.started == T0 + 50).unwrap().ended, Some(T0 + 99));
    assert_eq!(
        history.entries.iter().find(|e| e.started == T0 + 51).unwrap().ended,
        Some(T0 + 51),
        "never before its start"
    );
    history.save(&paths).unwrap();
    assert_eq!(History::load(&paths), history);
    let text = fs::read_to_string(paths.history_file()).unwrap();
    assert!(text.contains("\"kind\": \"world\"") && text.contains("\"target\": \"w119\""), "{text}");
}

#[test]
fn a_broken_or_foreign_record_is_an_empty_one() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    fs::create_dir_all(&paths.data).unwrap();
    for text in ["{", "[]", r#"{"entries": [{"started": 1, "kind": "teleport", "target": "x"}]}"#, ""] {
        fs::write(paths.history_file(), text).unwrap();
        assert!(History::load(&paths).entries.is_empty(), "{text:?}");
    }
    fs::write(paths.history_file(), r#"{"entries": [{"started": 5, "kind": "menu"}]}"#).unwrap();
    assert_eq!(
        History::load(&paths).entries,
        [Entry { started: 5, ended: None, version: String::new(), start: Start::Menu }]
    );
}

#[test]
fn recording_to_the_file_marks_the_start_and_its_end() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    let started = record_start(&paths, Start::Join("h:1".into()), "0.3.0").unwrap();
    let after = History::load(&paths);
    assert_eq!(after.entries.len(), 1);
    assert_eq!((after.entries[0].started, after.entries[0].ended), (started, None));
    record_end(&paths, started).unwrap();
    assert!(History::load(&paths).entries[0].ended.is_some());
    let again = record_start(&paths, Start::Menu, "0.3.0").unwrap();
    assert!(again >= started);
    assert_eq!(History::load(&paths).entries.len(), 2);
}

#[test]
fn how_long_ago_has_five_sizes() {
    let now = T0;
    assert_eq!(ago(now, now - 20_000), Ago::JustNow);
    assert_eq!(ago(now, now + 5000), Ago::JustNow, "a clock that went back is just now");
    assert_eq!(ago(now, now - 5 * MIN), Ago::Minutes(5));
    assert_eq!(ago(now, now - 59 * MIN), Ago::Minutes(59));
    assert_eq!(ago(now, now - 60 * MIN), Ago::Hours(1));
    assert_eq!(ago(now, now - 23 * 60 * MIN - 59 * MIN), Ago::Hours(23));
    assert_eq!(ago(now, now - 24 * 60 * MIN), Ago::Days(1));
    assert_eq!(ago(now, now - 29 * 24 * 60 * MIN), Ago::Days(29));
    assert_eq!(ago(now, now - 30 * 24 * 60 * MIN), Ago::Date);
}
