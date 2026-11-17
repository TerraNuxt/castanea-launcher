//! Reading and atomically writing the launcher's JSON files.

use std::fs;
use std::io::Write;
use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{Error, Result};

/// The value in `path`, or the default when the file does not exist.
pub fn load_or_default<T: DeserializeOwned + Default>(path: &Path) -> Result<T> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map_err(|error| Error::json(path.display().to_string(), error)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(error) => Err(Error::io(path, error)),
    }
}

/// Writes pretty JSON to a temp file next to `path` and renames it over `path`.
pub fn save<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut text =
        serde_json::to_string_pretty(value).map_err(|error| Error::json(path.display().to_string(), error))?;
    text.push('\n');
    write_atomic(path, text.as_bytes())
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| Error::io(parent, error))?;
    }
    let temp = path.with_extension(format!("tmp{}", std::process::id()));
    let mut file = fs::File::create(&temp).map_err(|error| Error::io(&temp, error))?;
    file.write_all(bytes).and_then(|()| file.sync_all()).map_err(|error| Error::io(&temp, error))?;
    drop(file);
    fs::rename(&temp, path).map_err(|error| {
        let _ = fs::remove_file(&temp);
        Error::io(path, error)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_is_the_default_and_a_saved_one_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/b.json");
        let empty: Vec<String> = load_or_default(&path).unwrap();
        assert!(empty.is_empty());
        save(&path, &vec!["x".to_owned()]).unwrap();
        let back: Vec<String> = load_or_default(&path).unwrap();
        assert_eq!(back, ["x"]);
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1, "no temp file left");
    }

    #[test]
    fn broken_json_names_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        fs::write(&path, "{").unwrap();
        let error = load_or_default::<Vec<String>>(&path).unwrap_err().to_string();
        assert!(error.contains("bad.json"), "{error}");
    }
}
