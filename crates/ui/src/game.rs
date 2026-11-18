//! The game: which build starts, keeping it at the newest release (check, download, swap), and the running game with
//! its output.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use castanea_launcher_core::backend::{self, Choice, Verdict};
use castanea_launcher_core::install::update::{self, Check};
use castanea_launcher_core::install::{Build, Platform, Release, ReleaseApi, local, notes};
use castanea_launcher_core::launch::{self, Outcome, Running};
use eframe::egui;

use crate::app::App;
use crate::jobs::{self, Done};

/// Where the update of the game build stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Update {
    /// Not checked yet.
    Idle,
    Checking,
    UpToDate,
    Downloading {
        version: String,
    },
    /// Downloaded while the game ran; it is used when the game ends.
    Ready {
        version: String,
    },
    /// The downloaded build is being swapped in.
    Applying {
        version: String,
    },
    /// The release server has no build yet.
    NoRelease,
    /// There are no game builds for this operating system or processor.
    Unsupported(String),
    /// Offline mode is on.
    Offline,
    /// The check or the download failed; the installed build stays.
    Failed(String),
    /// A local build folder is used; no updates.
    Local,
}

impl Update {
    fn busy(&self) -> bool {
        matches!(self, Self::Checking | Self::Downloading { .. } | Self::Applying { .. })
    }
}

/// One start of the game: running, or ended with an outcome.
pub struct Session {
    pub command: String,
    pub running: Option<Running>,
    pub log: Vec<String>,
    pub log_total: u64,
    pub outcome: Option<Outcome>,
    pub started: Instant,
    /// The backend choice this start was made with.
    pub choice: Choice,
}

