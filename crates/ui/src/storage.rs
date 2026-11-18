//! Storage and reset (docs/ui-design.md, "Storage and reset"): the sizes measured in the background and the deletions,
//! on their own thread. The page that draws them is `pages/storage.rs`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use castanea_launcher_core::settings::Settings;
use castanea_launcher_core::storage::{self, Failure, Report, Row, Scope, Size};
use eframe::egui;

use crate::app::{App, Confirm};
use crate::game::Update;
use crate::jobs::Done;

/// What one row is, as far as measured.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RowInfo {
    pub size: Size,
    /// Something of the row exists on disk.
    pub present: bool,
}

/// A deletion the user confirmed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Deletion {
    Row(Row),
    /// All the launcher's own rows ([`Row::OWN`]).
    Everything,
    /// The same, then the launcher program itself.
    #[cfg(windows)]
    Uninstall,
}

impl Deletion {
    /// The rows this deletion goes through.
    fn rows(&self) -> Vec<Row> {
        match self {
            Self::Row(row) => vec![*row],
            Self::Everything => Row::OWN.to_vec(),
            #[cfg(windows)]
            Self::Uninstall => Row::OWN.to_vec(),
        }
    }
}

#[derive(Default)]
pub struct StorageState {
    /// Measured rows; a missing row is still being measured.
    pub info: HashMap<Row, RowInfo>,
    pub measuring: bool,
    /// What could not be deleted, per row, until the next try.
    pub failures: HashMap<Row, Vec<Failure>>,
    /// The text in the DELETE field of the game data question.
    pub typed: String,
    pub deleting: bool,
}

/// The row whose paths hold `path`, among the rows as they were before the deletion.
fn owner(before: &[(Row, Vec<PathBuf>)], path: &Path) -> Option<Row> {
    before.iter().find(|(_, paths)| paths.iter().any(|p| path.starts_with(p))).map(|(row, _)| *row)
}

/// The folder a row's size and Delete button stand for, shown under its name.
pub fn shown_path(paths: &castanea_launcher_core::paths::Paths, row: Row) -> PathBuf {
    match row {
        Row::Cache => paths.cache.clone(),
        Row::Game => paths.build(),
        Row::Mods => paths.mods(),
        Row::Logs => paths.game().join(castanea_launcher_core::launch::run::LOG_DIR),
        Row::Settings => paths.config.clone(),
        Row::GameData => paths.game(),
    }
}

/// `0 B`, `12 KB`, `1.5 MB`, `2.3 GB`.
pub fn bytes_text(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let value = bytes as f64;
    if bytes < 1024 {
        format!("{bytes} B")
    } else if value < KB * KB {
        format!("{} KB", bytes.div_ceil(1024))
    } else if value < KB * KB * KB {
        format!("{:.1} MB", value / (KB * KB))
    } else {
        format!("{:.1} GB", value / (KB * KB * KB))
    }
}

impl App {
    /// Folders the user owns that deletion must never enter: the local game folders.
    fn protected_folders(&self) -> Vec<PathBuf> {
        self.dev_folder().into_iter().collect()
    }

    pub fn storage_scope(&self) -> Scope {
        Scope::new(&self.paths, &self.protected_folders())
    }

