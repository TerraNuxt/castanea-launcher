//! The launcher keeping itself at its newest release: look once at start, stage and put the new build next to the
//! running one, offer a restart. A managed install (package, AppImage, read-only folder) only shows the new version.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use castanea_launcher_core::NOT_SIGNED;
use castanea_launcher_core::install::{Release, ReleaseApi, notes};
use castanea_launcher_core::selfupdate::{self, Check, Files, Install, Layout, Os, Why};
use eframe::egui;

use crate::app::App;
use crate::game::Update;
use crate::jobs::{self, Done};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Where releases of the launcher can be downloaded by hand.
pub fn download_page() -> String {
    format!("{}/releases", env!("CARGO_PKG_REPOSITORY"))
}

/// Where the launcher's own update stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LauncherUpdate {
    /// Not checked yet.
    Idle,
    /// The setting is off.
    Off,
    Offline,
    Checking,
    UpToDate,
    Downloading {
        version: String,
    },
    /// The new launcher is in place; it runs after a restart.
    Restart {
        version: String,
    },
    /// A newer launcher exists but something else owns the installation.
    Managed {
        version: String,
        why: Why,
    },
    /// The new launcher's download was not signed by castanea and was thrown away.
    Refused {
        version: String,
    },
    Failed(String),
}

impl LauncherUpdate {
    fn busy(&self) -> bool {
        matches!(self, Self::Checking | Self::Downloading { .. })
    }
}

impl App {
    /// The game's own update is not in the middle of a download or swap.
    fn game_settled(&self) -> bool {
        !matches!(
            self.update,
            Update::Idle
                | Update::Checking
                | Update::Downloading { .. }
                | Update::Applying { .. }
                | Update::Ready { .. }
        )
    }

    /// Once per start, when the game's check is done: look for a newer launcher.
    pub fn auto_check_launcher(&mut self, ctx: &egui::Context) {
        if self.auto_check && !self.launcher_checked && self.checked && self.game_settled() {
            self.check_launcher(ctx);
        }
    }

    pub fn check_launcher(&mut self, ctx: &egui::Context) {
        self.launcher_checked = true;
        if self.launcher_layout.is_none() {
            return;
        }
        if self.launcher.busy() || matches!(self.launcher, LauncherUpdate::Restart { .. }) {
            return;
        }
        if !self.settings.launcher_update {
            self.launcher = LauncherUpdate::Off;
            return;
        }
        if self.settings.offline {
            self.launcher = LauncherUpdate::Offline;
            return;
        }
        let api = match ReleaseApi::from_settings(&self.settings) {
            Ok(api) => api.for_launcher(),
            Err(error) => {
                self.launcher = LauncherUpdate::Failed(error.to_string());
                return;
            }
        };
        self.launcher = LauncherUpdate::Checking;
        let files = Files::new(&self.paths);
        self.jobs.spawn(ctx, move || {
            Done::LauncherChecked(selfupdate::check(&api, &api.client(), VERSION, &files).map_err(|e| e.to_string()))
        });
    }

    pub fn on_launcher_checked(&mut self, ctx: &egui::Context, result: Result<Check, String>) {
        if self.launcher != LauncherUpdate::Checking {
            return;
        }
        let check = match result {
            Ok(check) => check,
            Err(error) => {
                self.launcher = LauncherUpdate::Failed(error);
                return;
            }
        };
        match check {
            Check::UpToDate | Check::NoRelease => self.launcher = LauncherUpdate::UpToDate,
            Check::Update(release) => {
                self.launcher_notes = Some((release.version.clone(), notes::plain(&release.notes)));
                let Some(layout) = self.launcher_layout.clone() else { return };
                if let Install::Managed(why) = selfupdate::detect(&layout.exe) {
                    self.launcher = LauncherUpdate::Managed { version: release.version.clone(), why };
                    return;
                }
                self.install_launcher(ctx, *release, layout);
            }
        }
    }

