//! Importing a mod from a zip or a folder: checked, then put into `mods/.disabled/` until the user enables it.

use std::fs;
use std::path::{Path, PathBuf};

use super::installed::{self, DISABLED_DIR, InstalledMod, Packaging};
use super::manifest::{self, Manifest};
use crate::archive::{self, Limits};
use crate::{Error, Result};

/// Imports `source` (a `.zip` or a folder with `mod.json`). `replace` overwrites a package with the same id.
pub fn import(mods_dir: &Path, source: &Path, replace: bool) -> Result<InstalledMod> {
    let (manifest, packaging) = if source.is_dir() {
        let manifest = installed::read_manifest(source, Packaging::Folder).map_err(Error::invalid)?;
        check_folder(source)?;
        (manifest, Packaging::Folder)
    } else {
        let contents = archive::check(source, Limits::MOD)?;
        if !contents.files.iter().any(|name| name == manifest::FILE) {
            return Err(Error::invalid(format!("{}: mod.json must be at the top of the zip", source.display())));
        }
        (installed::read_manifest(source, Packaging::Zip).map_err(Error::invalid)?, Packaging::Zip)
    };
    let existing: Vec<InstalledMod> = installed::list(mods_dir)?.into_iter().filter(|m| m.id() == manifest.id).collect();
    if !existing.is_empty() && !replace {
        return Err(Error::invalid(format!("a mod with the id `{}` is already installed", manifest.id)));
    }
    let disabled = mods_dir.join(DISABLED_DIR);
    let target = target_path(&disabled, &manifest, packaging);
    let temp = disabled.join(format!(".import-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp);
    let _ = fs::remove_file(&temp);
    fs::create_dir_all(&disabled).map_err(|error| Error::io(&disabled, error))?;
    let copied = match packaging {
        Packaging::Folder => copy_dir(source, &temp),
        Packaging::Zip => fs::copy(source, &temp).map(|_| ()).map_err(|error| Error::io(&temp, error)),
    };
    if let Err(error) = copied {
        let _ = fs::remove_dir_all(&temp);
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    for old in &existing {
        installed::remove(old)?;
    }
    fs::rename(&temp, &target).map_err(|error| Error::io(&target, error))?;
    Ok(InstalledMod {
        file_name: target.file_name().unwrap_or_default().to_string_lossy().into_owned(),
        path: target,
        packaging,
        enabled: false,
        manifest: Ok(manifest),
    })
}

/// Installs a downloaded zip whose hash was already checked; the same rules as an import.
pub fn install_zip(mods_dir: &Path, zip: &Path, replace: bool) -> Result<InstalledMod> {
    import(mods_dir, zip, replace)
}

fn target_path(disabled: &Path, manifest: &Manifest, packaging: Packaging) -> PathBuf {
    match packaging {
        Packaging::Folder => disabled.join(&manifest.id),
        Packaging::Zip => disabled.join(format!("{}.zip", manifest.id)),
    }
}

/// No links, at most the mod limits of files and bytes.
fn check_folder(root: &Path) -> Result<()> {
    let mut files = 0usize;
    let mut bytes = 0u64;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).map_err(|error| Error::io(&dir, error))? {
            let entry = entry.map_err(|error| Error::io(&dir, error))?;
            let kind = entry.file_type().map_err(|error| Error::io(&entry.path(), error))?;
            if kind.is_symlink() {
                return Err(Error::refused(format!("{}: links are not allowed in a mod", entry.path().display())));
            }
            if kind.is_dir() {
                stack.push(entry.path());
            } else {
                files += 1;
                bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
        if files > Limits::MOD.max_entries || bytes > Limits::MOD.max_bytes {
            return Err(Error::refused(format!("{}: larger than a mod may be", root.display())));
        }
    }
    Ok(())
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to).map_err(|error| Error::io(to, error))?;
    for entry in fs::read_dir(from).map_err(|error| Error::io(from, error))? {
        let entry = entry.map_err(|error| Error::io(from, error))?;
        let target = to.join(entry.file_name());
        if entry.file_type().map_err(|error| Error::io(&entry.path(), error))?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target).map_err(|error| Error::io(&target, error))?;
        }
    }
    Ok(())
}
