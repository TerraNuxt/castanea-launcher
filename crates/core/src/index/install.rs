//! Installing and updating mods from an index: download, SHA-256 check, manifest check, then a disabled import.

use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use super::format::{Index, IndexMod, IndexVersion};
use crate::download::{Client, Progress, Request};
use crate::mods::import;
use crate::mods::installed::{self, InstalledMod};
use crate::version;
use crate::{Error, Result};

/// Mod archives larger than this are refused, whatever the index says.
pub const MAX_MOD: u64 = 64 << 20;

/// Downloads `version` of `item` and installs it disabled; an installed copy with the same id is replaced and keeps
/// its enabled state.
pub fn install(
    client: &Client,
    item: &IndexMod,
    version: &IndexVersion,
    mods_dir: &Path,
    downloads_dir: &Path,
    progress: Progress<'_>,
    cancel: &AtomicBool,
) -> Result<InstalledMod> {
    if version.size > MAX_MOD {
        return Err(Error::refused(format!("{} {}: {} bytes is over the limit", item.id, version.version, version.size)));
    }
    let zip = downloads_dir.join(format!("{}.zip", version.sha256));
    let request =
        Request { url: &version.url, sha256: &version.sha256, size: Some(version.size), max_bytes: MAX_MOD, headers: Vec::new() };
    client.download(&request, &zip, progress, cancel)?;
    let result = install_checked(item, version, mods_dir, &zip);
    let _ = fs::remove_file(&zip);
    result
}

fn install_checked(item: &IndexMod, version: &IndexVersion, mods_dir: &Path, zip: &Path) -> Result<InstalledMod> {
    let manifest = installed::read_manifest(zip, installed::Packaging::Zip).map_err(Error::invalid)?;
    if manifest.id != item.id {
        return Err(Error::refused(format!("the archive is mod `{}`, the index says `{}`", manifest.id, item.id)));
    }
    if manifest.version != version.version {
        return Err(Error::refused(format!(
            "{}: the archive is version {}, the index says {}",
            item.id, manifest.version, version.version
        )));
    }
    let was_enabled = installed::list(mods_dir)?.iter().any(|m| m.id() == item.id && m.enabled);
    let mut package = import::install_zip(mods_dir, zip, true)?;
    if was_enabled {
        package.path = installed::set_enabled(mods_dir, &package, true)?;
        package.enabled = true;
    }
    Ok(package)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Update {
    pub id: String,
    pub installed: String,
    pub available: String,
}

/// Installed mods for which the index has a newer version that fits `game`.
pub fn updates(mods: &[InstalledMod], index: &Index, game: Option<&str>) -> Vec<Update> {
    mods.iter()
        .filter_map(|package| {
            let manifest = package.manifest.as_ref().ok()?;
            let best = index.get(&manifest.id)?.best_version(game)?;
            version::newer(&best.version, &manifest.version).then(|| Update {
                id: manifest.id.clone(),
                installed: manifest.version.clone(),
                available: best.version.clone(),
            })
        })
        .collect()
}

/// Index dependencies of `item` that are not installed, with their best versions, deepest first.
pub fn missing_dependencies<'a>(
    index: &'a Index,
    item: &IndexMod,
    version: &IndexVersion,
    mods: &[InstalledMod],
    game: Option<&str>,
) -> Vec<(&'a IndexMod, &'a IndexVersion)> {
    let mut out: Vec<(&IndexMod, &IndexVersion)> = Vec::new();
    let mut stack: Vec<String> = version.depends.keys().cloned().collect();
    let mut seen = std::collections::HashSet::from([item.id.clone()]);
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) || mods.iter().any(|m| m.id() == id) {
            continue;
        }
        let Some(dep) = index.get(&id) else { continue };
        let Some(best) = dep.best_version(game) else { continue };
        stack.extend(best.depends.keys().cloned());
        out.push((dep, best));
    }
    out.reverse();
    out
}
