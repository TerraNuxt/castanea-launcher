//! What the gallery may do to a file: rename, delete, trash. Only files directly in the screenshots folder.

use std::fs;
use std::path::{Path, PathBuf};

use super::{is_image_name, trash};
use crate::{Error, Result};

/// The longest file name stem a rename accepts, in characters.
pub const MAX_STEM: usize = 100;

/// Refuses a path that is not an image file directly in `dir`: the gallery never touches anything else.
pub fn check(dir: &Path, path: &Path) -> Result<()> {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let inside = path.parent().is_some_and(|parent| parent == dir);
    if !inside || !is_image_name(name) {
        return Err(Error::refused(format!("{} is not a screenshot in {}", path.display(), dir.display())));
    }
    let kind = fs::symlink_metadata(path).map_err(|e| Error::io(path, e))?;
    if !kind.file_type().is_file() {
        return Err(Error::refused(format!("{} is not a plain file", path.display())));
    }
    Ok(())
}

const RESERVED: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8", "com9", "lpt1", "lpt2",
    "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// The new name without its extension, checked: not empty, at most `MAX_STEM` characters, no path separators or characters
/// Windows refuses, no control characters, not starting with a dot, not ending with a dot or space, not a device name.
pub fn clean_stem(stem: &str) -> Result<String> {
    let stem = stem.trim();
    let bad = |c: char| c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|');
    if stem.is_empty() {
        return Err(Error::invalid("the name is empty"));
    }
    if stem.chars().count() > MAX_STEM {
        return Err(Error::invalid(format!("the name is longer than {MAX_STEM} characters")));
    }
    if stem.chars().any(bad) || stem.starts_with('.') || stem.ends_with('.') {
        return Err(Error::invalid("the name has a character a file name cannot have"));
    }
    if RESERVED.contains(&stem.split('.').next().unwrap_or("").to_ascii_lowercase().as_str()) {
        return Err(Error::invalid("that name is reserved by the system"));
    }
    Ok(stem.to_owned())
}

/// Renames `path` to `stem` plus its own extension in the same folder and returns the new path. An existing file is never
/// replaced; a change of letter case only is allowed.
pub fn rename(dir: &Path, path: &Path, stem: &str) -> Result<PathBuf> {
    check(dir, path)?;
    let stem = clean_stem(stem)?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("png");
    let target = dir.join(format!("{stem}.{ext}"));
    let same_name = target.file_name().is_some_and(|n| n.eq_ignore_ascii_case(path.file_name().unwrap_or_default()));
    if target.exists() && !same_name {
        return Err(Error::invalid(format!("{} exists already", target.display())));
    }
    fs::rename(path, &target).map_err(|e| Error::io(path, e))?;
    Ok(target)
}

/// Deletes the file for good.
pub fn delete(dir: &Path, path: &Path) -> Result<()> {
    check(dir, path)?;
    fs::remove_file(path).map_err(|e| Error::io(path, e))
}

/// Moves the file to the system's trash; the text says why it could not (then the caller may ask to delete for good).
pub fn trash(dir: &Path, path: &Path) -> std::result::Result<(), String> {
    check(dir, path).map_err(|e| e.to_string())?;
    trash::move_to_trash(path)
}
