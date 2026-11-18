//! Keeping the one installed build at the newest release (docs/design.md, "The game build").
//!
//! `build/current` is the build the game runs from. An update is downloaded, checked and unpacked into `build/next`
//! while `current` stays untouched; only then `current` moves to `build/previous`, `next` becomes `current`, and
//! `previous` is deleted. A failed download or check never touches `current`; start-up removes what a stopped update left.

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

/// Makes `build/next` the installed build. The old one goes once the swap worked; if the swap fails, `current` is
/// put back and nothing else changes. The game must not be running (Windows cannot move a folder with a running
/// program in it).
pub fn activate(paths: &Paths) -> Result<Build> {
    let (current, next, previous) = (current_dir(paths), next_dir(paths), previous_dir(paths));
    let Some(new) = Build::read(&next) else {
        return Err(Error::invalid(format!("{}: no downloaded build to use", next.display())));
    };
    if current.exists() {
        // Nothing is moved yet: a folder that cannot be removed stops the update with the installed build untouched.
        remove_dir(&previous)?;
        rename(&current, &previous)?;
    }
    if let Err(error) = rename(&next, &current) {
        if previous.exists() {
            let _ = rename(&previous, &current);
        }
        return Err(error);
    }
    // Best effort: a file a program still holds is left for the next start (`clean_leftovers`), never a failure.
    let _ = remove_dir(&previous);
    if let Some(sha) = &new.sha256 {
        forget_archive(&paths.downloads(), sha);
    }
    Build::read(&current).ok_or_else(|| Error::invalid(format!("{}: the new build is incomplete", current.display())))
}

/// Deletes the archive and partial file of a build that is now installed; nothing of it is needed any more.
fn forget_archive(dir: &Path, sha256: &str) {
    for name in [format!("{sha256}.zip"), format!("{sha256}.part")] {
        let _ = fs::remove_file(dir.join(name));
    }
}

/// At start, before anything downloads: removes `next` and `previous` left by an update that stopped. When `current` is
/// gone or broken and `previous` holds a whole build, that build is put back as `current`.
pub fn clean_leftovers(paths: &Paths) {
    let (current, next, previous) = (current_dir(paths), next_dir(paths), previous_dir(paths));
    let _ = remove_dir(&next);
    if Build::read(&current).is_some() {
        let _ = remove_dir(&previous);
    } else if Build::read(&previous).is_some() {
        if remove_dir(&current).is_ok() {
            let _ = rename(&previous, &current);
        }
    } else {
        let _ = remove_dir(&previous);
    }
}

/// Retries a few times: on Windows a virus scanner may hold a fresh file for a moment.
fn rename(from: &Path, to: &Path) -> Result<()> {
    let mut tries = 0;
    loop {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(error) if tries < RETRIES && error.kind() == io::ErrorKind::PermissionDenied => {
                tries += 1;
                std::thread::sleep(RETRY_WAIT);
            }
            Err(error) => return Err(Error::io(from, error)),
        }
    }
}

/// Removes a folder, retrying quietly when a file in it is locked (a virus scanner, a program that is just exiting).
fn remove_dir(dir: &Path) -> Result<()> {
    match retry_quietly(|| fs::remove_dir_all(dir)) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(Error::io(dir, error)),
        _ => Ok(()),
    }
}

/// Runs `op`, and again after a short wait while it fails for a reason that may pass (not `NotFound`). The last result
/// is returned; nothing is logged.
pub(super) fn retry_quietly(mut op: impl FnMut() -> io::Result<()>) -> io::Result<()> {
    let mut tries = 0;
    loop {
        match op() {
            Err(error) if error.kind() != io::ErrorKind::NotFound && tries < RETRIES => {
                tries += 1;
                std::thread::sleep(RETRY_WAIT);
            }
            result => return result,
        }
    }
}

const RETRIES: u32 = 5;
const RETRY_WAIT: Duration = Duration::from_millis(200);
