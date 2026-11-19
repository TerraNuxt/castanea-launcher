//! What the launcher put on this machine, and removing it again (docs/design.md, "Storage and reset"). Every row is a
//! group of paths below the launcher's own folders; nothing outside them is ever deleted.

mod remove;
mod size;
mod uninstall;

#[cfg(windows)]
mod finish;

#[cfg(test)]
mod tests;

use std::fs;
use std::path::PathBuf;

pub use remove::{Failure, Report, Scope, delete_all, delete_confirmed, delete_row, is_confirmation};
pub use size::{Size, measure, measure_path};
pub use uninstall::{InstallDir, Uninstall, finish_steps, plan as uninstall_plan, shared_folders, shortcut_folders};

#[cfg(windows)]
pub use finish::{finish_uninstall, start_finisher};

use crate::paths::Paths;

/// One line of the storage list in Settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Row {
    /// News, mod index and image caches, downloads in progress, the graphics probe results, launcher update files.
    Cache,
    /// The installed game build (`build/`) and the link to its assets.
    Game,
    Mods,
    /// The game's output logs.
    Logs,
    /// `launcher.json`, the window state and the record of starts (`history.json`).
    Settings,
    /// What the game wrote itself: worlds, its settings, crash reports. Never part of a bulk action.
    GameData,
}

impl Row {
    /// The rows 'Delete everything' removes: all the launcher's own, never the game's data.
    pub const OWN: [Self; 5] = [Self::Cache, Self::Game, Self::Mods, Self::Logs, Self::Settings];
    /// Every row, in the order Settings shows them.
    pub const ALL: [Self; 6] = [Self::Cache, Self::Game, Self::Mods, Self::Logs, Self::Settings, Self::GameData];

    /// The stem of the language keys (`storage.<key>`, `storage.<key>_hint`).
    pub fn key(self) -> &'static str {
        match self {
            Self::Cache => "cache",
            Self::Game => "game",
            Self::Mods => "mods",
            Self::Logs => "logs",
            Self::Settings => "settings",
            Self::GameData => "game_data",
        }
    }
}

/// The existing paths of one row. Each is a file, a folder or a link; a link is removed itself, never followed.
pub fn targets(paths: &Paths, row: Row) -> Vec<PathBuf> {
    let game = paths.game();
    let mut found = match row {
        Row::Cache => {
            let mut all = children(&paths.cache);
            all.extend([paths.backend_file(), paths.data.join("launcher-update")]);
            all
        }
        Row::Game => vec![paths.build(), game.join("assets")],
        Row::Mods => vec![paths.mods()],
        Row::Logs => vec![game.join(crate::launch::run::LOG_DIR)],
        Row::Settings => {
            let mut all = children(&paths.config);
            all.push(paths.data.join("window"));
            all.push(paths.history_file());
            all
        }
        Row::GameData => children(&game)
            .into_iter()
            .filter(|p| !matches!(name(p), "assets" | "mods" | crate::launch::run::LOG_DIR))
            .collect(),
    };
    found.retain(|p| fs::symlink_metadata(p).is_ok());
    found.sort();
    found.dedup();
    found
}

/// Whether the launcher knows a game data folder: the game's working directory exists.
pub fn knows_game_data(paths: &Paths) -> bool {
    paths.game().is_dir()
}

fn children(dir: &std::path::Path) -> Vec<PathBuf> {
    fs::read_dir(dir).map(|entries| entries.filter_map(|e| e.ok().map(|e| e.path())).collect()).unwrap_or_default()
}

fn name(path: &std::path::Path) -> &str {
    path.file_name().and_then(|n| n.to_str()).unwrap_or("")
}
