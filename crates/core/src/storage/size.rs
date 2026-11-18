//! How much room a row takes. Meant for a background thread: it walks without following links and stops when asked.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use super::{Row, targets};
use crate::paths::Paths;

/// Bytes and number of files of a row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Size {
    pub bytes: u64,
    pub files: u64,
}

impl Size {
    fn add(&mut self, other: Self) {
        self.bytes += other.bytes;
        self.files += other.files;
    }
}

/// The size of all paths of `row`; `None` when `cancel` was set before the walk ended.
pub fn measure(paths: &Paths, row: Row, cancel: &AtomicBool) -> Option<Size> {
    let mut total = Size::default();
    for path in targets(paths, row) {
        total.add(measure_path(&path, cancel)?);
    }
    Some(total)
}

/// One file or folder tree; a link counts as nothing (its target is not the launcher's).
pub fn measure_path(path: &Path, cancel: &AtomicBool) -> Option<Size> {
    let mut total = Size::default();
    let mut stack = vec![path.to_owned()];
    while let Some(next) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let Ok(meta) = fs::symlink_metadata(&next) else { continue };
        if super::remove::is_link(&meta) {
            continue;
        }
        if meta.is_dir() {
            if let Ok(entries) = fs::read_dir(&next) {
                stack.extend(entries.filter_map(|e| e.ok().map(|e| e.path())));
            }
        } else {
            total.add(Size { bytes: meta.len(), files: 1 });
        }
    }
    Some(total)
}
