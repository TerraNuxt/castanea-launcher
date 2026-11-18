//! What the launcher remembers about the graphics backend, in `<data>/backend.json`: the probe results (per game
//! version and GPU), the Vulkan starts that died, and the last start for the diagnostics.

use serde::{Deserialize, Serialize};

use super::probe::{Probe, Report, Unsupported};
use super::{FRESH_SECONDS, Reason, SHORT_FRESH_SECONDS};
use crate::Result;
use crate::json_file;
use crate::paths::Paths;

const KEEP_PROBES: usize = 8;
const KEEP_DIED: usize = 16;

/// A probe result as stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Stored {
    Report(Report),
    Unsupported { why: Unsupported },
    Crashed { code: Option<i32> },
}

impl Stored {
    pub fn from_probe(probe: Probe) -> Self {
        match probe {
            Probe::Report(report) => Self::Report(report),
            Probe::Unsupported(why) => Self::Unsupported { why },
            Probe::Crashed(code) => Self::Crashed { code },
        }
    }

    /// A timeout may be a hiccup: it is trusted for a day only, like a result for an unknown GPU.
    fn fresh_for(&self, gpu: &str) -> u64 {
        let short = gpu.is_empty() || matches!(self, Self::Unsupported { why: Unsupported::Timeout });
        if short { SHORT_FRESH_SECONDS } else { FRESH_SECONDS }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeEntry {
    pub version: String,
    pub gpu: String,
    pub at: u64,
    pub result: Stored,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Died {
    pub version: String,
    pub gpu: String,
}

/// The last start, for About.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LastLaunch {
    pub version: String,
    /// `vulkan`, `wgpu` or `auto`.
    pub backend: String,
    pub reason: Reason,
    pub adapter: String,
    pub driver: String,
    pub at: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BackendState {
    pub probes: Vec<ProbeEntry>,
    pub vulkan_died: Vec<Died>,
    pub last: Option<LastLaunch>,
}

impl BackendState {
    /// The saved state; a missing or broken file is an empty state (it only caches).
    pub fn load(paths: &Paths) -> Self {
        json_file::load_or_default(&paths.backend_file()).unwrap_or_default()
    }

    pub fn save(&self, paths: &Paths) -> Result<()> {
        json_file::save(&paths.backend_file(), self)
    }

    /// The cached probe for this game version and GPU, if it is not too old.
    pub fn fresh(&self, version: &str, gpu: &str, now: u64) -> Option<&Stored> {
        let entry = self.probes.iter().find(|e| e.version == version && e.gpu == gpu)?;
        let age = now.saturating_sub(entry.at);
        (age < entry.result.fresh_for(gpu)).then_some(&entry.result)
    }

    pub fn remember(&mut self, version: &str, gpu: &str, at: u64, result: Stored) {
        self.probes.retain(|e| !(e.version == version && e.gpu == gpu));
        self.probes.push(ProbeEntry { version: version.to_owned(), gpu: gpu.to_owned(), at, result });
        let extra = self.probes.len().saturating_sub(KEEP_PROBES);
        self.probes.drain(..extra);
    }

    pub fn vulkan_died(&self, version: &str, gpu: &str) -> bool {
        self.vulkan_died.iter().any(|d| d.version == version && d.gpu == gpu)
    }

    pub fn mark_vulkan_died(&mut self, version: &str, gpu: &str) {
        if self.vulkan_died(version, gpu) {
            return;
        }
        self.vulkan_died.push(Died { version: version.to_owned(), gpu: gpu.to_owned() });
        let extra = self.vulkan_died.len().saturating_sub(KEEP_DIED);
        self.vulkan_died.drain(..extra);
    }
}
