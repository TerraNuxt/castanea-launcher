//! A game build in a local folder: a release folder (binary next to `assets/`) or a checkout (`target/release`).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use super::registry::{Build, Origin, Registry};
use super::source::{GameSource, RemoteBuild};
use crate::download::{Client, Progress};
use crate::{Error, Result};

pub const BINARY: &str = if cfg!(windows) { "castanea.exe" } else { "castanea" };
pub const VERSION_FILE: &str = "version.txt";

/// The game binary for a game folder that holds `assets/`: next to it, or in a checkout's `target/release`.
pub fn find_binary(root: &Path) -> Option<PathBuf> {
    if !root.join("assets").is_dir() {
        return None;
    }
    [root.join(BINARY), root.join("target/release").join(BINARY), root.join("bin").join(BINARY)]
        .into_iter()
        .find(|path| path.is_file())
}

/// `version.txt` next to the binary or in the folder, else the checkout's Cargo.toml version, else `local`.
pub fn read_version(root: &Path, binary: &Path) -> String {
    let candidates = [binary.parent().map(|p| p.join(VERSION_FILE)), Some(root.join(VERSION_FILE))];
    for file in candidates.into_iter().flatten() {
        if let Ok(text) = fs::read_to_string(file)
            && let Some(line) = text.lines().next().map(str::trim).filter(|l| !l.is_empty() && l.len() <= 64)
        {
            return line.to_owned();
        }
    }
    if let Ok(text) = fs::read_to_string(root.join("Cargo.toml"))
        && text.contains("name = \"castanea\"")
        && let Some(version) = text.lines().find_map(|l| l.trim().strip_prefix("version = \""))
    {
        return version.trim_end_matches('"').to_owned();
    }
    "local".to_owned()
}

/// A build for `folder`, not yet in the registry; an error says what is missing.
pub fn inspect(registry: &Registry, folder: &Path, origin: Origin) -> Result<Build> {
    let root = folder.canonicalize().map_err(|error| Error::io(folder, error))?;
    let binary = find_binary(&root).ok_or_else(|| {
        Error::invalid(format!(
            "{}: no game here (needs assets/ and {BINARY} or target/release/{BINARY})",
            root.display()
        ))
    })?;
    if let Some(known) = registry.builds.iter().find(|b| b.root == root) {
        return Ok(known.clone());
    }
    let version = read_version(&root, &binary);
    let name = root.file_name().map_or("game".into(), |n| n.to_string_lossy().into_owned());
    Ok(Build {
        id: registry.free_id(&format!("local-{name}")),
        version,
        root,
        binary,
        origin,
        sha256: None,
        managed: false,
    })
}

/// The folder as a source with one build; installing registers it in place.
pub struct LocalFolder {
    pub folder: PathBuf,
}

impl GameSource for LocalFolder {
    fn name(&self) -> String {
        self.folder.display().to_string()
    }

    fn list(&self, _client: &Client) -> Result<Vec<RemoteBuild>> {
        let build = inspect(&Registry::default(), &self.folder, Origin::Local)?;
        Ok(vec![RemoteBuild {
            id: build.id,
            version: build.version,
            title: self.folder.display().to_string(),
            published: String::new(),
            notes: String::new(),
            prerelease: false,
            asset: None,
        }])
    }

    fn install(
        &self,
        _client: &Client,
        _build: &RemoteBuild,
        _versions_dir: &Path,
        _downloads_dir: &Path,
        _progress: Progress<'_>,
        _cancel: &AtomicBool,
    ) -> Result<Build> {
        inspect(&Registry::default(), &self.folder, Origin::Local)
    }
}
