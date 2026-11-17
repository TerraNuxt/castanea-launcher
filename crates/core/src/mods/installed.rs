//! The mods of one profile: `mods/` holds the enabled ones, `mods/.disabled/` the rest (the game skips dot names).

use std::fs;
use std::path::{Path, PathBuf};

use super::manifest::{self, Manifest};
use crate::archive;
use crate::{Error, Result};

pub const DISABLED_DIR: &str = ".disabled";
const MANIFEST_MAX: u64 = 256 << 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Packaging {
    Folder,
    Zip,
}

#[derive(Clone, Debug)]
pub struct InstalledMod {
    pub path: PathBuf,
    /// The folder or zip name in `mods/`.
    pub file_name: String,
    pub packaging: Packaging,
    pub enabled: bool,
    /// The manifest, or why it could not be read.
    pub manifest: std::result::Result<Manifest, String>,
}

impl InstalledMod {
    /// The id from the manifest, or the file name for a broken package.
    pub fn id(&self) -> &str {
        match &self.manifest {
            Ok(manifest) => &manifest.id,
            Err(_) => &self.file_name,
        }
    }
}

/// Every package in `mods/` and `mods/.disabled/`, sorted by id. A missing folder is no mods.
pub fn list(mods_dir: &Path) -> Result<Vec<InstalledMod>> {
    let mut out = Vec::new();
    scan(mods_dir, true, &mut out)?;
    scan(&mods_dir.join(DISABLED_DIR), false, &mut out)?;
    out.sort_by(|a, b| a.id().cmp(b.id()).then(a.file_name.cmp(&b.file_name)));
    Ok(out)
}

fn scan(dir: &Path, enabled: bool, out: &mut Vec<InstalledMod>) -> Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(Error::io(dir, error)),
    };
    for entry in entries {
        let entry = entry.map_err(|error| Error::io(dir, error))?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if file_name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let packaging = if path.is_dir() {
            Packaging::Folder
        } else if file_name.to_ascii_lowercase().ends_with(".zip") {
            Packaging::Zip
        } else {
            continue;
        };
        let manifest = read_manifest(&path, packaging);
        out.push(InstalledMod { path, file_name, packaging, enabled, manifest });
    }
    Ok(())
}

pub fn read_manifest(path: &Path, packaging: Packaging) -> std::result::Result<Manifest, String> {
    let bytes = match packaging {
        Packaging::Folder => {
            let file = path.join(manifest::FILE);
            if fs::metadata(&file).map_err(|error| format!("mod.json: {error}"))?.len() > MANIFEST_MAX {
                return Err("mod.json: too large".to_owned());
            }
            fs::read(&file).map_err(|error| format!("mod.json: {error}"))?
        }
        Packaging::Zip => archive::read_file(path, manifest::FILE, MANIFEST_MAX)
            .map_err(|error| error.to_string())?
            .ok_or("no mod.json at the top of the zip")?,
    };
    Manifest::parse(&String::from_utf8_lossy(&bytes))
}

/// Moves the package between `mods/` and `mods/.disabled/`; returns its new path.
pub fn set_enabled(mods_dir: &Path, package: &InstalledMod, enabled: bool) -> Result<PathBuf> {
    if package.enabled == enabled {
        return Ok(package.path.clone());
    }
    let target_dir = if enabled { mods_dir.to_path_buf() } else { mods_dir.join(DISABLED_DIR) };
    fs::create_dir_all(&target_dir).map_err(|error| Error::io(&target_dir, error))?;
    let target = target_dir.join(&package.file_name);
    if target.exists() {
        return Err(Error::invalid(format!("{} already exists", target.display())));
    }
    fs::rename(&package.path, &target).map_err(|error| Error::io(&package.path, error))?;
    Ok(target)
}

/// Deletes the package's folder or zip.
pub fn remove(package: &InstalledMod) -> Result<()> {
    let result = match package.packaging {
        Packaging::Folder => fs::remove_dir_all(&package.path),
        Packaging::Zip => fs::remove_file(&package.path),
    };
    result.map_err(|error| Error::io(&package.path, error))
}

/// What a package declares and holds, for the "enable this mod?" question.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Declared {
    pub files: usize,
    pub bytes: u64,
    /// File kinds by extension with their count, such as `json 12`.
    pub kinds: Vec<(String, usize)>,
}

pub fn declared(package: &InstalledMod) -> Result<Declared> {
    let mut names: Vec<(String, u64)> = Vec::new();
    match package.packaging {
        Packaging::Zip => {
            let contents = archive::check(&package.path, archive::Limits::MOD)?;
            names.extend(contents.files.into_iter().map(|name| (name, 0)));
            let mut out = summarise(&names);
            out.bytes = contents.bytes;
            return Ok(out);
        }
        Packaging::Folder => walk(&package.path, &package.path, &mut names)?,
    }
    Ok(summarise(&names))
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, u64)>) -> Result<()> {
    for entry in fs::read_dir(dir).map_err(|error| Error::io(dir, error))? {
        let entry = entry.map_err(|error| Error::io(dir, error))?;
        let kind = entry.file_type().map_err(|error| Error::io(&entry.path(), error))?;
        if kind.is_dir() {
            walk(root, &entry.path(), out)?;
        } else if kind.is_file() {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let name = entry.path().strip_prefix(root).unwrap_or(&entry.path()).to_string_lossy().into_owned();
            out.push((name, size));
        }
        if out.len() > archive::Limits::MOD.max_entries {
            return Err(Error::refused(format!("{}: too many files", root.display())));
        }
    }
    Ok(())
}

fn summarise(names: &[(String, u64)]) -> Declared {
    let mut kinds: Vec<(String, usize)> = Vec::new();
    for (name, _) in names {
        let ext = Path::new(name).extension().map_or("-".to_owned(), |e| e.to_string_lossy().to_ascii_lowercase());
        match kinds.iter_mut().find(|(k, _)| *k == ext) {
            Some((_, count)) => *count += 1,
            None => kinds.push((ext, 1)),
        }
    }
    kinds.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    Declared { files: names.len(), bytes: names.iter().map(|(_, size)| size).sum(), kinds }
}
