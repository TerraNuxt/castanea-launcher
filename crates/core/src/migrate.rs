//! Moving an old launcher home (profiles and several game versions) to the one game folder and the one build. Runs at
//! start, does nothing on a new home, and never deletes a game folder.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::install::{Build, local, update};
use crate::paths::Paths;
use crate::settings::{GameOptions, Settings};
use crate::{Error, Result, version};

/// What a migration did, for one note in the UI.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// The profile whose options and game folder were kept.
    pub profile: Option<String>,
    /// Game folders of other profiles, left where they are.
    pub left: Vec<PathBuf>,
}

/// Migrates `paths` once; `settings` gets the kept profile's options. `None` when there was nothing to do.
pub fn run(paths: &Paths, settings: &mut Settings) -> Result<Option<Report>> {
    let profiles_file = paths.config.join("profiles.json");
    let installs_file = paths.config.join("installs.json");
    let mut report = None;
    if profiles_file.is_file() {
        report = Some(profiles(paths, &profiles_file, settings)?);
        settings.save(paths)?;
        rename(&profiles_file, &profiles_file.with_extension("json.old"))?;
    }
    let versions = paths.data.join("versions");
    if versions.is_dir() {
        builds(paths, &versions)?;
    }
    if installs_file.is_file() {
        fs::remove_file(&installs_file).map_err(|error| Error::io(&installs_file, error))?;
    }
    Ok(report)
}

fn profiles(paths: &Paths, file: &Path, settings: &mut Settings) -> Result<Report> {
    let text = fs::read_to_string(file).map_err(|error| Error::io(file, error))?;
    let root: Value = serde_json::from_str(&text).map_err(|error| Error::json(file.display().to_string(), error))?;
    let list = root.get("profiles").and_then(Value::as_array).cloned().unwrap_or_default();
    let last = fs::read_to_string(paths.settings_file())
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("last_profile").and_then(Value::as_str).map(str::to_owned));
    let id_of = |p: &Value| p.get("id").and_then(Value::as_str).unwrap_or("default").to_owned();
    let Some(kept) = list.iter().find(|p| Some(id_of(p)) == last).or(list.first()).cloned() else {
        return Ok(Report::default());
    };
    if settings.game == GameOptions::default() {
        settings.game = serde_json::from_value(kept.clone()).unwrap_or_default();
    }
    let instances = paths.data.join("instances");
    let folder_of = |p: &Value| match p.get("folder").and_then(Value::as_str) {
        Some(folder) => PathBuf::from(folder),
        None => instances.join(id_of(p)),
    };
    let mut report =
        Report { profile: Some(kept.get("name").and_then(Value::as_str).unwrap_or("").to_owned()), left: vec![] };
    let folder = folder_of(&kept);
    if !paths.game().exists() && folder.is_dir() {
        fs::create_dir_all(&paths.data).map_err(|error| Error::io(&paths.data, error))?;
        // An old `assets` link points at a build that is about to move; prepare() links it again.
        let _ = remove_link(&folder.join("assets"));
        if fs::rename(&folder, paths.game()).is_err() {
            report.left.push(folder);
        }
    }
    for other in list.iter().filter(|p| id_of(p) != id_of(&kept)) {
        let folder = folder_of(other);
        if folder.is_dir() {
            report.left.push(folder);
        }
    }
    Ok(report)
}

/// Keeps the newest downloaded build as the installed one and deletes the other downloaded builds. Folders the user
/// added were registered in place, never copied, so nothing of theirs is in `versions/`.
fn builds(paths: &Paths, versions: &Path) -> Result<()> {
    let current = update::current_dir(paths);
    let mut found: Vec<(Build, PathBuf)> = Vec::new();
    for entry in fs::read_dir(versions).map_err(|error| Error::io(versions, error))?.flatten() {
        let dir = entry.path();
        let Some(old) =
            fs::read_to_string(dir.join("install.json")).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok())
        else {
            continue;
        };
        let Some(binary) = local::find_binary(&dir) else { continue };
        let text = |key: &str| old.get(key).and_then(Value::as_str).unwrap_or("").to_owned();
        let origin =
            |key: &str| old.pointer(&format!("/origin/{key}")).and_then(Value::as_str).unwrap_or("").to_owned();
        let build = Build {
            version: text("version"),
            tag: origin("tag"),
            asset: origin("asset"),
            sha256: old.get("sha256").and_then(Value::as_str).map(str::to_owned),
            root: current.clone(),
            binary: current.join(binary.file_name().unwrap_or_default()),
        };
        found.push((build, dir));
    }
    found.sort_by(|a, b| match (version::parse(&a.0.version), version::parse(&b.0.version)) {
        (Some(x), Some(y)) => x.cmp(&y),
        _ => a.0.version.cmp(&b.0.version),
    });
    if !current.exists()
        && let Some((build, dir)) = found.pop()
    {
        fs::create_dir_all(paths.build()).map_err(|error| Error::io(&paths.build(), error))?;
        rename(&dir, &current)?;
        build.write()?;
    }
    fs::remove_dir_all(versions).map_err(|error| Error::io(versions, error))
}

fn rename(from: &Path, to: &Path) -> Result<()> {
    fs::rename(from, to).map_err(|error| Error::io(from, error))
}

/// Removes a symlink or junction, never a real folder.
fn remove_link(link: &Path) -> std::io::Result<()> {
    let meta = fs::symlink_metadata(link)?;
    if meta.file_type().is_symlink() {
        return fs::remove_file(link).or_else(|_| fs::remove_dir(link));
    }
    #[cfg(windows)]
    if junction::exists(link).unwrap_or(false) {
        return junction::delete(link).and_then(|()| fs::remove_dir(link));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
