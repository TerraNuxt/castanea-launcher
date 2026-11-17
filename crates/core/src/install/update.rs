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
use super::source::{GameSource, RemoteBuild};
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

/// The newest release with an archive for this platform; pre-releases count only when there is no other.
pub fn latest(builds: Vec<RemoteBuild>) -> Option<RemoteBuild> {
    let usable = builds.into_iter().filter(|b| b.asset.is_some());
    let (stable, pre): (Vec<_>, Vec<_>) = usable.partition(|b| !b.prerelease);
    let pick = |list: Vec<RemoteBuild>| {
        list.into_iter().reduce(|best, b| if version::newer(&b.version, &best.version) { b } else { best })
    };
    pick(stable).or_else(|| pick(pre))
}

/// Whether `remote` should replace `installed`: nothing is installed, or it is another release that is not older.
pub fn wanted(remote: &RemoteBuild, installed: Option<&Build>) -> bool {
    match installed {
        None => true,
        Some(build) => build.tag != remote.tag && !version::newer(&build.version, &remote.version),
    }
}

/// What a check found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Check {
    /// The installed build is the newest release.
    UpToDate,
    /// A newer release to download.
    Update(Box<RemoteBuild>),
    /// The source has no release for this platform.
    NoRelease,
}

pub fn check(source: &dyn GameSource, client: &Client, installed: Option<&Build>) -> Result<Check> {
    Ok(match latest(source.list(client)?) {
        None => Check::NoRelease,
        Some(remote) if wanted(&remote, installed) => Check::Update(Box::new(remote)),
        Some(_) => Check::UpToDate,
    })
}

/// Downloads `remote`, checks its SHA-256 and unpacks it into `build/next`; the installed build is not touched.
pub fn download(
    source: &dyn GameSource,
    client: &Client,
    remote: &RemoteBuild,
    paths: &Paths,
    progress: Progress<'_>,
    cancel: &AtomicBool,
) -> Result<Build> {
    let next = next_dir(paths);
    remove_dir(&next)?;
    fs::create_dir_all(paths.build()).map_err(|error| Error::io(&paths.build(), error))?;
    fs::create_dir_all(paths.downloads()).map_err(|error| Error::io(&paths.downloads(), error))?;
    source.install(client, remote, &next, &paths.downloads(), progress, cancel)
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
