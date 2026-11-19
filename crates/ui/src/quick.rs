//! Quick start (docs/quick-start.md): what was played last, found on a worker from the launcher's own record of its
//! starts and the saves folder, and the picture the Continue card shows. The card itself is `pages/home/card.rs`.

use castanea_launcher_core::gallery::{self, Shot};
use castanea_launcher_core::history::{self, History, Played};
use castanea_launcher_core::launch::{self, Start};
use eframe::egui;

use crate::app::App;
use crate::jobs::Done;

/// Slack around a play session when looking for its screenshots, milliseconds.
const SLACK_MS: i64 = 5_000;

#[derive(Default)]
pub struct QuickState {
    /// The last world or server played; none when nothing was played yet.
    pub played: Option<Played>,
    pub loaded: bool,
    pub loading: bool,
    /// A new look is wanted at the next frame (the game ended).
    pub stale: bool,
    /// Bumped whenever `played` is replaced.
    pub generation: u64,
    /// The card's picture and what it was found for (`generation`, the gallery's `version`).
    pub picture: Option<Shot>,
    picture_for: Option<(u64, u64)>,
}

/// The newest screenshot of the play session, else the world's own picture.
pub fn pick_picture(played: &Played, shots: &[Shot]) -> Option<Shot> {
    let in_session =
        played.window.and_then(|(from, to)| gallery::newest_between(shots, from - SLACK_MS, to + SLACK_MS));
    match in_session {
        Some(shot) => Some(shot.clone()),
        None => played.preview.as_deref().and_then(Shot::from_file),
    }
}

impl App {
    /// Looks for the last played at the first frame and after a game ended.
    pub fn auto_last_played(&mut self, ctx: &egui::Context) {
        if (self.quick.loaded && !self.quick.stale) || self.quick.loading {
            return;
        }
        self.quick.stale = false;
        self.quick.loading = true;
        let paths = self.paths.clone();
        self.jobs.spawn(ctx, move || {
            let record = History::load(&paths);
            Done::LastPlayed(history::last_played(&record, &paths.game(), history::now_ms()))
        });
    }

    pub fn on_last_played(&mut self, played: Option<Played>) {
        self.quick.played = played;
        self.quick.loaded = true;
        self.quick.loading = false;
        self.quick.generation += 1;
    }

    /// Finds the card's picture again when the last played or the screenshot list changed.
    pub fn refresh_card_picture(&mut self) {
        let key = (self.quick.generation, self.gallery.version);
        if self.quick.picture_for == Some(key) {
            return;
        }
        self.quick.picture_for = Some(key);
        self.quick.picture = self.quick.played.as_ref().and_then(|p| pick_picture(p, &self.gallery.shots));
    }

    /// The Continue card is shown: something was played, the game does not run, and a start is possible.
    pub fn card_visible(&self) -> bool {
        self.quick.played.is_some() && !self.is_running()
    }

    /// Starts the game where it was left: the same world or server, straight in.
    pub fn continue_play(&mut self, ctx: &egui::Context) {
        if let Some(start) = self.quick.played.as_ref().map(|p| p.start.clone()) {
            self.play_with(ctx, start);
        }
    }

    /// Where the last start went, for the card's second line.
    pub fn card_kind_key(start: &Start) -> &'static str {
        match start {
            Start::Join(_) => "home.continue_server",
            _ => "home.continue_world",
        }
    }

    /// The card's picture is decoded or failed (screenshots and the idle tests wait for it).
    pub fn card_settled(&mut self) -> bool {
        if !self.quick.loaded || self.quick.loading {
            return false;
        }
        self.refresh_card_picture();
        match &self.quick.picture {
            Some(shot) if self.card_visible() => {
                self.gallery.failed.contains(&shot.path) || self.gallery.thumbs.get(&shot.path).is_some()
            }
            _ => true,
        }
    }
}

impl App {
    /// Puts a start into the record and returns its key. A start at the start screen with `--world` or `--join` in the
    /// extra arguments went there all the same, so that is what is recorded. A record that cannot be written is no error.
    pub fn record_game_start(&mut self, start: &Start, version: &str) -> Option<i64> {
        let target = match start {
            Start::Menu => launch::plan::split_args(&self.settings.game.extra_args)
                .map(|a| Start::from_args(&a))
                .unwrap_or_default(),
            other => other.clone(),
        };
        history::record_start(&self.paths, target, version).ok()
    }

    /// The game ended: closes its record, and the screenshots and the last played are looked at again.
    pub fn record_game_end(&mut self) {
        if let Some(started) = self.session.as_mut().and_then(|s| s.started_ms.take()) {
            let _ = history::record_end(&self.paths, started);
        }
        self.quick.stale = true;
        self.gallery.stale = true;
    }
}