    fn install_launcher(&mut self, ctx: &egui::Context, release: Release, layout: Layout) {
        let api = match ReleaseApi::from_settings(&self.settings) {
            Ok(api) => api.for_launcher(),
            Err(error) => {
                self.launcher = LauncherUpdate::Failed(error.to_string());
                return;
            }
        };
        self.launcher = LauncherUpdate::Downloading { version: release.version.clone() };
        let mut progress = jobs::reporter(ctx, &self.launcher_progress, release.version.clone());
        let paths = self.paths.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        self.jobs.spawn(ctx, move || {
            let mut work = || -> castanea_launcher_core::Result<String> {
                let staged = selfupdate::stage(&api, &api.client(), &release, &paths, &mut progress, &cancel)?;
                let files = Files::new(&paths);
                selfupdate::apply(Os::current(), &layout, &files, &staged, VERSION)?;
                Ok(staged.version)
            };
            Done::LauncherApplied(work().map_err(|e| e.to_string()))
        });
    }

    pub fn on_launcher_applied(&mut self, result: Result<String, String>) {
        jobs::set(&self.launcher_progress, None);
        match result {
            Ok(version) => {
                let text = self.tr.fill("launcher.ready_note", &[("version", &version)]);
                self.info(text);
                self.launcher = LauncherUpdate::Restart { version };
            }
            Err(error) if error.contains(NOT_SIGNED) => {
                let version = self.launcher_notes.as_ref().map(|(v, _)| v.clone()).unwrap_or_default();
                let text = self.tr.fill("launcher.refused_note", &[("version", &version)]);
                self.error(text);
                self.launcher = LauncherUpdate::Refused { version };
            }
            Err(error) => self.launcher = LauncherUpdate::Failed(error),
        }
    }

    /// Starts the updated launcher and closes this one; the game must not be running.
    pub fn restart_launcher(&mut self, ctx: &egui::Context) {
        if self.is_running() {
            let text = self.tr.get("launcher.restart_game_running").to_owned();
            self.error(text);
            return;
        }
        let Some(layout) = self.launcher_layout.clone() else { return };
        match selfupdate::relaunch(&layout, std::env::args_os().skip(1)) {
            Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Err(error) => self.error(format!("{}: {error}", layout.exe.display())),
        }
    }

    /// Look again at the next opportunity (after a setting changed); a launcher that waits for its restart stays so.
    pub fn reset_launcher_check(&mut self) {
        self.launcher_checked = false;
        if !matches!(self.launcher, LauncherUpdate::Restart { .. }) {
            self.launcher = LauncherUpdate::Idle;
        }
    }

    /// Counts drawn frames; after the third the new launcher counts as started and the old one is deleted.
    pub fn frame_drawn(&mut self) {
        self.frames_drawn = self.frames_drawn.saturating_add(1);
        if self.frames_drawn == 3
            && let Some(layout) = &self.launcher_layout
        {
            selfupdate::first_frame(&self.paths, layout);
        }
    }

    /// The quiet line: `castanea launcher 0.2.0 · up to date`.
    pub fn launcher_line(&self) -> String {
        let tr = &self.tr;
        let current = [("version", VERSION)];
        let with = |key: &str, new: &str, percent: &str| {
            tr.fill(key, &[("version", VERSION), ("new", new), ("percent", percent)])
        };
        match &self.launcher {
            LauncherUpdate::Idle | LauncherUpdate::UpToDate => tr.fill("launcher.line_up_to_date", &current),
            LauncherUpdate::Off => tr.fill("launcher.line_off", &current),
            LauncherUpdate::Offline => tr.fill("launcher.line_offline", &current),
            LauncherUpdate::Checking => tr.fill("launcher.line_checking", &current),
            LauncherUpdate::Downloading { version } => {
                let fraction = jobs::get(&self.launcher_progress).and_then(|p| p.fraction()).unwrap_or(0.0);
                with("launcher.line_downloading", version, &format!("{:.0}", fraction * 100.0))
            }
            LauncherUpdate::Restart { version } => with("launcher.line_restart", version, ""),
            LauncherUpdate::Managed { version, .. } => with("launcher.line_managed", version, ""),
            LauncherUpdate::Refused { version } => with("launcher.line_refused", version, ""),
            LauncherUpdate::Failed(_) => tr.fill("launcher.line_failed", &current),
        }
    }

    /// Why this installation does not update itself, in words.
    pub fn why_managed(&self, why: Why) -> String {
        let key = match why {
            Why::AppImage => "launcher.why_appimage",
            Why::System => "launcher.why_system",
            Why::ReadOnly => "launcher.why_readonly",
        };
        self.tr.get(key).to_owned()
    }
}
