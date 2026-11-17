//! `installs.json`: the game builds the launcher knows, managed (in `data/versions`) or registered in place.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::json_file;
use crate::paths::{Paths, safe_name};
use crate::{Error, Result};

/// Where a build came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Origin {
    Github { repo: String, tag: String, asset: String },
    /// A folder the user picked; never copied or changed.
    Local,
    /// Found on disk by `detect`.
    Detected,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Build {
    pub id: String,
    /// The game version, `local` when unknown.
    pub version: String,
    /// The game folder: it holds `assets/`.
    pub root: PathBuf,
    pub binary: PathBuf,
    pub origin: Origin,
    /// The checked SHA-256 of the archive it came from.
    #[serde(default)]
    pub sha256: Option<String>,
    /// True when the launcher owns the files (in `data/versions`) and may delete them.
    pub managed: bool,
}

impl Build {
    pub fn assets(&self) -> PathBuf {
        self.root.join("assets")
    }

    /// The binary and `assets/` still exist.
    pub fn is_present(&self) -> bool {
        self.binary.is_file() && self.assets().is_dir()
    }

    /// The managed build's description, written next to it.
    pub fn write_install_file(&self) -> Result<()> {
        json_file::save(&self.root.join(INSTALL_FILE), self)
    }
}

pub const INSTALL_FILE: &str = "install.json";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registry {
    pub builds: Vec<Build>,
}

impl Registry {
    pub fn load(paths: &Paths) -> Result<Self> {
        json_file::load_or_default(&paths.installs_file())
    }

    pub fn save(&self, paths: &Paths) -> Result<()> {
        json_file::save(&paths.installs_file(), self)
    }

    pub fn get(&self, id: &str) -> Option<&Build> {
        self.builds.iter().find(|b| b.id == id)
    }

    /// An id from `wanted` that no build has yet (`wanted`, `wanted-2`, ...).
    pub fn free_id(&self, wanted: &str) -> String {
        let base = safe_name(wanted);
        let mut id = base.clone();
        let mut n = 2;
        while self.get(&id).is_some() {
            id = format!("{base}-{n}");
            n += 1;
        }
        id
    }

    /// Adds `build`, or replaces the build with its id.
    pub fn put(&mut self, build: Build) {
        match self.builds.iter_mut().find(|b| b.id == build.id) {
            Some(slot) => *slot = build,
            None => self.builds.push(build),
        }
        self.builds.sort_by(|a, b| a.id.cmp(&b.id));
    }

    /// Forgets the build; a managed build's folder is deleted, a registered one is left alone.
    pub fn remove(&mut self, paths: &Paths, id: &str) -> Result<Option<Build>> {
        let Some(index) = self.builds.iter().position(|b| b.id == id) else { return Ok(None) };
        let build = self.builds.remove(index);
        if build.managed && is_inside(&build.root, &paths.versions()) && build.root.exists() {
            fs::remove_dir_all(&build.root).map_err(|error| Error::io(&build.root, error))?;
        }
        Ok(Some(build))
    }

    /// Adds managed builds found in `data/versions` that the file does not list (after a lost `installs.json`).
    pub fn adopt_managed(&mut self, paths: &Paths) {
        let Ok(entries) = fs::read_dir(paths.versions()) else { return };
        for entry in entries.flatten() {
            let file = entry.path().join(INSTALL_FILE);
            let Ok(build) = json_file::load_or_default::<Option<Build>>(&file) else { continue };
            if let Some(mut build) = build
                && self.get(&build.id).is_none()
            {
                build.root = entry.path();
                build.binary = build.root.join(build.binary.file_name().unwrap_or_default());
                self.put(build);
            }
        }
    }
}

fn is_inside(path: &Path, dir: &Path) -> bool {
    match (path.canonicalize(), dir.canonicalize()) {
        (Ok(path), Ok(dir)) => path.starts_with(&dir) && path != dir,
        _ => false,
    }
}
