//! The screenshots the game takes: scanning the folder, sorting newest first, grouping by day, thumbnails with a disk
//! cache, and the file actions (rename, trash, delete). No UI; docs/screenshots-gallery.md.
//!
//! The game writes `screenshots/castanea-<milliseconds since 1970>.png` in its working directory (F2), so the folder is
//! `<game folder>/screenshots`. The name carries the time; it is the time a shot was taken, the file's time is the fallback.

pub mod files;
pub mod thumb;
pub mod time;
pub mod trash;

#[cfg(test)]
mod tests;

use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub use time::Day;

/// Folder of the game's screenshots below its working directory.
pub const DIR: &str = "screenshots";
/// File extensions that count as a screenshot (lower case).
pub const EXTENSIONS: [&str; 3] = ["png", "jpg", "jpeg"];

/// One screenshot file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shot {
    pub path: PathBuf,
    /// The file name.
    pub name: String,
    /// When it was taken, milliseconds since 1970: from the name when it has one, else the file's time.
    pub taken_ms: i64,
    pub size: u64,
    /// The file's modification time, milliseconds since 1970 (part of the thumbnail cache key).
    pub modified_ms: i64,
}

impl Shot {
    /// A picture outside the screenshots folder (a world's own picture), from its file's metadata.
    pub fn from_file(path: &Path) -> Option<Self> {
        let meta = fs::metadata(path).ok().filter(|m| m.is_file())?;
        let name = path.file_name()?.to_string_lossy().into_owned();
        let modified_ms = meta.modified().map_or(0, millis);
        let taken_ms = taken_from_name(&name).unwrap_or(modified_ms);
        Some(Self { path: path.to_path_buf(), name, taken_ms, size: meta.len(), modified_ms })
    }
}

/// The result of one scan.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scan {
    /// Newest first.
    pub shots: Vec<Shot>,
    /// Entries that looked like screenshots but could not be read (they are left out).
    pub unreadable: usize,
}

/// The time in a game file name `castanea-<ms>.png`: 13 digits between 2020 and 2100.
pub fn taken_from_name(name: &str) -> Option<i64> {
    let digits = name.strip_prefix("castanea-")?.split('.').next()?;
    if digits.len() != 13 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let ms: i64 = digits.parse().ok()?;
    (1_577_836_800_000..=4_102_444_800_000).contains(&ms).then_some(ms)
}

/// Whether `name` has a screenshot extension.
pub fn is_image_name(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.iter().any(|known| known.eq_ignore_ascii_case(e)))
}

fn millis(time: std::time::SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// The screenshots in `dir`, newest first. Reads only the folder's entries and their metadata (never the images), so a
/// folder of tens of thousands of files is a quick listing; links and sub-folders are ignored. A missing folder is empty.
pub fn scan(dir: &Path) -> Scan {
    let mut scan = Scan::default();
    let Ok(entries) = fs::read_dir(dir) else { return scan };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !is_image_name(&name) {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            scan.unreadable += 1;
            continue;
        };
        if !kind.is_file() {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            scan.unreadable += 1;
            continue;
        };
        let modified_ms = meta.modified().map_or(0, millis);
        let taken_ms = taken_from_name(&name).unwrap_or(modified_ms);
        scan.shots.push(Shot { path: entry.path(), name, taken_ms, size: meta.len(), modified_ms });
    }
    sort(&mut scan.shots);
    scan
}

/// Newest first; shots of one instant by name, so the order never flickers between scans.
pub fn sort(shots: &mut [Shot]) {
    shots.sort_by(|a, b| b.taken_ms.cmp(&a.taken_ms).then_with(|| b.name.cmp(&a.name)));
}

/// Shots of one day: `range` indexes the (sorted) list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub day: Day,
    pub range: Range<usize>,
}

/// Groups a newest-first list by calendar day; `offset(unix_seconds)` is the local time's offset from UTC in seconds
/// (`time::local_offset` on this machine).
pub fn group_by_day(shots: &[Shot], offset: impl Fn(i64) -> i64) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for (index, shot) in shots.iter().enumerate() {
        let seconds = shot.taken_ms.div_euclid(1000);
        let day = time::day_of(seconds + offset(seconds));
        match groups.last_mut() {
            Some(last) if last.day == day => last.range.end = index + 1,
            _ => groups.push(Group { day, range: index..index + 1 }),
        }
    }
    groups
}

/// The newest shot taken from `from_ms` to `to_ms`, both included: the picture of a play session.
pub fn newest_between(shots: &[Shot], from_ms: i64, to_ms: i64) -> Option<&Shot> {
    shots.iter().filter(|s| (from_ms..=to_ms).contains(&s.taken_ms)).max_by_key(|s| s.taken_ms)
}
