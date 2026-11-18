//! The news feed on the home page: what is shown, where it stands, and the background request. It never blocks
//! Play; whatever is saved on disk shows at once, and a failed request only changes a small mark.

use std::collections::HashSet;

use castanea_launcher_core::install::release_api::{URL_ENV, base_url};
use castanea_launcher_core::news::{self, Item};
use eframe::egui;

use crate::app::App;
use crate::jobs::Done;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    /// Nothing asked yet.
    #[default]
    Idle,
    Loading,
    /// The server answered.
    Fresh,
    /// Offline mode: only what is saved.
    Offline,
    /// The request failed: only what is saved.
    Failed,
}

#[derive(Default)]
pub struct NewsState {
    pub items: Vec<Item>,
    pub phase: Phase,
    /// The request for the current language went out (or was skipped for offline mode).
    pub requested: bool,
    /// Items shown in full.
    pub open: HashSet<String>,
}

impl NewsState {
    /// The saved items are shown with a quiet "offline" mark instead of being current.
    pub fn stale(&self) -> bool {
        matches!(self.phase, Phase::Offline | Phase::Failed)
    }
}

/// Three made-up items in `lang`, for screenshots and tests.
pub fn demo(lang: &str) -> Vec<Item> {
    let item = |id: &str, title: &str, body: &str, date: &str, tag: &str, url: Option<&str>| Item {
        id: id.into(),
        title: title.into(),
        body: body.into(),
        date: date.into(),
        tag: tag.into(),
        url: url.map(Into::into),
    };
    if lang == "de" {
        vec![
            item(
                "steam-age",
                "Das Dampfzeitalter beginnt",
                "Schmelzöfen, Gießtische und die ersten Dampfmaschinen sind da. **Gieß dein erstes Kupferteil** und lies in [der Gießanleitung](https://castanea.xenoo.dev/docs/casting), wie es funktioniert.\n\nDie Kupferzeit endet, sobald der erste Stahlbarren den Ofen verlässt. Bis dahin: viel Spaß am warmen Glühen des neuen Schmelzofens.",
                "2026-11-12",
                "Release",
                Some("https://castanea.xenoo.dev/news/steam-age"),
            ),
            item(
                "mod-index",
                "Mod-Browser mit Versionsfilter",
                "Der Browser zeigt jetzt nur Mods, die zu deiner Spielversion passen. Für Modder ändert sich am [Index-Format](https://castanea.xenoo.dev/docs/mod-index) nichts.",
                "2026-11-03",
                "Launcher",
                None,
            ),
            item(
                "playtest",
                "Playtest am Wochenende",
                "Am 21. und 22. November ist die Testwelt offen. Fehlerberichte sind willkommen.",
                "2026-10-28",
                "Community",
                None,
            ),
        ]
    } else {
        vec![
            item(
                "steam-age",
                "The steam age begins",
                "Smelteries, casting tables and the first steam machines are in. **Cast your first copper part**, then read how it works in [the casting guide](https://castanea.xenoo.dev/docs/casting).\n\nThe copper age ends when the first steel ingot leaves the furnace. Until then, enjoy the warm glow of the new smeltery.",
                "2026-11-12",
                "Release",
                Some("https://castanea.xenoo.dev/news/steam-age"),
            ),
            item(
                "mod-index",
                "A mod browser that knows your version",
                "The browser now shows only mods that fit your game version. Modders: the [index format](https://castanea.xenoo.dev/docs/mod-index) did not change.",
                "2026-11-03",
                "Launcher",
                None,
            ),
            item(
                "playtest",
                "Playtest this weekend",
                "The test world is open on 21 and 22 November. Bug reports are welcome.",
                "2026-10-28",
                "Community",
                None,
            ),
        ]
    }
}

impl App {
    /// The saved feed of the current language, read from disk (small, limited): shown from the first frame.
    pub fn load_news_cache(&mut self) {
        self.news =
            NewsState { items: news::load(&self.paths.news_cache(), &self.tr.code).items, ..NewsState::default() };
    }

    /// Shows the made-up items in the current language, as if the server just sent them.
    pub fn show_demo_news(&mut self, phase: Phase) {
        self.news.items = demo(&self.tr.code);
        self.news.phase = phase;
    }

    /// Asks the server for the news once per language and per offline switch; called every frame, cheap.
    pub fn auto_news(&mut self, ctx: &egui::Context) {
        if !self.auto_check {
            return;
        }
        if self.settings.offline {
            self.news.phase = Phase::Offline;
            self.news.requested = false;
            return;
        }
        if self.news.requested {
            return;
        }
        self.news.requested = true;
        self.news.phase = Phase::Loading;
        let dir = self.paths.news_cache();
        let base = base_url(&self.settings, std::env::var(URL_ENV).ok().as_deref());
        let lang = self.tr.code.clone();
        self.jobs.spawn(ctx, move || {
            let result = news::sync(&dir, &base, &lang).map_err(|e| e.to_string());
            Done::News { lang, result }
        });
    }

    pub fn on_news(&mut self, lang: String, result: Result<Vec<Item>, String>) {
        if lang != self.tr.code {
            return;
        }
        match result {
            Ok(items) => {
                self.news.items = items;
                self.news.phase = Phase::Fresh;
            }
            Err(_) => self.news.phase = Phase::Failed,
        }
    }
}
