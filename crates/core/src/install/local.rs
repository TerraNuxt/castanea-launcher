//! A game build in a local folder: a release folder (binary next to `assets/`) or a checkout (`target/release`). For
//! people who build the game from source; the folder is run in place, never copied or changed.

use std::fs;
use std::path::{Path, PathBuf};

use super::build::Build;
use crate::{Error, Result};

pub const BINARY: &str = if cfg!(windows) { "castanea.exe" } else { "castanea" };
pub const VERSION_FILE: &str = "version.txt";
/// Points the launcher at a local build folder for one run, like the `--game-dir` flag.
pub const GAME_DIR_ENV: &str = "CASTANEA_GAME_DIR";

/// The game binary for a game folder that holds `assets/`: next to it, or in a checkout's `target/release`.
pub fn find_binary(root: &Path) -> Option<PathBuf> {
    if !root.join("assets").is_dir() {
        return None;
    }
    [root.join(BINARY), root.join("target/release").join(BINARY), root.join("bin").join(BINARY)]
        .into_iter()
        .find(|path| path.is_file())
}

/// `version.txt` next to the binary or in the folder, else the checkout's Cargo.toml version, else `local`.
pub fn read_version(root: &Path, binary: &Path) -> String {
    let candidates = [binary.parent().map(|p| p.join(VERSION_FILE)), Some(root.join(VERSION_FILE))];
    for file in candidates.into_iter().flatten() {
        if let Ok(text) = fs::read_to_string(file)
            && let Some(line) = text.lines().next().map(str::trim).filter(|l| !l.is_empty() && l.len() <= 64)
        {
            return line.to_owned();
        }
    }
    if let Ok(text) = fs::read_to_string(root.join("Cargo.toml"))
        && text.contains("name = \"castanea\"")
        && let Some(version) = text.lines().find_map(|l| l.trim().strip_prefix("version = \""))
    {
        return version.trim_end_matches('"').to_owned();
    }
    "local".to_owned()
}

/// The build in `folder`; an error says what is missing.
pub fn inspect(folder: &Path) -> Result<Build> {
    let root = folder.canonicalize().map_err(|error| Error::io(folder, error))?;
    let binary = find_binary(&root).ok_or_else(|| {
        Error::invalid(format!(
            "{}: no game here (needs assets/ and {BINARY} or target/release/{BINARY})",
            root.display()
        ))
    })?;
    let version = read_version(&root, &binary);
    Ok(Build { version, root, binary, ..Build::default() })
}
