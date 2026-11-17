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

/// How the game is started.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameOptions {
    /// The player name (`--name`).
    pub player_name: String,
    /// The relay backend, written into the game's `settings.json` as `backend_url`; empty leaves it alone.
    pub backend_url: String,
    pub graphics: Graphics,
    /// `CASTANEA_WINDOW`, such as `1280x720`; empty for the game's default.
    pub window: String,
    /// More arguments, split like a shell does (quotes group words).
    pub extra_args: String,
    pub env: Vec<(String, String)>,
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
            game: GameOptions::default(),
            dev_build: None,
        }
    }
}

impl Settings {
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
    fn missing_and_old_keys_are_fine() {
        let old = r#"{"offline": true, "sources": [{"kind": "github", "repo": "a/b"}], "last_profile": "x",
            "system_frame": true}"#;
        let settings: Settings = serde_json::from_str(old).unwrap();
        assert!(settings.offline);
        assert_eq!(settings.log_lines, 5000);
        assert_eq!(settings.release_url, DEFAULT_RELEASE_URL, "the old `repo` key is ignored");
    }
}
