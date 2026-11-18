//! The Windows uninstall as data: what would be removed, worked out without touching the disk (so every platform tests
//! it), and the steps that remove it, run by the helper copy of the launcher after the real one has exited.

use std::fs;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;

use super::remove::{Failure, Report, Scope, delete_all};
use super::{Row, targets};
use crate::paths::Paths;
use crate::selfupdate::Layout;

/// A folder that is only ours when it is named for the launcher.
const OWN_FOLDER_PREFIX: &str = "castanea-launcher";
/// Files the release zip puts next to the exe.
const SHIPPED: [&str; 1] = ["LICENSE"];
/// Shortcuts a person may have made by hand; the launcher and its zip make none.
const SHORTCUT_NAMES: [&str; 3] = ["castanea-launcher.lnk", "Castanea Launcher.lnk", "Castanea.lnk"];

/// Where the launcher executable sits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallDir {
    /// A folder made for the launcher: its files and the folder itself (when empty after) go.
    Owned(PathBuf),
    /// A folder shared with other things (Downloads, the desktop, the user folder): only the exe and its update
    /// leftovers go, the folder stays.
    Shared(PathBuf),
}

/// Everything an uninstall removes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Uninstall {
    /// The launcher's data rows, as 'Delete everything' lists them (existing paths only).
    pub rows: Vec<(Row, Vec<PathBuf>)>,
    /// Shortcuts that exist.
    pub shortcuts: Vec<PathBuf>,
    /// Registry keys: none, the launcher writes none.
    pub registry: Vec<String>,
    /// The executable and what updates left next to it (`.old`, `.bad`, `.new`), those that exist.
    pub exe: PathBuf,
    pub leftovers: Vec<PathBuf>,
    /// Shipped files next to the exe that exist (only in an owned folder).
    pub shipped: Vec<PathBuf>,
    pub dir: InstallDir,
}

/// Folders the exe may sit in without owning them: the user's, the system's and the temp folder.
pub fn shared_folders() -> Vec<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    let mut all: Vec<PathBuf> = Vec::new();
    for home in [var("USERPROFILE"), var("HOME")].into_iter().flatten() {
        all.extend(["Downloads", "Desktop", "Documents"].map(|n| home.join(n)));
        all.push(home);
    }
    all.extend(
        ["TEMP", "TMP", "ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA", "APPDATA", "SystemRoot"]
            .iter()
            .filter_map(|n| var(n)),
    );
    all.push(std::env::temp_dir());
    all
}

/// Where hand-made shortcuts would be: the per-user Start menu and the desktop.
pub fn shortcut_folders() -> Vec<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    let mut all = Vec::new();
    if let Some(roaming) = var("APPDATA") {
        all.push(roaming.join("Microsoft/Windows/Start Menu/Programs"));
    }
    if let Some(home) = var("USERPROFILE") {
        all.push(home.join("Desktop"));
    }
    all
}

fn same(a: &Path, b: &Path) -> bool {
    let (a, b) =
        (fs::canonicalize(a).unwrap_or_else(|_| a.to_owned()), fs::canonicalize(b).unwrap_or_else(|_| b.to_owned()));
    a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy())
}

/// Works out the uninstall for the launcher at `exe`. Only reads: whether files exist decides what is listed.
pub fn plan(paths: &Paths, exe: &Path, shared: &[PathBuf], shortcut_dirs: &[PathBuf]) -> Uninstall {
    let layout = Layout::new(exe);
    let dir = layout.dir().to_owned();
    let named =
        dir.file_name().is_some_and(|n| n.to_string_lossy().to_ascii_lowercase().starts_with(OWN_FOLDER_PREFIX));
    let owned = named && dir.parent().is_some() && !shared.iter().any(|s| same(s, &dir));
    let exists = |p: &PathBuf| fs::symlink_metadata(p).is_ok();
    let rows = Row::OWN.iter().map(|row| (*row, targets(paths, *row))).collect();
    let shortcuts: Vec<PathBuf> =
        shortcut_dirs.iter().flat_map(|d| SHORTCUT_NAMES.iter().map(|n| d.join(n))).filter(exists).collect();
    let shipped = if owned { SHIPPED.iter().map(|n| dir.join(n)).filter(exists).collect() } else { Vec::new() };
    Uninstall {
        rows,
        shortcuts,
        registry: Vec::new(),
        exe: exe.to_owned(),
        leftovers: layout.leftovers().into_iter().filter(exists).collect(),
        shipped,
        dir: if owned { InstallDir::Owned(dir) } else { InstallDir::Shared(dir) },
    }
}

/// Removes what the plan lists, in this order: the data rows, the shortcuts, the update leftovers, the shipped files,
/// the executable (tried `tries` times, `pause` apart: a virus scanner or the exiting launcher may still hold it) and
/// last the install folder, when it is ours and empty. A failed item is reported and the rest goes on.
pub fn finish_steps(plan: &Uninstall, paths: &Paths, scope: &Scope, tries: u32, pause: Duration) -> Report {
    let mut report = delete_all(scope, paths, false).unwrap_or_default();
    let files = plan.shortcuts.iter().chain(&plan.leftovers).chain(&plan.shipped);
    for file in files {
        remove_plain(file, &mut report);
    }
    for attempt in 0..=tries {
        match fs::remove_file(&plan.exe) {
            Ok(()) => {
                report.removed += 1;
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) if attempt == tries => {
                report.failures.push(Failure { path: plan.exe.clone(), error: error.to_string() });
            }
            Err(_) => sleep(pause),
        }
    }
    if let InstallDir::Owned(dir) = &plan.dir {
        let _ = fs::remove_dir(dir);
    }
    report
}

fn remove_plain(path: &Path, report: &mut Report) {
    match fs::remove_file(path) {
        Ok(()) => report.removed += 1,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => report.failures.push(Failure { path: path.to_owned(), error: error.to_string() }),
    }
}
