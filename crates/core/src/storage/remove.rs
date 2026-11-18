//! Deleting launcher files safely: only inside the launcher's own folders, links removed and never followed, a failed
//! item reported and skipped.

use std::fs;
use std::path::{Component, Path, PathBuf};

use super::{Row, targets};
use crate::paths::Paths;
use crate::{Error, Result};

/// What the user must type before the game's own data is deleted.
const CONFIRM_WORD: &str = "DELETE";

/// Whether `typed` is the confirmation word (exactly, apart from spaces around it).
pub fn is_confirmation(typed: &str) -> bool {
    typed.trim() == CONFIRM_WORD
}

/// One item that could not be removed, with the reason (a locked file on Windows, a missing permission).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub path: PathBuf,
    pub error: String,
}

/// What a deletion did: how many files, folders and links went, and what stayed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub removed: u64,
    pub failures: Vec<Failure>,
}

impl Report {
    pub fn is_clean(&self) -> bool {
        self.failures.is_empty()
    }

    fn fail(&mut self, path: &Path, error: impl ToString) {
        self.failures.push(Failure { path: path.to_owned(), error: error.to_string() });
    }
}

/// The folders deletion is allowed in, and paths it must never touch (a local game folder added by path).
#[derive(Clone, Debug)]
pub struct Scope {
    roots: Vec<PathBuf>,
    protected: Vec<PathBuf>,
}

impl Scope {
    /// The launcher's config, data and cache folders; `protected` are folders the user owns (a local build folder).
    pub fn new(paths: &Paths, protected: &[PathBuf]) -> Self {
        Self {
            roots: vec![paths.config.clone(), paths.data.clone(), paths.cache.clone()],
            protected: protected.to_vec(),
        }
    }

    /// Ok when `path` is a strict child of one of the folders (its parent resolved, so a link in the way cannot lead
    /// out), and neither holds nor lies in a protected folder.
    pub fn check(&self, path: &Path) -> Result<()> {
        let refuse = |why: &str| Err(Error::refused(format!("{}: {why}", path.display())));
        if path.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir)) {
            return refuse("not a plain path");
        }
        let (Some(parent), Some(file)) = (path.parent(), path.file_name()) else {
            return refuse("no parent folder");
        };
        let Ok(parent) = fs::canonicalize(parent) else { return refuse("its folder cannot be resolved") };
        let real = parent.join(file);
        let inside = self
            .roots
            .iter()
            .filter_map(|root| fs::canonicalize(root).ok())
            .any(|root| real.starts_with(&root) && real != root);
        if !inside {
            return refuse("outside the launcher's folders");
        }
        for keep in self.protected.iter().filter_map(|p| fs::canonicalize(p).ok()) {
            if real.starts_with(&keep) || keep.starts_with(&real) {
                return refuse("it holds a game folder you added; only the launcher's reference is removed");
            }
        }
        Ok(())
    }
}

/// Removes one row. Refused while the game runs; a failed item does not stop the others.
pub fn delete_row(scope: &Scope, paths: &Paths, row: Row, game_running: bool) -> Result<Report> {
    refuse_running(game_running)?;
    let mut report = Report::default();
    remove_targets(scope, &targets(paths, row), &mut report);
    Ok(report)
}

/// The game's own data (worlds, its settings): only with the typed word.
pub fn delete_confirmed(scope: &Scope, paths: &Paths, game_running: bool, typed: &str) -> Result<Report> {
    if !is_confirmation(typed) {
        return Err(Error::refused(format!("type {CONFIRM_WORD} to delete the game's data")));
    }
    delete_row(scope, paths, Row::GameData, game_running)
}

/// All the launcher's own rows ([`Row::OWN`]), then the folders that are empty now. The game's data stays.
pub fn delete_all(scope: &Scope, paths: &Paths, game_running: bool) -> Result<Report> {
    refuse_running(game_running)?;
    let mut report = Report::default();
    for row in Row::OWN {
        remove_targets(scope, &targets(paths, row), &mut report);
    }
    // Only empty folders go (`remove_dir`), the cache before the data folder that holds it on Windows.
    for dir in [paths.game(), paths.cache.clone(), paths.data.clone(), paths.config.clone()] {
        let _ = fs::remove_dir(&dir);
    }
    Ok(report)
}

fn refuse_running(running: bool) -> Result<()> {
    if running { Err(Error::refused("the game is running; close it first")) } else { Ok(()) }
}

fn remove_targets(scope: &Scope, found: &[PathBuf], report: &mut Report) {
    for path in found {
        match scope.check(path) {
            Ok(()) => remove_path(path, report),
            Err(error) => report.fail(path, error),
        }
    }
}

/// A file, a link or a folder's whole tree. Links are removed, not followed.
fn remove_path(path: &Path, report: &mut Report) {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => return report.fail(path, error),
    };
    if is_link(&meta) {
        return remove_one(path, report, remove_link);
    }
    if !meta.is_dir() {
        return remove_one(path, report, remove_file);
    }
    match fs::read_dir(path) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(entry) => remove_path(&entry.path(), report),
                    Err(error) => report.fail(path, error),
                }
            }
        }
        Err(error) => return report.fail(path, error),
    }
    remove_one(path, report, |p| fs::remove_dir(p));
}

fn remove_one(path: &Path, report: &mut Report, remove: impl Fn(&Path) -> std::io::Result<()>) {
    match remove(path) {
        Ok(()) => report.removed += 1,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => report.fail(path, explain(&error)),
    }
}

/// A symlink or, on Windows, a junction or other reparse point.
pub(super) fn is_link(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const REPARSE_POINT: u32 = 0x400;
        meta.file_attributes() & REPARSE_POINT != 0 || meta.file_type().is_symlink()
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}

/// A directory link needs `remove_dir` on Windows; both calls only ever remove the link itself.
fn remove_link(path: &Path) -> std::io::Result<()> {
    fs::remove_file(path).or_else(|_| fs::remove_dir(path))
}

/// Removes a file; on Windows a read-only one loses the flag and is tried once more.
fn remove_file(path: &Path) -> std::io::Result<()> {
    #[cfg(test)]
    if LOCKED.with(|l| l.borrow().iter().any(|p| p == path)) {
        return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "locked by a test"));
    }
    let first = fs::remove_file(path);
    #[cfg(windows)]
    if let Err(error) = &first
        && error.kind() == std::io::ErrorKind::PermissionDenied
        && let Ok(meta) = fs::metadata(path)
        && meta.permissions().readonly()
    {
        let mut permissions = meta.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        let _ = fs::set_permissions(path, permissions);
        return fs::remove_file(path);
    }
    first
}

#[cfg(test)]
thread_local! {
    /// Files a test pretends are locked by another program.
    pub(super) static LOCKED: std::cell::RefCell<Vec<PathBuf>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The error with a hint for the usual Windows cause.
fn explain(error: &std::io::Error) -> String {
    if cfg!(windows) && matches!(error.raw_os_error(), Some(5 | 32 | 33)) {
        format!("{error} (in use by another program)")
    } else {
        error.to_string()
    }
}
