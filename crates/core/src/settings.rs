//! `launcher.json`: the launcher's settings and the options the game starts with.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::json_file;
use crate::paths::Paths;

/// The release server (docs/release-api.md).
pub const DEFAULT_RELEASE_URL: &str = "https://castanea.xenoo.dev";

/// The game's renderer, through `CASTANEA_BACKEND` (docs/dev/wgpu-backend.md of the game).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Graphics {
    /// The game's own setting decides.
    #[default]
    Auto,
    Vulkan,
    Wgpu,
}

impl Graphics {
    pub const ALL: [Self; 3] = [Self::Auto, Self::Vulkan, Self::Wgpu];

    pub fn env_value(self) -> Option<&'static str> {
        match self {
            Self::Auto => None,
            Self::Vulkan => Some("vulkan"),
            Self::Wgpu => Some("wgpu"),
        }
    }
}

/// What the launcher window does once the game is running (the game itself is detached either way).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnGameStart {
    /// Minimise the window; it comes back when the game ends.
    #[default]
    Minimise,
    /// Close the launcher; the game keeps running and the next launcher start finds it again.
    Close,
    KeepOpen,
}

impl OnGameStart {
    pub const ALL: [Self; 3] = [Self::Minimise, Self::Close, Self::KeepOpen];
}

/// How the game is started.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameOptions {
    /// The player name the user chose (`--name`); empty means the OS account's name (docs/player-name.md).
    pub player_name: String,
    pub graphics: Graphics,
    /// `CASTANEA_WINDOW`, such as `1280x720`; empty for the game's default.
    pub window: String,
    /// More arguments, split like a shell does (quotes group words).
    pub extra_args: String,
    pub env: Vec<(String, String)>,
}

impl GameOptions {
    /// These options with the name the game gets: the typed one, else the OS account's (`playername::resolve`).
    pub fn with_default_name(&self, system: Option<&str>) -> Self {
        let name = crate::playername::resolve(&self.player_name, system).map(|n| n.shown).unwrap_or_default();
        Self { player_name: name, ..self.clone() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// `en`, `de`, or empty for the system language.
    pub language: String,
    /// No network: only the installed build and the cached index.
    pub offline: bool,
    /// Mod index locations: `https://` URLs or local files.
    pub indexes: Vec<String>,
    /// Lines of game output the Output page keeps.
    pub log_lines: usize,
    /// The release server the game builds come from; `CASTANEA_RELEASE_URL` overrides it.
    pub release_url: String,
    /// Also offer `beta` builds, not only `stable` ones.
    pub beta: bool,
    /// Keep the launcher itself at its newest release (download, stage and apply it).
    pub launcher_update: bool,
    /// Developer use: install a game build whose signature is missing or wrong. Debug builds only; a release build
    /// ignores it (see `unsigned_allowed`), and the launcher's own updates always need a signature.
    pub allow_unsigned: bool,
    /// What the launcher window does when the game has started.
    pub on_game_start: OnGameStart,
    pub game: GameOptions,
    /// For people building the game from source: run this folder in place instead of the newest release.
    pub dev_build: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: String::new(),
            offline: false,
            indexes: Vec::new(),
            log_lines: 5000,
            release_url: DEFAULT_RELEASE_URL.to_owned(),
            beta: false,
            launcher_update: true,
            allow_unsigned: false,
            on_game_start: OnGameStart::default(),
            game: GameOptions::default(),
            dev_build: None,
        }
    }
}

impl Settings {
    /// Whether unsigned game builds may be installed: the switch is on and this is a debug build.
    pub fn unsigned_allowed(&self) -> bool {
        self.allow_unsigned && cfg!(debug_assertions)
    }

    pub fn load(paths: &Paths) -> Result<Self> {
        json_file::load_or_default(&paths.settings_file())
    }

