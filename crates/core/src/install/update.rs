//! Keeping the one installed build at the newest release (docs/design.md, "The game build").
//!
//! `build/current` is the build the game runs from. An update is downloaded, checked and unpacked into `build/next`
//! while `current` stays untouched; only then `current` moves to `build/previous`, `next` becomes `current`, and
//! `previous` is deleted. A failed download or check never touches `current`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use super::build::Build;
use super::release::Release;
use super::source::GameSource;
use crate::download::{Client, Progress};
use crate::paths::Paths;
use crate::{Error, Result, version};

pub fn current_dir(paths: &Paths) -> PathBuf {
    paths.build().join("current")
}

pub fn next_dir(paths: &Paths) -> PathBuf {
    paths.build().join("next")
}

pub fn previous_dir(paths: &Paths) -> PathBuf {
    paths.build().join("previous")
}

/// The installed build: `current`, or `previous` when an update stopped half way.
pub fn installed(paths: &Paths) -> Option<Build> {
    Build::read(&current_dir(paths)).or_else(|| Build::read(&previous_dir(paths)))
}

/// Whether `release` should replace `installed`: nothing is installed, or it is another version that is not older.
pub fn wanted(release: &Release, installed: Option<&Build>) -> bool {
    match installed {
        None => true,
        Some(build) => build.version != release.version && !version::newer(&build.version, &release.version),
    }
}

/// What a check found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Check {
    /// The installed build is the newest release.
    UpToDate,
    /// A newer release to download.
    Update(Box<Release>),
    /// The release server has no release for this platform yet.
    NoRelease,
}

pub fn check(source: &dyn GameSource, client: &Client, installed: Option<&Build>) -> Result<Check> {
    Ok(match source.latest(client)? {
        None => Check::NoRelease,
        Some(release) if wanted(&release, installed) => Check::Update(Box::new(release)),
        Some(_) => Check::UpToDate,
    })
}

/// Downloads `release` (continuing a partial file from an earlier try), checks its SHA-256 and unpacks it into
/// `build/next`; the installed build is not touched.
pub fn download(
    source: &dyn GameSource,
    client: &Client,
    release: &Release,
    paths: &Paths,
    progress: Progress<'_>,
    cancel: &AtomicBool,
) -> Result<Build> {
    let next = next_dir(paths);
    remove_dir(&next)?;
    fs::create_dir_all(paths.build()).map_err(|error| Error::io(&paths.build(), error))?;
    fs::create_dir_all(paths.downloads()).map_err(|error| Error::io(&paths.downloads(), error))?;
    forget_other_downloads(&paths.downloads(), &release.sha256);
    source.install(client, release, &next, &paths.downloads(), progress, cancel)
}

/// Deletes partial and finished archives of other builds; the one for `keep` (a SHA-256) stays to be resumed.
fn forget_other_downloads(dir: &Path, keep: &str) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().starts_with(keep) && entry.path().is_file() {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// Makes `build/next` the installed build. The old one is deleted once the swap worked, and put back if it did not.
/// The game must not be running (Windows cannot move a folder with a running program in it).
pub fn activate(paths: &Paths) -> Result<Build> {
    let (current, next, previous) = (current_dir(paths), next_dir(paths), previous_dir(paths));
    if Build::read(&next).is_none() {
        return Err(Error::invalid(format!("{}: no downloaded build to use", next.display())));
    }
    if current.exists() {
        remove_dir(&previous)?;
        rename(&current, &previous)?;
    }
    if let Err(error) = rename(&next, &current) {
        if previous.exists() {
            let _ = rename(&previous, &current);
        }
        return Err(error);
    }
    // A leftover `previous` is harmless: it is removed before the next swap.
    let _ = remove_dir(&previous);
    Build::read(&current).ok_or_else(|| Error::invalid(format!("{}: the new build is incomplete", current.display())))
}

/// Retries a few times: on Windows a virus scanner may hold a fresh file for a moment.
fn rename(from: &Path, to: &Path) -> Result<()> {
    let mut tries = 0;
    loop {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(error) if tries < 5 && error.kind() == io::ErrorKind::PermissionDenied => {
                tries += 1;
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(error) => return Err(Error::io(from, error)),
        }
    }
}

fn remove_dir(dir: &Path) -> Result<()> {
    match fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::io(dir, error)),
    }
}
