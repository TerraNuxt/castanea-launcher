//! `launcher.json`: the launcher's own settings.

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::json_file;
use crate::paths::Paths;

pub const DEFAULT_REPO: &str = "TerraNuxt/castanea";

/// Where game builds come from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceConfig {
    /// The releases of a GitHub repository, `owner/name`.
    Github { repo: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// `en`, `de`, or empty for the system language.
    pub language: String,
    /// No network: only installed builds and the cached index.
    pub offline: bool,
    pub sources: Vec<SourceConfig>,
    /// Mod index locations: `https://` URLs or local files.
    pub indexes: Vec<String>,
    pub last_profile: Option<String>,
    /// Lines of game output the Play tab keeps.
    pub log_lines: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: String::new(),
            offline: false,
            sources: vec![SourceConfig::Github { repo: DEFAULT_REPO.to_owned() }],
            indexes: Vec::new(),
            last_profile: None,
            log_lines: 5000,
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
    fn defaults_use_the_game_repo_and_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::portable(dir.path());
        let mut settings = Settings::load(&paths).unwrap();
        assert_eq!(settings.sources, [SourceConfig::Github { repo: DEFAULT_REPO.to_owned() }]);
        settings.language = "de".to_owned();
        settings.indexes.push("https://example.org/index.json".to_owned());
        settings.save(&paths).unwrap();
        assert_eq!(Settings::load(&paths).unwrap(), settings);
    }

    #[test]
    fn missing_keys_take_their_defaults() {
        let settings: Settings = serde_json::from_str(r#"{"offline": true}"#).unwrap();
        assert!(settings.offline);
        assert_eq!(settings.log_lines, 5000);
    }
}