    /// Why nothing can be deleted now, as a language key; `None` when it can.
    pub fn storage_blocked(&self) -> Option<&'static str> {
        if self.is_running() {
            Some("storage.running")
        } else if self.storage.deleting {
            Some("storage.busy")
        } else if matches!(self.update, Update::Downloading { .. } | Update::Applying { .. })
            || self.news.phase == crate::news::Phase::Loading
        {
            Some("storage.updating")
        } else {
            None
        }
    }

    /// Measures every row on its own thread; the page shows `…` until each answer is in.
    pub fn measure_storage(&mut self, ctx: &egui::Context) {
        self.storage.measuring = true;
        for row in Row::ALL {
            let paths = self.paths.clone();
            self.jobs.spawn(ctx, move || {
                let present = !storage::targets(&paths, row).is_empty();
                let size = storage::measure(&paths, row, &AtomicBool::new(false)).unwrap_or_default();
                Done::StorageSize { row, info: RowInfo { size, present } }
            });
        }
    }

    /// Starts a deletion on its own thread; the result comes back as `Done::StorageDeleted`.
    pub fn start_delete(&mut self, ctx: &egui::Context, what: Deletion) {
        if let Some(why) = self.storage_blocked() {
            let text = self.tr.get(why).to_owned();
            self.error(text);
            return;
        }
        self.storage.deleting = true;
        let (scope, paths, running) = (self.storage_scope(), self.paths.clone(), self.is_running());
        self.jobs.spawn(ctx, move || {
            let rows = what.rows();
            let before: Vec<(Row, Vec<PathBuf>)> = rows.iter().map(|r| (*r, storage::targets(&paths, *r))).collect();
            let report = match &what {
                Deletion::Row(Row::GameData) => storage::delete_confirmed(&scope, &paths, running, "DELETE"),
                Deletion::Row(row) => storage::delete_row(&scope, &paths, *row, running),
                _ => storage::delete_all(&scope, &paths, running),
            };
            let result = report.map_err(|e| e.to_string()).map(|report| {
                let failures =
                    report.failures.iter().map(|f| (owner(&before, &f.path).unwrap_or(rows[0]), f.clone())).collect();
                (report, failures)
            });
            Done::StorageDeleted { what, result }
        });
    }

    pub fn on_storage_info(&mut self, row: Row, info: RowInfo) {
        self.storage.info.insert(row, info);
    }

    pub fn on_storage_deleted(
        &mut self,
        ctx: &egui::Context,
        what: Deletion,
        result: Result<(Report, Vec<(Row, Failure)>), String>,
    ) {
        self.storage.deleting = false;
        let failures = match result {
            Ok((_, failures)) => failures,
            Err(error) => return self.error(error),
        };
        let affected = what.rows();
        for row in &affected {
            self.storage.failures.remove(row);
        }
        let count = failures.len();
        for (row, failure) in failures {
            self.storage.failures.entry(row).or_default().push(failure);
        }
        let gone = |row: Row| affected.contains(&row) && !self.storage.failures.contains_key(&row);
        let (game, mods, settings) = (gone(Row::Game), gone(Row::Mods), gone(Row::Settings));
        if game {
            self.refresh_installed();
            self.update = Update::Idle;
            // The user just deleted the game: no new download until they ask for it.
            self.checked = true;
        }
        if mods {
            self.reload_mods();
        }
        if settings {
            self.settings = Settings::default();
            self.release_url_input = self.settings.release_url.clone();
            self.reload_language();
        }
        self.storage.info.clear();
        self.measure_storage(ctx);
        if count > 0 {
            let text = self.tr.fill("storage.partial", &[("count", &count.to_string())]);
            return self.error(text);
        }
        match what {
            Deletion::Row(row) => {
                let name = self.tr.get(row_name_key(row)).to_owned();
                let text = self.tr.fill("storage.deleted", &[("name", &name)]);
                self.info(text);
            }
            Deletion::Everything => {
                // Nothing may fetch or save anything again before the window closes.
                self.auto_check = false;
                self.confirm = Some(Confirm::Goodbye);
            }
            #[cfg(windows)]
            Deletion::Uninstall => self.finish_uninstall(),
        }
    }

    /// The data is gone: hand over to the helper that removes the program, then exit without saving anything.
    #[cfg(windows)]
    fn finish_uninstall(&mut self) {
        let Some(layout) = self.launcher_layout.clone() else { return };
        match storage::start_finisher(&layout.exe) {
            Ok(()) => std::process::exit(0),
            Err(error) => self.error(error.to_string()),
        }
    }
}

/// The language key of a row's name; literal keys, so the key check finds them.
pub fn row_name_key(row: Row) -> &'static str {
    match row {
        Row::Cache => "storage.cache",
        Row::Game => "storage.game",
        Row::Mods => "storage.mods",
        Row::Logs => "storage.logs",
        Row::Settings => "storage.settings",
        Row::GameData => "storage.game_data",
    }
}

/// The language key of what a row holds.
pub fn row_hint_key(row: Row) -> &'static str {
    match row {
        Row::Cache => "storage.cache_hint",
        Row::Game => "storage.game_hint",
        Row::Mods => "storage.mods_hint",
        Row::Logs => "storage.logs_hint",
        Row::Settings => "storage.settings_hint",
        Row::GameData => "storage.game_data_hint",
    }
}
