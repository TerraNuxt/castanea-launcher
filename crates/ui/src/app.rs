//! The launcher's state and its actions for mods and the index; the game and its updates are in `game.rs`, the pages
//! draw it (`pages/`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Instant;

use castanea_launcher_core::download::{Client, UrlPolicy};
use castanea_launcher_core::index::client::{self as index_client, ModIndex, StaticIndex};
use castanea_launcher_core::index::{Index, install as index_install};
use castanea_launcher_core::install::Build;
use castanea_launcher_core::migrate;
use castanea_launcher_core::mods::{InstalledMod, installed};
use castanea_launcher_core::paths::Paths;
use castanea_launcher_core::settings::Settings;
use eframe::egui;

use crate::game::{Session, Update};
use crate::i18n::Strings;
use crate::icons;
use crate::jobs::{self, Done, Jobs, Slot};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Home,
    Mods,
    Log,
    Settings,
    About,
}

impl Page {
    /// Sidebar order, which is also the order of Ctrl+1 … Ctrl+5.
    pub const ALL: [Self; 5] = [Self::Home, Self::Mods, Self::Log, Self::Settings, Self::About];
    /// The pages at the top of the sidebar; About sits at the bottom.
    pub const TOP: usize = 4;

    pub fn key(self) -> &'static str {
        match self {
            Self::Home => "nav.play",
            Self::Mods => "nav.mods",
            Self::Log => "nav.output",
            Self::Settings => "nav.settings",
            Self::About => "nav.about",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Home => icons::PLAY,
            Self::Mods => icons::PUZZLE,
            Self::Log => icons::TERMINAL,
            Self::Settings => icons::GEAR,
            Self::About => icons::INFO,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModsView {
    Installed,
    Browse,
}

/// A question the user must answer before something happens.
pub enum Confirm {
    EnableMod { file_name: String },
    RemoveMod { file_name: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteKind {
    Info,
    Error,
}

/// A message shown for a few seconds at the bottom right.
pub struct Note {
    pub kind: NoteKind,
    pub text: String,
    pub created: Instant,
}

#[derive(Default)]
pub struct BrowseState {
    pub query: String,
    pub tag: Option<String>,
    pub compatible_only: bool,
    pub selected: Option<String>,
}

pub enum ImageState {
    Loading,
    Ready(egui::load::Bytes),
    Failed,
}

pub struct App {
    pub paths: Paths,
    pub settings: Settings,
    pub tr: Strings,
    pub page: Page,
    pub mods_view: ModsView,
    /// The build the game starts from: the installed release, or the local build folder.
    pub installed: Option<Build>,
    pub update: Update,
    pub update_progress: Slot,
    /// A local build folder for this run only (`--game-dir`, `CASTANEA_GAME_DIR`).
    pub dev_override: Option<PathBuf>,
    /// Look for a newer build at the first frame; off for screenshots.
    pub auto_check: bool,
    pub checked: bool,
    pub session: Option<Session>,
    pub notes: Vec<Note>,
    pub index: Index,
    pub index_warnings: Vec<String>,
    pub index_loaded: bool,
    pub mods: Vec<InstalledMod>,
    pub browse: BrowseState,
    pub confirm: Option<Confirm>,
    pub jobs: Jobs,
    pub images: HashMap<String, ImageState>,
    pub token_input: String,
    pub new_index: String,
    pub advanced_open: bool,
    /// Scrolls the open Advanced section into view once.
    pub reveal_advanced: bool,
    pub focus_search: bool,
}

impl App {
    pub fn new(paths: Paths, dev_override: Option<PathBuf>) -> Self {
        let mut errors = Vec::new();
        let mut settings = Settings::load(&paths).unwrap_or_else(|e| {
            errors.push(e.to_string());
            Settings::default()
        });
        let migrated = migrate::run(&paths, &mut settings).unwrap_or_else(|e| {
            errors.push(e.to_string());
            None
        });
        let mut app = Self {
            tr: Strings::load(&settings.language),
            paths,
            settings,
            page: Page::Home,
            mods_view: ModsView::Installed,
            installed: None,
            update: Update::Idle,
            update_progress: Slot::default(),
            dev_override,
            auto_check: true,
            checked: false,
            session: None,
            notes: Vec::new(),
            index: Index::default(),
            index_warnings: Vec::new(),
            index_loaded: false,
            mods: Vec::new(),
            browse: BrowseState::default(),
            confirm: None,
            jobs: Jobs::default(),
            images: HashMap::new(),
            token_input: String::new(),
            new_index: String::new(),
            advanced_open: false,
            reveal_advanced: false,
            focus_search: false,
        };
        for error in errors {
            app.error(error);
        }
        if let Some(report) = migrated {
            let profile = report.profile.unwrap_or_default();
            let mut text = app.tr.fill("notes.migrated", &[("profile", &profile)]);
            if !report.left.is_empty() {
                let folders: Vec<String> = report.left.iter().map(|p| p.display().to_string()).collect();
                text = format!("{text} {}", app.tr.fill("notes.migrated_left", &[("folders", &folders.join(", "))]));
            }
            app.info(text);
        }
        app.refresh_installed();
        app.reload_mods();
        app
    }

    pub fn client() -> Client {
        Client::new(UrlPolicy::default())
    }

    pub fn go(&mut self, page: Page) {
        self.page = page;
    }

    /// The version of the build the game starts, for mod compatibility checks.
    pub fn game_version(&self) -> Option<String> {
        self.installed.as_ref().map(|b| b.version.clone())
    }

    pub fn error(&mut self, text: impl Into<String>) {
        self.notes.push(Note { kind: NoteKind::Error, text: text.into(), created: Instant::now() });
    }

    pub fn info(&mut self, text: impl Into<String>) {
        self.notes.push(Note { kind: NoteKind::Info, text: text.into(), created: Instant::now() });
    }

    pub fn save_settings(&mut self) {
        if let Err(error) = self.settings.save(&self.paths) {
            self.error(error.to_string());
        }
    }

    pub fn mods_dir(&self) -> PathBuf {
        self.paths.mods()
    }

    pub fn reload_mods(&mut self) {
        match installed::list(&self.mods_dir()) {
            Ok(list) => self.mods = list,
            Err(error) => {
                self.mods.clear();
                self.error(error.to_string());
            }
        }
    }

    pub fn load_index(&mut self, ctx: &egui::Context) {
        let sources: Vec<Box<dyn ModIndex>> = self
            .settings
            .indexes
            .iter()
            .map(|location| Box::new(StaticIndex::new(location, &self.paths.index_cache())) as Box<dyn ModIndex>)
            .collect();
        let offline = self.settings.offline;
        self.index_loaded = true;
        self.jobs.spawn(ctx, move || {
            let (index, warnings) = index_client::load_all(&sources, &App::client(), offline);
            Done::Index { index, warnings }
        });
    }

    pub fn install_mod(&mut self, ctx: &egui::Context, id: &str) {
        let game = self.game_version();
        let Some(item) = self.index.get(id).cloned() else { return };
        let Some(version) = item.best_version(game.as_deref()).cloned() else { return };
        let (mods, downloads) = (self.mods_dir(), self.paths.downloads());
        let label = format!("{} {}", item.name, version.version);
        let mut progress = jobs::reporter(ctx, &self.jobs.progress, label);
        let cancel = self.jobs.cancel.clone();
        cancel.store(false, Ordering::Relaxed);
        let id = id.to_owned();
        self.jobs.spawn(ctx, move || {
            let result =
                index_install::install(&App::client(), &item, &version, &mods, &downloads, &mut progress, &cancel);
            Done::ModInstalled { id, result: result.map(|_| ()).map_err(|e| e.to_string()) }
        });
    }

    pub fn request_image(&mut self, ctx: &egui::Context, url: &str) {
        if self.images.contains_key(url) {
            return;
        }
        self.images.insert(url.to_owned(), ImageState::Loading);
        let (url, cache, offline) = (url.to_owned(), self.paths.cache.clone(), self.settings.offline);
        self.jobs.spawn(ctx, move || {
            let result = index_client::image(&App::client(), &url, &cache, offline).map_err(|e| e.to_string());
            Done::Image { url, result }
        });
    }

    /// Applies finished background jobs.
    pub fn collect_jobs(&mut self, ctx: &egui::Context) {
        for done in self.jobs.finished() {
            match done {
                Done::Index { index, warnings } => {
                    self.index = index;
                    self.index_warnings = warnings;
                }
                Done::Checked(result) => self.on_checked(ctx, result),
                Done::Downloaded(result) => self.on_downloaded(ctx, result),
                Done::Applied(result) => self.on_applied(result),
                Done::ModInstalled { id, result } => {
                    jobs::set(&self.jobs.progress, None);
                    match result {
                        Ok(()) => {
                            let text = self.tr.fill("mods.installed_disabled", &[("id", &id)]);
                            self.info(text);
                        }
                        Err(error) => self.error(error),
                    }
                    self.reload_mods();
                }
                Done::Image { url, result } => {
                    let state = match result {
                        Ok(bytes) => ImageState::Ready(egui::load::Bytes::from(bytes)),
                        Err(_) => ImageState::Failed,
                    };
                    self.images.insert(url, state);
                }
            }
        }
    }

    pub fn reload_language(&mut self) {
        self.tr = Strings::load(&self.settings.language);
    }
}

/// Opens a folder in the system's file manager.
pub fn open_folder(path: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)?;
    #[cfg(windows)]
    let program = "explorer";
    #[cfg(not(windows))]
    let program = "xdg-open";
    std::process::Command::new(program).arg(path).spawn().map(|_| ())
}

/// Opens a folder and reports a failure as a note.
pub fn open_or_note(app: &mut App, path: &std::path::Path) {
    if let Err(error) = open_folder(path) {
        app.error(format!("{}: {error}", path.display()));
    }
}
