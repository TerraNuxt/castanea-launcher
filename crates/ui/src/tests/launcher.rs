//! The launcher's own update in the UI: the quiet line, the actions it offers, and when a check starts.

use castanea_launcher_core::selfupdate::{Layout, Why};

use super::Harness;
use crate::app::{NoteKind, Page};
use crate::game::Update;
use crate::launcher::{LauncherUpdate, VERSION};

fn states() -> Vec<LauncherUpdate> {
    vec![
        LauncherUpdate::Idle,
        LauncherUpdate::Off,
        LauncherUpdate::Offline,
        LauncherUpdate::Checking,
        LauncherUpdate::UpToDate,
        LauncherUpdate::Downloading { version: "0.3.0".into() },
        LauncherUpdate::Restart { version: "0.3.0".into() },
        LauncherUpdate::Managed { version: "0.3.0".into(), why: Why::System },
        LauncherUpdate::Refused { version: "0.3.0".into() },
        LauncherUpdate::Failed("x".into()),
    ]
}

#[test]
fn the_launcher_line_reads_every_state_in_both_languages() {
    for language in ["en", "de"] {
        let mut h = Harness::new(language);
        for state in states() {
            h.app.launcher = state.clone();
            let text = h.app.launcher_line();
            assert!(text.contains(VERSION), "{language} {state:?}: {text}");
            assert!(!text.contains("launcher.") && !text.contains('{'), "{language} {state:?}: a raw key in {text}");
            let names_new = matches!(
                state,
                LauncherUpdate::Downloading { .. }
                    | LauncherUpdate::Restart { .. }
                    | LauncherUpdate::Managed { .. }
                    | LauncherUpdate::Refused { .. }
            );
            assert_eq!(text.contains("0.3.0"), names_new, "{language} {state:?}: {text}");
        }
    }
    let mut h = Harness::new("en");
    h.app.launcher = LauncherUpdate::UpToDate;
    assert_eq!(h.app.launcher_line(), format!("castanea launcher {VERSION} · up to date"));
    h.app.launcher = LauncherUpdate::Managed { version: "0.3.0".into(), why: Why::ReadOnly };
    assert!(h.app.launcher_line().ends_with("update available: 0.3.0 (managed install)"));
}

#[test]
fn every_state_draws_on_the_about_and_home_pages() {
    for language in ["en", "de"] {
        let mut h = Harness::new(language);
        h.app.launcher_notes = Some(("0.3.0".into(), "• faster start".into()));
        for state in states() {
            h.app.launcher = state.clone();
            for page in [Page::About, Page::Home] {
                h.go(page);
                h.draw();
            }
            assert!(
                h.app.notes.iter().all(|n| n.kind != NoteKind::Error),
                "{language} {state:?}: {:?}",
                h.app.notes.iter().map(|n| &n.text).collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn a_managed_install_explains_why_in_both_languages() {
    for language in ["en", "de"] {
        let h = Harness::new(language);
        for why in [Why::AppImage, Why::System, Why::ReadOnly] {
            let text = h.app.why_managed(why);
            assert!(!text.starts_with("launcher."), "{language} {why:?}: {text}");
        }
    }
}

#[test]
fn the_check_waits_for_the_game_and_follows_the_settings() {
    let dir = tempfile::tempdir().unwrap();
    let layout = Layout::new(dir.path().join("castanea-launcher"));

    // No layout (screenshots): never checks.
    let mut h = Harness::new("en");
    h.draw();
    assert_eq!(h.app.launcher, LauncherUpdate::Idle);

    // The game's own update is still downloading: wait.
    let mut h = Harness::new("en");
    h.app.launcher_layout = Some(layout.clone());
    h.app.launcher_checked = false;
    h.app.settings.offline = false;
    h.app.checked = true;
    h.app.update = Update::Downloading { version: "0.3.0".into() };
    h.draw();
    assert_eq!(h.app.launcher, LauncherUpdate::Idle);
    assert!(!h.app.launcher_checked);

    // Offline mode: no request.
    let mut h = Harness::new("en");
    h.app.launcher_layout = Some(layout.clone());
    h.app.launcher_checked = false;
    h.draw();
    assert_eq!(h.app.launcher, LauncherUpdate::Offline);
    assert_eq!(h.app.jobs.running, 0, "no request went out");

    // The setting is off: no request either.
    let mut h = Harness::new("en");
    h.app.launcher_layout = Some(layout);
    h.app.launcher_checked = false;
    h.app.settings.offline = false;
    h.app.settings.launcher_update = false;
    h.app.update = Update::UpToDate;
    h.app.checked = true;
    h.draw();
    assert_eq!(h.app.launcher, LauncherUpdate::Off);
    assert_eq!(h.app.jobs.running, 0);
}

#[test]
fn a_changed_setting_looks_again_but_a_waiting_restart_stays() {
    let mut h = Harness::new("en");
    h.app.launcher = LauncherUpdate::UpToDate;
    h.app.launcher_checked = true;
    h.app.reset_launcher_check();
    assert_eq!((h.app.launcher.clone(), h.app.launcher_checked), (LauncherUpdate::Idle, false));
    h.app.launcher = LauncherUpdate::Restart { version: "0.3.0".into() };
    h.app.launcher_checked = true;
    h.app.reset_launcher_check();
    assert!(matches!(h.app.launcher, LauncherUpdate::Restart { .. }), "the new launcher is in place already");
}

#[test]
fn a_launcher_that_was_not_signed_is_reported_as_refused_with_a_toast() {
    let mut h = Harness::new("en");
    h.app.launcher_notes = Some(("0.3.0".into(), String::new()));
    h.app.launcher = LauncherUpdate::Downloading { version: "0.3.0".into() };
    h.app.on_launcher_applied(Err(format!("{}: unknown key", castanea_launcher_core::NOT_SIGNED)));
    assert_eq!(h.app.launcher, LauncherUpdate::Refused { version: "0.3.0".into() });
    assert!(h.app.notes.iter().any(|n| n.kind == NoteKind::Error && n.text.contains("not signed by castanea")));
    h.app.launcher = LauncherUpdate::Downloading { version: "0.3.0".into() };
    let toasts = h.app.notes.len();
    h.app.on_launcher_applied(Err("connection reset".into()));
    assert!(matches!(h.app.launcher, LauncherUpdate::Failed(_)));
    assert_eq!(h.app.notes.len(), toasts, "a network failure is quiet");
    h.app.on_launcher_applied(Ok("0.3.0".into()));
    assert_eq!(h.app.launcher, LauncherUpdate::Restart { version: "0.3.0".into() });
}

#[test]
fn the_game_update_says_so_when_the_signature_is_missing() {
    let mut h = Harness::new("en");
    h.app.update = Update::Downloading { version: "0.3.0".into() };
    h.app.on_downloaded(&h.ctx.clone(), Err(format!("{}: no signature", castanea_launcher_core::NOT_SIGNED)));
    assert!(h.app.notes.iter().any(|n| n.text.contains("not signed by castanea") && n.text.contains("stays")));
    assert!(matches!(h.app.update, Update::Failed(_)));
}
