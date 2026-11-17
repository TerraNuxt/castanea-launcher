//! Finding castanea builds that are on disk but not in the registry.

use std::path::{Path, PathBuf};

use super::local;
use super::registry::{Build, Origin, Registry};
use crate::paths::Paths;

/// Points the launcher at a game folder without the UI.
pub const GAME_DIR_ENV: &str = "CASTANEA_GAME_DIR";

/// Folders that may hold a game: `CASTANEA_GAME_DIR`, the launcher's own folder (a bundle), `castanea` on `PATH`,
/// the Nix package's data folder and a checkout in the home folder.
pub fn candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(dir) = std::env::var_os(GAME_DIR_ENV).filter(|v| !v.is_empty()) {
        out.push(PathBuf::from(dir));
    }
    if let Some(dir) = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_path_buf)) {
        out.push(dir);
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            if dir.join(local::BINARY).is_file() {
                out.push(dir.clone());
                if let Some(parent) = dir.parent() {
                    out.push(parent.join("share/castanea"));
                }
            }
        }
    }
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    if let Some(data) =
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| home.as_ref().map(|h| h.join(".local/share")))
    {
        out.push(data.join("castanea"));
    }
    if let Some(home) = home {
        out.push(home.join("castanea"));
    }
    out
}

/// Builds in `folders` that hold a game and are not registered yet.
pub fn detect(registry: &Registry, folders: &[PathBuf]) -> Vec<Build> {
    let mut found: Vec<Build> = Vec::new();
    let mut known = registry.clone();
    for folder in folders {
        let Ok(build) = local::inspect(&known, folder, Origin::Detected) else { continue };
        if known.builds.iter().any(|b| b.root == build.root) {
            continue;
        }
        known.put(build.clone());
        found.push(build);
    }
    found
}

/// Loads the registry, adopts managed builds and adds what `detect` finds; saves it when anything changed.
pub fn refresh(paths: &Paths) -> crate::Result<Registry> {
    let mut registry = Registry::load(paths)?;
    let before = registry.clone();
    registry.adopt_managed(paths);
    for build in detect(&registry, &candidates()) {
        registry.put(build);
    }
    if registry != before {
        registry.save(paths)?;
    }
    Ok(registry)
}
