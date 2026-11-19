//! The default player name: the OS account's name until one is typed, what the game gets, and the greeting.

use castanea_launcher_core::settings::Settings;

use super::Harness;
use crate::app::Page;
use crate::probe;

#[test]
fn the_os_name_is_the_default_until_a_name_is_typed_and_returns_when_it_is_cleared() {
    let mut h = Harness::new("en");
    h.app.os_name = Some("Eva Schmidt".into());
    assert_eq!(h.app.player_name().as_deref(), Some("Eva Schmidt"));
    assert_eq!(h.app.game_name().as_deref(), Some("Eva_Schmidt"));
    assert_eq!(h.app.game_options().player_name, "Eva Schmidt");
    h.app.settings.game.player_name = "  Anna ".into();
    assert_eq!(h.app.player_name().as_deref(), Some("Anna"));
    h.app.os_name = Some("Someone Else".into());
    assert_eq!(h.app.game_name().as_deref(), Some("Anna"), "a typed name stays when the OS name changes");
    h.app.settings.game.player_name.clear();
    assert_eq!(h.app.player_name().as_deref(), Some("Someone Else"));
    h.app.os_name = None;
    assert_eq!((h.app.player_name(), h.app.game_name()), (None, None), "no name at all: the game keeps its own");
}

#[test]
fn a_typed_name_is_saved_and_the_os_name_is_never_written() {
    let mut h = Harness::new("en");
    h.app.os_name = Some("Eva Schmidt".into());
    h.app.settings.game.player_name = "Anna".into();
    h.app.save_settings();
    let saved = std::fs::read_to_string(h.app.paths.settings_file()).unwrap();
    assert!(saved.contains("\"player_name\": \"Anna\""), "{saved}");
    h.app.settings.game.player_name.clear();
    h.app.save_settings();
    let saved = std::fs::read_to_string(h.app.paths.settings_file()).unwrap();
    assert!(!saved.contains("Eva"), "the OS name is only a default, never stored: {saved}");
    assert_eq!(Settings::load(&h.app.paths).unwrap().game.player_name, "");
}

#[test]
fn the_home_page_greets_with_the_name_in_use_in_both_languages() {
    for (language, hello) in [("en", "Hello, Eva Schmidt"), ("de", "Hallo, Eva Schmidt")] {
        let mut h = Harness::new(language);
        h.app.os_name = Some("Eva Schmidt".into());
        assert_eq!(h.app.tr.fill("home.hello", &[("name", "Eva Schmidt")]), hello);
        h.go(Page::Home);
        h.draw();
        assert!(probe::take(&h.ctx).iter().any(|(n, _)| n == "home:hello"), "{language}: a greeting is drawn");
        h.app.os_name = None;
        h.draw();
        assert!(!probe::take(&h.ctx).iter().any(|(n, _)| n == "home:hello"), "{language}: no name, no greeting");
    }
}

#[test]
fn the_settings_page_draws_with_a_long_unicode_name() {
    let mut h = Harness::new("de");
    h.app.settings.game.player_name = "Jürgen Müller-Lüdenscheidt der Dritte von Irgendwo".into();
    h.go(Page::Settings);
    h.draw();
    assert_eq!(h.app.game_name().as_deref(), Some("Juergen_Mueller"));
}
