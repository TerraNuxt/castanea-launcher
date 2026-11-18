//! Getting the game folder ready: the folder, `mods/`, the build's `assets/` linked in. The game's own `settings.json`
//! (including its `backend_url`) is never written by the launcher.

use std::fs;
use std::path::Path;

use crate::install::Build;
use crate::{Error, Result};

pub fn prepare(game_dir: &Path, build: &Build) -> Result<()> {
    fs::create_dir_all(game_dir.join("mods")).map_err(|error| Error::io(game_dir, error))?;
    if !same_dir(game_dir, &build.root) {
        link_assets(&game_dir.join("assets"), &build.assets())?;
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
