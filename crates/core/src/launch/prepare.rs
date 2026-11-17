//! Getting a profile's game folder ready: the folder, `mods/`, the build's `assets/` linked in, `backend_url` set.

use std::fs;
use std::path::Path;

use serde_json::{Map, Value};

use crate::install::Build;
use crate::json_file;
use crate::{Error, Result};

pub const GAME_SETTINGS: &str = "settings.json";
pub const BACKEND_KEY: &str = "backend_url";

pub fn prepare(game_dir: &Path, build: &Build, backend_url: &str) -> Result<()> {
    fs::create_dir_all(game_dir.join("mods")).map_err(|error| Error::io(game_dir, error))?;
    if !same_dir(game_dir, &build.root) {
        link_assets(&game_dir.join("assets"), &build.assets())?;
    }
    if !backend_url.trim().is_empty() {
        set_game_setting(&game_dir.join(GAME_SETTINGS), BACKEND_KEY, backend_url.trim())?;
    }
    Ok(())
}

/// A build run in its own folder (a checkout) needs no link.
fn same_dir(a: &Path, b: &Path) -> bool {
    matches!((a.canonicalize(), b.canonicalize()), (Ok(a), Ok(b)) if a == b)
}

/// Makes `link` point at `target`; an old link is replaced, a real folder is never touched.
pub fn link_assets(link: &Path, target: &Path) -> Result<()> {
    if !target.is_dir() {
        return Err(Error::invalid(format!("{}: the build has no assets folder", target.display())));
    }
    if let Ok(meta) = fs::symlink_metadata(link) {
        if is_link(link, &meta) {
            if fs::read_link(link).is_ok_and(|current| current == target) {
                return Ok(());
            }
            remove_link(link)?;
        } else {
            return Err(Error::invalid(format!(
                "{} is a real folder, not a link; move it away so the launcher can link the build's assets",
                link.display()
            )));
        }
    }
    create_link(target, link)
}

#[cfg(unix)]
fn is_link(_link: &Path, meta: &fs::Metadata) -> bool {
    meta.file_type().is_symlink()
}

#[cfg(windows)]
fn is_link(link: &Path, meta: &fs::Metadata) -> bool {
    meta.file_type().is_symlink() || junction::exists(link).unwrap_or(false)
}

#[cfg(unix)]
fn remove_link(link: &Path) -> Result<()> {
    fs::remove_file(link).map_err(|error| Error::io(link, error))
}

#[cfg(windows)]
fn remove_link(link: &Path) -> Result<()> {
    fs::remove_dir(link).or_else(|_| fs::remove_file(link)).map_err(|error| Error::io(link, error))
}

#[cfg(unix)]
fn create_link(target: &Path, link: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, link).map_err(|error| Error::io(link, error))
}

/// A junction needs no admin rights or developer mode, unlike a symlink.
#[cfg(windows)]
fn create_link(target: &Path, link: &Path) -> Result<()> {
    junction::create(target, link).map_err(|error| Error::io(link, error))
}

/// Sets one key in the game's `settings.json` (a flat JSON object), keeping every other key.
pub fn set_game_setting(file: &Path, key: &str, value: &str) -> Result<()> {
    let mut map: Map<String, Value> = match fs::read_to_string(file) {
        Ok(text) if !text.trim().is_empty() => match serde_json::from_str(&text) {
            Ok(Value::Object(map)) => map,
            _ => return Err(Error::invalid(format!("{}: not a JSON object; fix or delete it", file.display()))),
        },
        Ok(_) => Map::new(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Map::new(),
        Err(error) => return Err(Error::io(file, error)),
    };
    if map.get(key).and_then(Value::as_str) == Some(value) {
        return Ok(());
    }
    map.insert(key.to_owned(), Value::String(value.to_owned()));
    json_file::save(file, &Value::Object(map))
}
