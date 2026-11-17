//! A game build on disk: a folder with the binary next to `assets/`, described by its `install.json`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::local;
use crate::Result;
use crate::json_file;

pub const INSTALL_FILE: &str = "install.json";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Build {
    /// The game version, `local` when unknown.
    pub version: String,
    /// `stable` or `beta`; empty for a local folder.
    #[serde(default)]
    pub channel: String,
    #[serde(default)]
    pub released_at: String,
    /// What changed in this release, as plain text.
    #[serde(default)]
    pub notes: String,
    /// The checked SHA-256 of the archive it came from.
    #[serde(default)]
    pub sha256: Option<String>,
    /// The game folder: it holds `assets/`.
    #[serde(skip)]
    pub root: PathBuf,
    #[serde(skip)]
    pub binary: PathBuf,
}

impl Build {
    pub fn assets(&self) -> PathBuf {
        self.root.join("assets")
    }

    /// The binary and `assets/` still exist.
    pub fn is_present(&self) -> bool {
        self.binary.is_file() && self.assets().is_dir()
    }

    /// The build in `root`, from its `install.json`; `None` when the file or the game is missing.
    pub fn read(root: &Path) -> Option<Self> {
        let mut build: Option<Self> = json_file::load_or_default(&root.join(INSTALL_FILE)).ok()?;
        let build = build.as_mut()?;
        build.root = root.to_path_buf();
        build.binary = local::find_binary(root)?;
        Some(build.clone())
    }

    pub fn write(&self) -> Result<()> {
        json_file::save(&self.root.join(INSTALL_FILE), self)
    }
}