/// `4:05` or `1:02:03`.
pub fn clock(seconds: u64) -> String {
    let (h, m, s) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

impl App {
    /// The local build folder in use, if any: the one for this run, else the one in the settings.
    pub fn dev_folder(&self) -> Option<PathBuf> {
        self.dev_override.clone().or_else(|| self.settings.dev_build.clone())
    }

    /// Reads which build the game starts: the local build folder or the installed release.
    pub fn refresh_installed(&mut self) {
        self.installed = match self.dev_folder() {
            Some(folder) => match local::inspect(&folder) {
                Ok(build) => Some(build),
                Err(error) => {
                    self.error(error.to_string());
                    None
                }
            },
            None => update::installed(&self.paths),
        };
    }

    /// What the newest release changed, as plain text with its version: the one being installed, else the installed one.
    pub fn whats_new(&self) -> Option<(String, String)> {
        let pending =
            matches!(self.update, Update::Downloading { .. } | Update::Ready { .. } | Update::Applying { .. });
        let pending = self.pending_notes.clone().filter(|(_, text)| pending && !text.is_empty());
        let installed = self.installed.as_ref().filter(|b| !b.notes.is_empty() && self.dev_folder().is_none());
        pending.or_else(|| installed.map(|b| (b.version.clone(), b.notes.clone())))
    }

    /// Looks for a newer release and downloads it; offline mode and a local build folder skip it.
    pub fn check_updates(&mut self, ctx: &egui::Context) {
        self.checked = true;
        if self.update.busy() {
            return;
        }
        if self.dev_folder().is_some() {
            self.update = Update::Local;
            return;
        }
        if self.settings.offline {
            self.update = Update::Offline;
            return;
        }
        if let Err(error) = Platform::detect() {
            self.update = Update::Unsupported(error.to_string());
            return;
        }
        let api = match ReleaseApi::from_settings(&self.settings) {
            Ok(api) => api,
            Err(error) => {
                self.update = Update::Failed(error.to_string());
                return;
            }
        };
        self.update = Update::Checking;
        let installed = self.installed.clone();
        self.jobs.spawn(ctx, move || {
            let result = update::check(&api, &api.client(), installed.as_ref());
            Done::Checked(result.map_err(|e| e.to_string()))
        });
    }

    pub fn on_checked(&mut self, ctx: &egui::Context, result: Result<Check, String>) {
        if self.update != Update::Checking {
            return;
        }
        self.update = match result {
            Ok(Check::UpToDate) => Update::UpToDate,
            Ok(Check::NoRelease) => Update::NoRelease,
            Ok(Check::Update(remote)) => return self.download(ctx, *remote),
            Err(error) => Update::Failed(error),
        };
    }

    fn download(&mut self, ctx: &egui::Context, release: Release) {
        let api = match ReleaseApi::from_settings(&self.settings) {
            Ok(api) => api,
            Err(error) => {
                self.update = Update::Failed(error.to_string());
                return;
            }
        };
        self.update = Update::Downloading { version: release.version.clone() };
        self.pending_notes = Some((release.version.clone(), notes::plain(&release.notes)));
        let mut progress = jobs::reporter(ctx, &self.update_progress, release.version.clone());
        let paths = self.paths.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        self.jobs.spawn(ctx, move || {
            let result = update::download(&api, &api.client(), &release, &paths, &mut progress, &cancel);
            Done::Downloaded(result.map_err(|e| e.to_string()))
        });
    }

    pub fn on_downloaded(&mut self, ctx: &egui::Context, result: Result<Build, String>) {
        jobs::set(&self.update_progress, None);
        match result {
            Ok(build) if self.is_running() => self.update = Update::Ready { version: build.version },
            Ok(build) => self.apply(ctx, build.version),
            Err(error) => {
                let text = if error.contains(castanea_launcher_core::NOT_SIGNED) {
                    self.tr.get("notes.update_unsigned").to_owned()
                } else {
                    self.tr.fill("notes.update_failed", &[("error", &error)])
                };
                self.error(text);
                self.update = Update::Failed(error);
            }
        }
    }

    fn apply(&mut self, ctx: &egui::Context, version: String) {
        self.update = Update::Applying { version };
        let paths = self.paths.clone();
        self.jobs.spawn(ctx, move || Done::Applied(update::activate(&paths).map_err(|e| e.to_string())));
    }

    pub fn on_applied(&mut self, result: Result<Build, String>) {
        match result {
            Ok(build) => {
                let text = self.tr.fill("notes.updated", &[("version", &build.version)]);
                self.info(text);
                if self.dev_folder().is_none() {
                    self.installed = Some(build);
                }
                self.update = if self.dev_folder().is_some() { Update::Local } else { Update::UpToDate };
            }
            Err(error) => {
                let text = self.tr.fill("notes.update_failed", &[("error", &error)]);
                self.error(text);
                self.refresh_installed();
                self.update = Update::Failed(error);
            }
        }
    }

    pub fn is_running(&self) -> bool {
        self.session.as_ref().is_some_and(|s| s.running.is_some())
    }

    /// Play is possible: a build is there and is not being swapped.
    pub fn can_play(&self) -> bool {
        self.installed.as_ref().is_some_and(Build::is_present)
            && !matches!(self.update, Update::Applying { .. })
            && !self.starting
    }

    /// Starts the game: first decides the graphics backend on a worker (the probe may run), then `on_backend` starts it.
    pub fn play(&mut self, ctx: &egui::Context) {
        if self.is_running() || !self.can_play() {
            return;
        }
        let Some(build) = self.installed.clone() else { return };
        self.starting = true;
        let (paths, options) = (self.paths.clone(), self.settings.game.clone());
        self.jobs.spawn(ctx, move || {
            let gpu = backend::gpu::system_key();
            Done::Backend(
                backend::choose(&paths, &build, &options, &gpu, backend::PROBE_TIMEOUT).map_err(|e| e.to_string()),
            )
        });
    }

    /// `play` without the worker, for screenshots and tests that have no window to wait on.
    pub fn play_blocking(&mut self) {
        if self.is_running() || !self.can_play() {
            return;
        }
        let Some(build) = self.installed.clone() else { return };
        let gpu = backend::gpu::system_key();
        let result = backend::choose(&self.paths, &build, &self.settings.game, &gpu, backend::PROBE_TIMEOUT);
        self.on_backend(result.map_err(|e| e.to_string()));
    }

    pub fn on_backend(&mut self, result: Result<Choice, String>) {
        self.starting = false;
        self.backend_last = backend::BackendState::load(&self.paths).last;
        let choice = match result {
            Ok(choice) => choice,
            Err(error) => {
                let text = self.tr.fill("notes.no_graphics", &[("error", &error)]);
                return self.error(text);
            }
        };
        let Some(build) = self.installed.clone() else { return };
        let mut options = self.settings.game.clone();
        options.graphics = choice.graphics;
        match launch::launch(&self.paths, &build, &options, self.settings.log_lines) {
            Ok((plan, running)) => {
                self.session = Some(Session {
                    command: plan.display(),
                    running: Some(running),
                    log: Vec::new(),
                    log_total: 0,
                    outcome: None,
                    started: Instant::now(),
                    choice,
                });
            }
            Err(error) => self.error(error.to_string()),
        }
    }

    pub fn stop(&mut self) {
        let Some(session) = &mut self.session else { return };
        let Some(mut running) = session.running.take() else { return };
        match running.stop() {
            Ok(outcome) => {
                if let Ok(log) = running.log.lock() {
                    session.log = log.lines.iter().cloned().collect();
                }
                session.outcome = Some(outcome);
            }
            Err(error) => self.error(error.to_string()),
        }
    }

    /// The hint for a game that could not load a library (exit 127 or a loader error), in the language of the UI.
    pub fn missing_library_hint(&self, outcome: &Outcome) -> Option<String> {
        let text = match (&outcome.missing_library, outcome.code) {
            (Some(lib), _) => {
                let package = launch::library_package(lib).unwrap_or(lib);
                self.tr.fill("hint.missing_lib", &[("lib", lib), ("package", package)])
            }
            (None, Some(127)) => self.tr.get("hint.exit_unknown_lib").to_owned(),
            _ => return None,
        };
        let nix = if std::path::Path::new("/etc/NIXOS").exists() { self.tr.get("hint.missing_lib_nix") } else { "" };
        Some(format!("{text}{nix}"))
    }

    /// Seconds since the running game started.
    pub fn running_seconds(&self) -> Option<u64> {
        self.session.as_ref().filter(|s| s.running.is_some()).map(|s| s.started.elapsed().as_secs())
    }

    /// Copies new output and notices the exit; called every frame.
    pub fn poll_session(&mut self, ctx: &egui::Context) {
        let Some(session) = &mut self.session else { return };
        let Some(running) = &mut session.running else { return };
        if let Ok(log) = running.log.lock()
            && log.total != session.log_total
        {
            session.log = log.lines.iter().cloned().collect();
            session.log_total = log.total;
        }
        let ended = match running.poll() {
            Ok(None) => return,
            Ok(Some(outcome)) => {
                if let Ok(log) = running.log.lock() {
                    session.log = log.lines.iter().cloned().collect();
                }
                session.outcome = Some(outcome.clone());
                session.running = None;
                Ok(outcome)
            }
            Err(error) => {
                session.running = None;
                Err(error.to_string())
            }
        };
        match ended {
            Ok(outcome) if outcome.crashed => {
                let code = outcome.code.map_or("-".to_owned(), |c| c.to_string());
                let text = match self.missing_library_hint(&outcome) {
                    Some(hint) => hint,
                    None => self.tr.fill("notes.crashed", &[("code", &code)]),
                };
                self.error(text);
                let choice = self.session.as_ref().map(|s| s.choice.clone());
                if let Some(choice) = choice
                    && backend::record_outcome(&self.paths, &choice, &outcome) == Verdict::VulkanDied
                {
                    let text = self.tr.get("notes.vulkan_died").to_owned();
                    self.info(text);
                }
            }
            Ok(_) => {}
            Err(error) => self.error(error),
        }
        if let Update::Ready { version } = &self.update {
            let version = version.clone();
            self.apply(ctx, version);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn clock_shows_minutes_and_hours() {
        assert_eq!(super::clock(5), "0:05");
        assert_eq!(super::clock(245), "4:05");
        assert_eq!(super::clock(3723), "1:02:03");
    }
}
