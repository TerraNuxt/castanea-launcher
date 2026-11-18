//! The launcher's folders per platform (docs/design.md, "Data on disk").

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Puts config, data and cache below one folder: portable mode, tests and screenshots.
pub const HOME_ENV: &str = "CASTANEA_LAUNCHER_HOME";
const APP: &str = "castanea-launcher";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paths {
    pub config: PathBuf,
    pub data: PathBuf,
    pub cache: PathBuf,
}

impl Paths {
    /// All three folders below `root`.
    pub fn portable(root: &Path) -> Self {
        Self { config: root.join("config"), data: root.join("data"), cache: root.join("cache") }
    }

    /// The folders for this user: `CASTANEA_LAUNCHER_HOME` first, then the platform's places.
    pub fn for_user() -> Option<Self> {
        if let Some(root) = var(HOME_ENV) {
            return Some(Self::portable(Path::new(&root)));
        }
        Self::platform()
    }

    #[cfg(windows)]
    fn platform() -> Option<Self> {
        let roaming = PathBuf::from(var("APPDATA")?).join(APP);
        let local = PathBuf::from(var("LOCALAPPDATA")?).join(APP);
        Some(Self { config: roaming, data: local.clone(), cache: local.join("cache") })
    }

    #[cfg(not(windows))]
    fn platform() -> Option<Self> {
        let home = var("HOME").map(PathBuf::from);
        let xdg = |name: &str, fallback: &str| {
            var(name).map(PathBuf::from).or_else(|| home.as_ref().map(|h| h.join(fallback)))
        };
        Some(Self {
            config: xdg("XDG_CONFIG_HOME", ".config")?.join(APP),
            data: xdg("XDG_DATA_HOME", ".local/share")?.join(APP),
            cache: xdg("XDG_CACHE_HOME", ".cache")?.join(APP),
        })
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config.join("launcher.json")
    }

    /// The GitHub token file of old launchers; it is deleted at start.
    pub fn legacy_token_file(&self) -> PathBuf {
        self.config.join("github-token")
    }

    /// The one game folder: the game's working directory with saves, settings, mods and logs.
    pub fn game(&self) -> PathBuf {
        self.data.join("game")
    }

    /// What the launcher learned about the graphics backend (probe results, Vulkan starts that died).
    pub fn backend_file(&self) -> PathBuf {
        self.data.join("backend.json")
    }

    pub fn mods(&self) -> PathBuf {
        self.game().join("mods")
    }

    /// The installed release lives in `build/current`; `next` and `previous` exist only during an update.
    pub fn build(&self) -> PathBuf {
        self.data.join("build")
    }

    pub fn index_cache(&self) -> PathBuf {
        self.cache.join("index")
    }

    pub fn downloads(&self) -> PathBuf {
        self.cache.join("downloads")
    }
}

fn var(name: &str) -> Option<OsString> {
    std::env::var_os(name).filter(|value| !value.is_empty())
}

/// Keeps only characters that are safe in a folder name on every platform.
pub fn safe_name(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
        .collect();
    let trimmed = cleaned.trim_matches('.');
    if trimmed.is_empty() { "_".to_owned() } else { trimmed.chars().take(64).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_puts_everything_below_one_root() {
        let paths = Paths::portable(Path::new("/x"));
        assert_eq!(paths.settings_file(), Path::new("/x/config/launcher.json"));
        assert_eq!(paths.mods(), Path::new("/x/data/game/mods"));
        assert_eq!(paths.index_cache(), Path::new("/x/cache/index"));
    }

    #[test]
    fn safe_names_have_no_separators_or_dots_at_the_ends() {
        assert_eq!(safe_name("../evil/../x"), "_evil_.._x");
        assert_eq!(safe_name("v0.1.0"), "v0.1.0");
        assert_eq!(safe_name("..."), "_");
        assert_eq!(safe_name("a b:c"), "a_b_c");
    }
}
