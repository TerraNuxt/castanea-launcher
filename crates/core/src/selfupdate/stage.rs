//! Downloading a launcher release and unpacking it into the staging folder.

use std::fs;
use std::sync::atomic::AtomicBool;

use super::state::{Files, Staged};
use crate::archive::{self, Limits};
use crate::download::{Client, Progress};
use crate::install::{Release, ReleaseApi, notes};
use crate::paths::Paths;
use crate::{Error, Result};

/// A launcher archive is the executable (and maybe a licence), a few MB.
const MAX_ARCHIVE: u64 = 256 << 20;
const LIMITS: Limits = Limits { max_entries: 64, max_bytes: 512 << 20 };

/// The release, downloaded (resumed if it was interrupted), hash and signature checked, and unpacked with the same
/// path and size checks as a game build. A staged copy of this very release is reused. The installed launcher is not
/// touched.
pub fn stage(
    api: &ReleaseApi,
    client: &Client,
    release: &Release,
    paths: &Paths,
    progress: Progress<'_>,
    cancel: &AtomicBool,
) -> Result<Staged> {
    let files = Files::new(paths);
    if let Some(staged) = files.staged()
        && staged.version == release.version
        && staged.sha256 == release.sha256
    {
        return Ok(staged);
    }
    files.clear_staged();
    fs::create_dir_all(&files.dir).map_err(|error| Error::io(&files.dir, error))?;
    fs::create_dir_all(paths.downloads()).map_err(|error| Error::io(&paths.downloads(), error))?;
    let zip = api.fetch_verified(client, release, &paths.downloads(), MAX_ARCHIVE, progress, cancel)?;
    let unpacked = archive::extract(&zip, &files.unpack_dir(), LIMITS);
    let _ = fs::remove_file(&zip);
    unpacked?;
    let name = api.platform.launcher_binary();
    let binary = files.unpack_dir().join(name);
    let empty = fs::metadata(&binary).map_or(true, |m| !m.is_file() || m.len() == 0);
    if empty {
        files.clear_staged();
        return Err(Error::invalid(format!("launcher {}: the archive must hold {name} at its top", release.version)));
    }
    let staged = Staged {
        version: release.version.clone(),
        channel: release.channel.clone(),
        sha256: release.sha256.clone(),
        notes: notes::plain(&release.notes),
        binary: files.staged_dir().join(name),
    };
    let recorded = fs::rename(files.unpack_dir(), files.staged_dir())
        .map_err(|error| Error::io(&files.staged_dir(), error))
        .and_then(|()| files.set_staged(&staged));
    if let Err(error) = recorded {
        files.clear_staged();
        return Err(error);
    }
    Ok(staged)
}
