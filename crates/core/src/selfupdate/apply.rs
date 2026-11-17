//! Putting a new launcher in place, as a plan of file operations (so the sequence can be tested on any system) and a
//! runner that undoes what it did when a step fails.
//!
//! Linux: the running file may be replaced by an atomic rename, so the old one is copied to `.old` first.
//! Windows: the running exe cannot be overwritten or deleted but can be renamed: it becomes `.old.exe`, the new exe
//! takes its name, and the running process carries on until the user restarts.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::layout::Layout;
use crate::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Unix,
    Windows,
}

impl Os {
    pub const fn current() -> Self {
        if cfg!(windows) { Self::Windows } else { Self::Unix }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    /// Best effort: a file that is not there or is locked is not an error here.
    Remove(PathBuf),
    Rename {
        from: PathBuf,
        to: PathBuf,
    },
    Copy {
        from: PathBuf,
        to: PathBuf,
    },
    /// Gives `path` the permissions of `like` (and the execute bits) on Unix.
    ModeLike {
        path: PathBuf,
        like: PathBuf,
    },
}

fn rename(from: &Path, to: &Path) -> Op {
    Op::Rename { from: from.to_owned(), to: to.to_owned() }
}

fn copy(from: &Path, to: &Path) -> Op {
    Op::Copy { from: from.to_owned(), to: to.to_owned() }
}

/// The steps that make `staged` the launcher at `layout.exe`, keeping the current one as `layout.old()`.
pub fn apply_plan(os: Os, layout: &Layout, staged: &Path) -> Vec<Op> {
    let (exe, old, incoming) = (&layout.exe, layout.old(), layout.incoming());
    match os {
        Os::Unix => vec![
            Op::Remove(old.clone()),
            copy(exe, &old),
            Op::Remove(incoming.clone()),
            copy(staged, &incoming),
            Op::ModeLike { path: incoming.clone(), like: exe.clone() },
            rename(&incoming, exe),
        ],
        Os::Windows => vec![
            Op::Remove(old.clone()),
            rename(exe, &old),
            Op::Remove(incoming.clone()),
            copy(staged, &incoming),
            rename(&incoming, exe),
        ],
    }
}

/// The steps that put the previous launcher back and set the failed one aside.
pub fn rollback_plan(os: Os, layout: &Layout) -> Vec<Op> {
    let (exe, old, bad) = (&layout.exe, layout.old(), layout.bad());
    match os {
        Os::Unix => vec![rename(&old, exe)],
        Os::Windows => vec![Op::Remove(bad.clone()), rename(exe, &bad), rename(&old, exe)],
    }
}

/// What undoes a finished step.
enum Undo {
    RenameBack { from: PathBuf, to: PathBuf },
    Delete(PathBuf),
}

/// Runs the steps in order. When one fails, the earlier renames and copies are undone and the error is returned.
pub fn run(ops: &[Op]) -> Result<()> {
    let mut undo = Vec::new();
    for op in ops {
        if let Err(error) = step(op, &mut undo) {
            for action in undo.into_iter().rev() {
                match action {
                    Undo::RenameBack { from, to } => {
                        let _ = fs::rename(&to, &from);
                    }
                    Undo::Delete(path) => {
                        let _ = fs::remove_file(path);
                    }
                }
            }
            return Err(error);
        }
    }
    Ok(())
}

fn step(op: &Op, undo: &mut Vec<Undo>) -> Result<()> {
    match op {
        Op::Remove(path) => {
            let _ = fs::remove_file(path);
        }
        Op::Rename { from, to } => {
            retry(from, || fs::rename(from, to))?;
            undo.push(Undo::RenameBack { from: from.clone(), to: to.clone() });
        }
        Op::Copy { from, to } => {
            fs::copy(from, to).map_err(|error| Error::io(to, error))?;
            undo.push(Undo::Delete(to.clone()));
        }
        Op::ModeLike { path, like } => mode_like(path, like)?,
    }
    Ok(())
}

/// A few tries: on Windows a virus scanner may hold a fresh file for a moment.
fn retry(path: &Path, mut action: impl FnMut() -> io::Result<()>) -> Result<()> {
    let mut tries = 0;
    loop {
        match action() {
            Ok(()) => return Ok(()),
            Err(error) if tries < 5 && error.kind() == io::ErrorKind::PermissionDenied => {
                tries += 1;
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(error) => return Err(Error::io(path, error)),
        }
    }
}

#[cfg(unix)]
fn mode_like(path: &Path, like: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(like).map_or(0o755, |m| m.permissions().mode() & 0o777) | 0o111;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|error| Error::io(path, error))
}

#[cfg(not(unix))]
fn mode_like(_path: &Path, _like: &Path) -> Result<()> {
    Ok(())
}
