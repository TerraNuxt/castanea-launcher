//! "When the game starts": what the window does once the game runs (Minimise, Close, Keep open) and the way back.

use std::time::Duration;

use castanea_launcher_core::settings::OnGameStart;
use eframe::egui::{self, ViewportCommand};

use super::{Harness, install_build};
use crate::game::START_CONFIRM;

fn commands(output: &egui::FullOutput) -> Vec<ViewportCommand> {
    output.viewport_output.get(&egui::ViewportId::ROOT).map(|v| v.commands.clone()).unwrap_or_default()
}

fn started(choice: OnGameStart) -> Harness {
    let mut h = Harness::new("en");
    h.app.settings.on_game_start = choice;
    h.play_and_wait();
    assert!(h.app.is_running());
    h
}

/// Pretends the game has been up for a while.
fn age(h: &mut Harness) {
    let session = h.app.session.as_mut().unwrap();
    session.started = session.started.checked_sub(START_CONFIRM + Duration::from_secs(1)).unwrap();
}

#[test]
fn the_default_is_minimise_and_it_waits_until_the_game_stays_up() {
    let mut h = started(OnGameStart::default());
    assert_eq!(h.app.settings.on_game_start, OnGameStart::Minimise);
    assert!(commands(&h.frame()).is_empty(), "not before the game has stayed up");
    age(&mut h);
    let sent = commands(&h.frame());
    assert!(sent.contains(&ViewportCommand::Minimized(true)), "{sent:?}");
    assert!(h.app.minimised_for_game);
    assert!(commands(&h.frame()).is_empty(), "only once");
    h.app.stop();
}

#[test]
fn close_closes_the_window_and_leaves_the_game_to_run() {
    let mut h = started(OnGameStart::Close);
    age(&mut h);
    let sent = commands(&h.frame());
    assert!(sent.contains(&ViewportCommand::Close), "{sent:?}");
    assert!(!h.app.minimised_for_game);
    assert!(h.app.is_running(), "closing the window does not touch the game");
    h.app.stop();
}

#[test]
fn keep_open_does_nothing() {
    let mut h = started(OnGameStart::KeepOpen);
    age(&mut h);
    assert!(commands(&h.frame()).is_empty());
    assert!(!h.app.minimised_for_game);
    h.app.stop();
}

#[test]
fn the_window_comes_back_when_the_game_ends() {
    let mut h = Harness::new("en");
    install_build(&h.app.paths, "sleep 1");
    h.app.refresh_installed();
    h.play_and_wait();
    h.app.minimised_for_game = true;
    let mut sent = Vec::new();
    for _ in 0..300 {
        sent.extend(commands(&h.frame()));
        if !h.app.is_running() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!h.app.is_running());
    assert!(sent.contains(&ViewportCommand::Minimized(false)), "{sent:?}");
    assert!(sent.contains(&ViewportCommand::Focus), "{sent:?}");
    assert!(!h.app.minimised_for_game);
    assert!(h.app.session.as_ref().unwrap().outcome.is_some(), "the exit status is there to show");
}

#[test]
fn a_game_that_dies_at_once_keeps_the_launcher_in_view() {
    let mut h = Harness::new("en");
    install_build(&h.app.paths, "exit 3");
    h.app.refresh_installed();
    h.play_and_wait();
    let mut sent = Vec::new();
    for _ in 0..300 {
        sent.extend(commands(&h.frame()));
        if !h.app.is_running() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!sent.contains(&ViewportCommand::Minimized(true)) && !sent.contains(&ViewportCommand::Close), "{sent:?}");
    assert!(!h.app.start_action_due);
}
