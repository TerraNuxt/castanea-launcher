//! The launcher updating itself (docs/design.md, "Launcher self-update"): look for a newer launcher release, stage it
//! (resumable, hash and signature checked), put it in place next to the running one, and roll back when the new one
//! does not start. The game's own update flow (`install::update`) is separate and unchanged.

mod apply;
mod layout;
mod location;
mod stage;
mod state;

pub use apply::{Op, Os, apply_plan, rollback_plan};
pub use layout::Layout;
pub use location::{Install, Why, classify, detect, dir_writable};
pub use stage::stage;
pub use state::{Decision, Files, Pending, Staged, decide, now};

use std::time::Duration;

use crate::download::Client;
use crate::install::{GameSource, Release, ReleaseApi};
use crate::paths::Paths;
use crate::{Error, Result, version};

/// What a check found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Check {
    UpToDate,
    /// A newer launcher than the running one, not on the skip list.
    Update(Box<Release>),
    /// The server has no launcher release for this platform yet.
    NoRelease,
}

/// Compares the newest launcher release (of the stable channel, and the beta one when beta builds are on) with the
/// running version. A version that failed to start before is skipped.
pub fn check(api: &ReleaseApi, client: &Client, running: &str, files: &Files) -> Result<Check> {
    Ok(match api.latest(client)? {
        None => Check::NoRelease,
        Some(release) if files.is_skipped(&release.version) => Check::UpToDate,
        Some(release) if version::newer(&release.version, running) => Check::Update(Box::new(release)),
        Some(_) => Check::UpToDate,
    })
}

/// Puts the staged launcher in place. The marker is written first so a launcher that never starts is rolled back;
/// a failed step undoes itself and removes the marker.
pub fn apply(os: Os, layout: &Layout, files: &Files, staged: &Staged, running: &str) -> Result<()> {
    if !version::newer(&staged.version, running) {
        return Err(Error::invalid(format!("launcher {} is not newer than {running}", staged.version)));
    }
    let pending = Pending { from: running.to_owned(), to: staged.version.clone(), applied_at: now(), started_at: None };
    files.set_pending(&pending)?;
    if let Err(error) = apply::run(&apply_plan(os, layout, &staged.binary)) {
        files.clear_pending();
        return Err(error);
    }
    files.clear_staged();
    Ok(())
}

/// What the start-up check did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Startup {
    Normal,
    /// The version that failed to start was replaced by the one before it; start that one again.
    RolledBack {
        failed: String,
        restored: String,
    },
    /// The new launcher failed to start and there is no earlier one to restore.
    NoBackup {
        failed: String,
    },
}

/// First thing in `main`: decides from the marker whether this start is the first after an update (stamp it) or the
/// second one of a launcher that never drew a frame (roll back).
pub fn startup(os: Os, paths: &Paths, layout: &Layout, running: &str) -> Startup {
    let files = Files::new(paths);
    match decide(running, files.pending(), now()) {
        Decision::Nothing => {
            cleanup_in_background(layout);
            Startup::Normal
        }
        Decision::Stale => {
            files.clear_pending();
            cleanup_in_background(layout);
            Startup::Normal
        }
        Decision::FirstStart(pending) => {
            let _ = files.set_pending(&pending);
            Startup::Normal
        }
        Decision::RollBack(pending) => {
            let _ = files.skip(&pending.to);
            files.clear_pending();
            if !layout.old().is_file() {
                return Startup::NoBackup { failed: pending.to };
            }
            match apply::run(&rollback_plan(os, layout)) {
                Ok(()) => Startup::RolledBack { failed: pending.to, restored: pending.from },
                Err(_) => Startup::NoBackup { failed: pending.to },
            }
        }
    }
}

/// After the first successful frame: the new launcher is good, the old one can go.
pub fn first_frame(paths: &Paths, layout: &Layout) {
    let files = Files::new(paths);
    if files.pending().is_some() {
        files.clear_pending();
        cleanup_in_background(layout);
    }
}

/// Deletes what an update left next to the launcher. Returns whether everything is gone.
pub fn remove_leftovers(layout: &Layout) -> bool {
    layout.leftovers().iter().fold(true, |gone, path| {
        let _ = std::fs::remove_file(path);
        gone && !path.exists()
    })
}

/// Tries for a few seconds (a Windows virus scanner or the exiting old process may still hold a file), without a word.
fn cleanup_in_background(layout: &Layout) {
    if layout.leftovers().iter().all(|p| !p.exists()) {
        return;
    }
    let layout = layout.clone();
    std::thread::spawn(move || {
        for _ in 0..10 {
            if remove_leftovers(&layout) {
                return;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    });
}

/// Starts the launcher at `layout.exe` again with these arguments (after an update or a rollback).
pub fn relaunch(layout: &Layout, args: impl IntoIterator<Item = std::ffi::OsString>) -> std::io::Result<()> {
    std::process::Command::new(&layout.exe).args(args).spawn().map(|_| ())
}

#[cfg(test)]
mod tests;
