//! The launcher update's files in its data folder and the start-up state machine over them.
//!
//! `update-pending` is written before the new launcher is started and holds who replaced whom. The new launcher stamps
//! `started_at` when it starts and the marker is deleted after its first frame. A marker that is still stamped at the
//! next start means the new launcher never drew a frame: it is rolled back and its version goes on the skip list.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::paths::Paths;
use crate::{Result, json_file, version};

pub const MARKER: &str = "update-pending";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pending {
    /// The version that was replaced and the one that replaced it.
    pub from: String,
    pub to: String,
    pub applied_at: u64,
    /// Set when the new launcher started; `None` until then.
    #[serde(default)]
    pub started_at: Option<u64>,
}

/// What the running launcher does about the marker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// No update is pending.
    Nothing,
    /// First start after an update: stamp `started_at` and run.
    FirstStart(Pending),
    /// The new launcher started before and never reached its first frame: restore the old one.
    RollBack(Pending),
    /// The marker belongs to another version than the running one; forget it.
    Stale,
}

pub fn decide(running: &str, pending: Option<Pending>, now: u64) -> Decision {
    let Some(pending) = pending else { return Decision::Nothing };
    let same = match (version::parse(&pending.to), version::parse(running)) {
        (Some(a), Some(b)) => a == b,
        _ => pending.to == running,
    };
    if !same {
        return Decision::Stale;
    }
    match pending.started_at {
        None => Decision::FirstStart(Pending { started_at: Some(now), ..pending }),
        Some(_) => Decision::RollBack(pending),
    }
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// A staged launcher: unpacked, signature checked, waiting to be put in place.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Staged {
    pub version: String,
    pub channel: String,
    /// The SHA-256 of the archive it came from.
    pub sha256: String,
    pub notes: String,
    pub binary: PathBuf,
}

/// The launcher update's folder, `<data>/launcher-update`.
#[derive(Clone, Debug)]
pub struct Files {
    pub dir: PathBuf,
}

impl Files {
    pub fn new(paths: &Paths) -> Self {
        Self { dir: paths.data.join("launcher-update") }
    }

    pub fn marker(&self) -> PathBuf {
        self.dir.join(MARKER)
    }

    pub fn skip_file(&self) -> PathBuf {
        self.dir.join("skip.json")
    }

    pub fn staged_dir(&self) -> PathBuf {
        self.dir.join("staged")
    }

    pub fn unpack_dir(&self) -> PathBuf {
        self.dir.join("unpacking")
    }

    fn staged_file(&self) -> PathBuf {
        self.dir.join("staged.json")
    }

    /// The marker; a missing or unreadable one is no marker.
    pub fn pending(&self) -> Option<Pending> {
        fs::read_to_string(self.marker()).ok().and_then(|text| serde_json::from_str(&text).ok())
    }

    pub fn set_pending(&self, pending: &Pending) -> Result<()> {
        json_file::save(&self.marker(), pending)
    }

    pub fn clear_pending(&self) {
        let _ = fs::remove_file(self.marker());
    }

    /// Versions that failed to start and are never installed again.
    pub fn skipped(&self) -> Vec<String> {
        json_file::load_or_default(&self.skip_file()).unwrap_or_default()
    }

    pub fn skip(&self, version: &str) -> Result<()> {
        let mut skipped = self.skipped();
        if !skipped.iter().any(|v| v == version) {
            skipped.push(version.to_owned());
            json_file::save(&self.skip_file(), &skipped)?;
        }
        Ok(())
    }

    pub fn is_skipped(&self, version: &str) -> bool {
        self.skipped().iter().any(|v| v == version)
    }

    pub fn staged(&self) -> Option<Staged> {
        let text = fs::read_to_string(self.staged_file()).ok()?;
        let staged: Staged = serde_json::from_str(&text).ok()?;
        staged.binary.is_file().then_some(staged)
    }

    pub fn set_staged(&self, staged: &Staged) -> Result<()> {
        json_file::save(&self.staged_file(), staged)
    }

    /// Removes the staged launcher and its record.
    pub fn clear_staged(&self) {
        let _ = fs::remove_dir_all(self.staged_dir());
        let _ = fs::remove_dir_all(self.unpack_dir());
        let _ = fs::remove_file(self.staged_file());
    }
}
