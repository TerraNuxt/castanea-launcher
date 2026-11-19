//! Quick start: the Continue card for the last played world or server, what it starts, and the record of starts.

use std::fs;

use castanea_launcher_core::history::{self, Entry, History};
use castanea_launcher_core::launch::Start;

use super::Harness;
use super::layout as layout_checks;
use crate::app::Page;
use crate::game::Update;
use crate::pages::home::card;
use crate::probe;

/// A world folder the game would have saved.
fn world(h: &Harness, name: &str) {
    let dir = h.app.paths.game().join("saves").join(name);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("world.meta"), "x").unwrap();
}

/// Draws until the last played has been looked up.
fn looked_up(h: &mut Harness) {
    // The harness has looked once already, before the test set its files up.
    h.app.quick.stale = true;
    for _ in 0..300 {
        h.frame();
        if h.app.quick.loaded && !h.app.quick.loading && h.app.jobs.running == 0 {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("the last played was never looked up");
}

fn has_card(h: &mut Harness) -> bool {
    h.draw();
    probe::take(&h.ctx).iter().any(|(name, _)| name == "home:continue")
}

fn record(h: &Harness) -> Vec<Entry> {
    History::load(&h.app.paths).entries
}

#[cfg(unix)]
/// A game that writes its arguments to `args.txt` in its folder, one line per start, and stays up.
fn arg_logging(h: &mut Harness) {
    super::install_build(&h.app.paths, "echo \"$*\" >> args.txt; exec sleep 30");
    h.app.refresh_installed();
    h.app.update = Update::Offline;
    h.app.settings.game.graphics = castanea_launcher_core::settings::Graphics::Wgpu;
    h.app.os_name = Some("Eva Schmidt".into());
    h.app.settings.game.player_name = "Anna".into();
}

#[cfg(unix)]
fn start_and_wait(h: &mut Harness, how: impl FnOnce(&mut Harness)) {
    how(h);
    for _ in 0..500 {
        h.frame();
        if !h.app.starting {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(h.app.is_running(), "{:?}", h.app.notes.iter().map(|n| n.text.clone()).collect::<Vec<_>>());
    for _ in 0..200 {
        if h.app.paths.game().join("args.txt").exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("the game never ran");
}

#[cfg(unix)]
fn args_lines(h: &Harness) -> Vec<String> {
    fs::read_to_string(h.app.paths.game().join("args.txt")).unwrap().lines().map(str::to_owned).collect()
}

#[test]
fn nothing_played_shows_only_play() {
    let mut h = Harness::new("en");
    h.go(Page::Home);
    looked_up(&mut h);
    assert!(h.app.quick.played.is_none());
    assert!(!has_card(&mut h));
}

#[test]
fn the_card_names_the_world_played_last_in_both_languages() {
    for (language, kind) in [("en", "World"), ("de", "Welt")] {
        let mut h = Harness::new(language);
        world(&h, "lake");
        h.go(Page::Home);
        looked_up(&mut h);
        let played = h.app.quick.played.clone().unwrap();
        assert_eq!((played.start.clone(), played.label.as_str()), (Start::World("lake".into()), "lake"));
        assert!(has_card(&mut h), "{language}");
        let detail = card::detail(&h.app, &played);
        assert!(detail.starts_with(kind), "{detail}");
        let name = h.app.tr.fill("home.continue_name", &[("name", "lake"), ("detail", &detail)]);
        assert!(name.contains("lake") && name.contains(kind), "{name}");
    }
}

#[test]
fn how_long_ago_is_said_in_both_languages() {
    for (language, ago) in [("en", "5 min ago"), ("de", "vor 5 Min.")] {
        let h = Harness::new(language);
        assert_eq!(card::when_text(&h.app, history::now_ms() - 5 * 60_000), ago);
        assert!(!card::when_text(&h.app, history::now_ms()).is_empty());
        assert_eq!(card::when_text(&h.app, history::now_ms() - 90 * 86_400_000).len(), 10, "a date");
    }
}

#[cfg(unix)]
#[test]
fn continue_starts_the_world_straight_in_with_the_name_and_records_the_start() {
    let mut h = Harness::new("en");
    arg_logging(&mut h);
    world(&h, "lake");
    h.go(Page::Home);
    looked_up(&mut h);
    start_and_wait(&mut h, |h| h.app.continue_play(&h.ctx.clone()));
    assert_eq!(args_lines(&h), ["--world lake --name Anna"]);
    let entries = record(&h);
    assert_eq!((entries.len(), &entries[0].start, entries[0].ended), (1, &Start::World("lake".into()), None));
    assert_eq!(entries[0].version, "0.2.0");
    assert!(!has_card(&mut h), "no card while the game runs");
    h.app.stop();
    assert!(record(&h)[0].ended.is_some(), "the end is recorded");
    assert!(h.app.quick.stale && h.app.gallery.stale, "the last played and the screenshots are looked at again");
    looked_up(&mut h);
    assert!(has_card(&mut h), "the card is back when the game ended");
}

#[cfg(unix)]
#[test]
fn continue_joins_the_server_last_played() {
    let mut h = Harness::new("en");
    arg_logging(&mut h);
    fs::create_dir_all(h.app.paths.game()).unwrap();
    fs::write(h.app.paths.game().join("servers.txt"), "Friends\t10.0.0.5:47800\n").unwrap();
    let started = history::record_start(&h.app.paths, Start::Join("10.0.0.5:47800".into()), "0.2.0").unwrap();
    history::record_end(&h.app.paths, started).unwrap();
    h.go(Page::Home);
    looked_up(&mut h);
    assert_eq!(h.app.quick.played.as_ref().unwrap().label, "Friends");
    assert!(has_card(&mut h));
    start_and_wait(&mut h, |h| h.app.continue_play(&h.ctx.clone()));
    assert_eq!(args_lines(&h), ["--join 10.0.0.5:47800 --name Anna"]);
    assert_eq!(record(&h).len(), 2);
}

#[cfg(unix)]
#[test]
fn play_still_opens_the_start_screen_and_a_menu_start_is_recorded_as_a_menu_start() {
    let mut h = Harness::new("en");
    arg_logging(&mut h);
    world(&h, "lake");
    h.go(Page::Home);
    looked_up(&mut h);
    start_and_wait(&mut h, |h| h.app.play(&h.ctx.clone()));
    assert_eq!(args_lines(&h), ["--menu --name Anna"]);
    assert_eq!(record(&h)[0].start, Start::Menu);
}

#[cfg(unix)]
#[test]
fn extra_arguments_that_name_a_world_are_recorded_as_that_world() {
    let mut h = Harness::new("en");
    arg_logging(&mut h);
    h.app.settings.game.extra_args = "--world lake".into();
    world(&h, "lake");
    start_and_wait(&mut h, |h| h.app.play(&h.ctx.clone()));
    assert_eq!(record(&h)[0].start, Start::World("lake".into()));
}

#[test]
fn a_start_that_fails_records_nothing() {
    let mut h = Harness::new("en");
    h.app.settings.game.extra_args = "'open".into();
    h.app.update = Update::Offline;
    h.app.play_blocking_with(Start::World("lake".into()));
    assert!(record(&h).is_empty());
    assert!(h.app.session.is_none());
}

#[test]
fn a_record_from_a_newer_launcher_with_unknown_starts_does_not_break_the_card() {
    let mut h = Harness::new("en");
    fs::create_dir_all(&h.app.paths.data).unwrap();
    fs::write(h.app.paths.history_file(), r#"{"entries":[{"started":1,"kind":"teleport","target":"x"}]}"#).unwrap();
    world(&h, "lake");
    h.go(Page::Home);
    looked_up(&mut h);
    assert_eq!(h.app.quick.played.as_ref().unwrap().label, "lake", "falls back to the saves");
}

#[test]
fn the_card_fits_at_every_home_size() {
    for language in ["en", "de"] {
        for size in crate::tests::HOME_SIZES {
            let mut h = Harness::new(language);
            world(&h, "ein_sehr_langer_weltname_der_gekuerzt_werden_muss_weil_er_nicht");
            h.size = eframe::egui::vec2(size[0], size[1]);
            h.go(Page::Home);
            looked_up(&mut h);
            for state in [Update::UpToDate, Update::Downloading { version: "0.3.0".into() }, Update::Failed("x".into())]
            {
                h.app.update = state.clone();
                h.draw();
                let marks = probe::take(&h.ctx);
                let context = format!("{language} {size:?} {state:?}");
                let find = |name: &str| marks.iter().find(|(n, _)| n == name).cloned();
                let names = ["home:wordmark", "home:play", "home:status", "home:hello", "home:links", "home:continue"];
                let home: Vec<_> = names.iter().filter_map(|n| find(n)).collect();
                assert!(home.iter().any(|(n, _)| n == "home:continue"), "{context}: the card is drawn");
                let content = find("content").unwrap().1;
                layout_checks::all_inside(&context, &home, content);
                layout_checks::disjoint(&context, &home);
                let (card, play) = (find("home:continue").unwrap().1, find("home:play").unwrap().1);
                assert!((card.center().x - play.center().x).abs() < 1.0, "{context}: the card is centred under Play");
            }
        }
    }
}