    pub fn save(&self, paths: &Paths) -> Result<()> {
        json_file::save(&paths.settings_file(), self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_use_the_release_server_and_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::portable(dir.path());
        let mut settings = Settings::load(&paths).unwrap();
        assert_eq!(settings.release_url, DEFAULT_RELEASE_URL);
        assert!(!settings.beta);
        settings.beta = true;
        settings.language = "de".to_owned();
        settings.game.graphics = Graphics::Wgpu;
        settings.game.env.push(("RUST_LOG".to_owned(), "info".to_owned()));
        settings.indexes.push("https://example.org/index.json".to_owned());
        settings.save(&paths).unwrap();
        assert_eq!(Settings::load(&paths).unwrap(), settings);
    }

    #[test]
    fn when_the_game_starts_defaults_to_minimise_and_round_trips() {
        assert_eq!(Settings::default().on_game_start, OnGameStart::Minimise);
        let none: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(none.on_game_start, OnGameStart::Minimise, "an old file without the key");
        for choice in OnGameStart::ALL {
            let settings = Settings { on_game_start: choice, ..Settings::default() };
            let text = serde_json::to_string(&settings).unwrap();
            assert_eq!(serde_json::from_str::<Settings>(&text).unwrap().on_game_start, choice);
        }
        let named: Settings = serde_json::from_str(r#"{"on_game_start": "keep_open"}"#).unwrap();
        assert_eq!(named.on_game_start, OnGameStart::KeepOpen);
        let close: Settings = serde_json::from_str(r#"{"on_game_start": "close"}"#).unwrap();
        assert_eq!(close.on_game_start, OnGameStart::Close);
    }

    #[test]
    fn the_player_name_is_the_users_until_cleared_then_the_os_name() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::portable(dir.path());
        let mut settings = Settings::default();
        assert!(settings.game.player_name.is_empty(), "unset by default: the OS name is used");
        assert_eq!(settings.game.with_default_name(Some("Eva Schmidt")).player_name, "Eva Schmidt");
        assert_eq!(settings.game.with_default_name(None).player_name, "");
        settings.game.player_name = "  Anna ".to_owned();
        settings.save(&paths).unwrap();
        let loaded = Settings::load(&paths).unwrap();
        assert_eq!(loaded.game.with_default_name(Some("Eva Schmidt")).player_name, "Anna", "the typed name wins");
        assert_eq!(
            loaded.game.with_default_name(Some("Eva Neu")).player_name,
            "Anna",
            "and stays when the OS name changes"
        );
        let mut cleared = loaded;
        cleared.game.player_name.clear();
        assert_eq!(cleared.game.with_default_name(Some("Eva Neu")).player_name, "Eva Neu");
        assert!(
            !std::fs::read_to_string(paths.settings_file()).unwrap().contains("Eva"),
            "the OS name is never stored"
        );
    }

    #[test]
    fn missing_and_old_keys_are_fine() {
        let old = r#"{"offline": true, "sources": [{"kind": "github", "repo": "a/b"}], "last_profile": "x",
            "system_frame": true}"#;
        let settings: Settings = serde_json::from_str(old).unwrap();
        assert!(settings.offline);
        assert_eq!(settings.log_lines, 5000);
        assert_eq!(settings.release_url, DEFAULT_RELEASE_URL, "the old `repo` key is ignored");
    }

    #[test]
    fn a_stale_backend_url_key_is_ignored_and_dropped_at_the_next_save() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::portable(dir.path());
        let old = r#"{"game": {"player_name": "Anna", "backend_url": "https://relay.example.org"}}"#;
        std::fs::create_dir_all(&paths.config).unwrap();
        std::fs::write(paths.settings_file(), old).unwrap();
        let settings = Settings::load(&paths).unwrap();
        assert_eq!(settings.game.player_name, "Anna");
        settings.save(&paths).unwrap();
        assert!(!std::fs::read_to_string(paths.settings_file()).unwrap().contains("backend_url"));
    }
}
