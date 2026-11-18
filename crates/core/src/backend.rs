//! Choosing the game's graphics backend: Vulkan first, but only when a probe of the installed game says it works,
//! else wgpu; the answer is cached per game version and GPU, and a Vulkan start that dies at once is remembered.
//! The contract with the game is in docs/game-launch.md.

pub mod gpu;
pub mod probe;
pub mod state;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::install::Build;
use crate::launch::{Outcome, prepare};
use crate::paths::Paths;
use crate::settings::{GameOptions, Graphics};
use crate::{Error, Result};

pub use probe::{BackendReport, Probe, Report, Unsupported};
pub use state::{BackendState, LastLaunch, ProbeEntry, Stored};

/// The variable a user may set in the extra environment; it beats every choice.
pub const BACKEND_ENV: &str = "CASTANEA_BACKEND";
/// How long the probe may run before the game counts as not supporting it.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(8);
/// A start that fails within this many seconds counts as "did not come up".
pub const EARLY_SECONDS: u64 = 10;
/// A probe result is trusted this long (a day for an unknown GPU or a timeout).
const FRESH_SECONDS: u64 = 30 * 24 * 3600;
const SHORT_FRESH_SECONDS: u64 = 24 * 3600;

/// Why the launcher started the game the way it did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// The probe says Vulkan works.
    VulkanWorks,
    /// The probe says Vulkan does not work (or the probe crashed), wgpu does.
    VulkanFailed,
    /// An earlier start with Vulkan died within seconds on this game version and GPU.
    VulkanCrashed,
    /// The game has no probe (or it timed out): it picks by itself.
    NoProbe,
    /// The user picked the backend in Settings.
    Setting,
    /// `CASTANEA_BACKEND` is set in the extra environment.
    Environment,
}

/// What to start: the backend to pass (`Auto` leaves the choice to the game) and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub graphics: Graphics,
    pub reason: Reason,
    /// The adapter and driver the probe reported for the chosen backend; empty when unknown.
    pub adapter: String,
    pub driver: String,
    /// The game version and GPU key the choice was made for (the crash memory uses them).
    pub version: String,
    pub gpu: String,
}

/// What `record_outcome` learned from a finished start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Nothing,
    /// Vulkan, chosen by the launcher, died within seconds: wgpu is used from now on for this version and GPU.
    VulkanDied,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Decides the backend for this start. Runs the probe when nothing fresh is cached (up to `timeout`), so call it
/// from a worker thread. `gpu` is `gpu::system_key()`.
pub fn choose(paths: &Paths, build: &Build, options: &GameOptions, gpu: &str, timeout: Duration) -> Result<Choice> {
    choose_at(paths, build, options, gpu, timeout, now())
}

pub fn choose_at(
    paths: &Paths,
    build: &Build,
    options: &GameOptions,
    gpu: &str,
    timeout: Duration,
    now: u64,
) -> Result<Choice> {
    let made = |graphics, reason, adapter: &str, driver: &str| Choice {
        graphics,
        reason,
        adapter: adapter.to_owned(),
        driver: driver.to_owned(),
        version: build.version.clone(),
        gpu: gpu.to_owned(),
    };
    let mut state = BackendState::load(paths);
    let choice = if options.env.iter().any(|(k, v)| k.trim() == BACKEND_ENV && !v.trim().is_empty()) {
        made(Graphics::Auto, Reason::Environment, "", "")
    } else if options.graphics != Graphics::Auto {
        made(options.graphics, Reason::Setting, "", "")
    } else if state.vulkan_died(&build.version, gpu) {
        made(Graphics::Wgpu, Reason::VulkanCrashed, "", "")
    } else {
        let stored = match state.fresh(&build.version, gpu, now) {
            Some(stored) => stored.clone(),
            None => {
                if !build.is_present() {
                    return Err(Error::invalid(format!("{} is missing", build.binary.display())));
                }
                let game_dir = paths.game();
                prepare::prepare(&game_dir, build)?;
                let probed = probe::run(&build.binary, &game_dir, &options.env, timeout)?;
                let stored = Stored::from_probe(probed);
                state.remember(&build.version, gpu, now, stored.clone());
                stored
            }
        };
        decide(&stored, &made)?
    };
    state.last = Some(LastLaunch {
        version: build.version.clone(),
        backend: choice.graphics.env_value().unwrap_or("auto").to_owned(),
        reason: choice.reason,
        adapter: choice.adapter.clone(),
        driver: choice.driver.clone(),
        at: now,
    });
    let _ = state.save(paths);
    Ok(choice)
}

type Made<'a> = &'a dyn Fn(Graphics, Reason, &str, &str) -> Choice;

/// The rule: Vulkan if it works, else wgpu if it works (or was not tested), else an error that says why.
fn decide(stored: &Stored, made: Made) -> Result<Choice> {
    let report = match stored {
        Stored::Report(report) => report,
        Stored::Unsupported { .. } => return Ok(made(Graphics::Auto, Reason::NoProbe, "", "")),
        Stored::Crashed { .. } => return Ok(made(Graphics::Wgpu, Reason::VulkanFailed, "", "")),
    };
    if report.vulkan.ok {
        return Ok(made(Graphics::Vulkan, Reason::VulkanWorks, &report.vulkan.adapter, &report.vulkan.driver));
    }
    match &report.wgpu {
        Some(wgpu) if !wgpu.ok => Err(Error::invalid(format!(
            "no graphics backend works. Vulkan: {}. wgpu: {}",
            report.vulkan.error.as_deref().unwrap_or("failed"),
            wgpu.error.as_deref().unwrap_or("failed")
        ))),
        wgpu => {
            let (adapter, driver) = wgpu.as_ref().map_or(("", ""), |w| (w.adapter.as_str(), w.driver.as_str()));
            Ok(made(Graphics::Wgpu, Reason::VulkanFailed, adapter, driver))
        }
    }
}

/// Learns from a finished start: a Vulkan start the launcher chose that failed within `EARLY_SECONDS` is remembered,
/// so the next start for this game version and GPU uses wgpu. Call it only for a game that ended by itself.
pub fn record_outcome(paths: &Paths, choice: &Choice, outcome: &Outcome) -> Verdict {
    let counted = choice.graphics == Graphics::Vulkan && choice.reason == Reason::VulkanWorks;
    if !counted || !outcome.crashed || outcome.seconds >= EARLY_SECONDS {
        return Verdict::Nothing;
    }
    let mut state = BackendState::load(paths);
    state.mark_vulkan_died(&choice.version, &choice.gpu);
    let _ = state.save(paths);
    Verdict::VulkanDied
}

/// Forgets the cached probes and the remembered Vulkan crashes (a changed Graphics setting starts fresh).
pub fn forget(paths: &Paths) {
    let mut state = BackendState::load(paths);
    state.probes.clear();
    state.vulkan_died.clear();
    let _ = state.save(paths);
}

#[cfg(test)]
mod tests;
