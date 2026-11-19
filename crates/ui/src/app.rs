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
use crate::launcher::LauncherUpdate;
use crate::news::NewsState;
use crate::storage::StorageState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Home,
    Mods,
    Screenshots,
    Log,
    Settings,
    About,
}

impl Page {
    /// Sidebar order, which is also the order of Ctrl+1 … Ctrl+6.
    pub const ALL: [Self; 6] = [Self::Home, Self::Mods, Self::Screenshots, Self::Log, Self::Settings, Self::About];
    /// The pages at the top of the sidebar; About sits at the bottom.
    pub const TOP: usize = 5;

    pub fn key(self) -> &'static str {
        match self {
            Self::Home => "nav.play",
            Self::Mods => "nav.mods",
            Self::Screenshots => "nav.screenshots",
            Self::Log => "nav.output",
            Self::Settings => "nav.settings",
            Self::About => "nav.about",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Home => icons::PLAY,
            Self::Mods => icons::PUZZLE,
            Self::Screenshots => icons::DRAWN,
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
    EnableMod {
        file_name: String,
    },
    RemoveMod {
        file_name: String,
    },
    /// Delete one row of the storage list; the game data row has its own question.
    DeleteRow(castanea_launcher_core::storage::Row),
    /// Worlds and game settings: asks for the typed word.
    DeleteGameData,
    DeleteEverything,
    /// A screenshot, to the trash.
    DeleteShot(PathBuf),
    /// A screenshot for good, when the trash did not take it (`reason`).
    DeleteShotForever {
        path: PathBuf,
        reason: String,
    },
    /// Windows: delete everything, then the launcher program.
    #[cfg(windows)]
    Uninstall,
    /// Everything is deleted; the launcher closes.
    Goodbye,
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
    /// The OS account's display name, the default player name until one is typed. Kept in memory only.
    pub os_name: Option<String>,
    pub tr: Strings,
    pub page: Page,
    pub mods_view: ModsView,
    /// The build the game starts from: the installed release, or the local build folder.
    pub installed: Option<Build>,
    pub update: Update,
    pub update_progress: Slot,
    /// The launcher's own update.
    pub launcher: LauncherUpdate,
    pub launcher_progress: Slot,
    pub launcher_checked: bool,
    /// Where the running launcher is installed; `None` for screenshots, which never update.
    pub launcher_layout: Option<castanea_launcher_core::selfupdate::Layout>,
    /// Version and plain-text notes of the newer launcher release.
    pub launcher_notes: Option<(String, String)>,
    pub frames_drawn: u32,
    /// A local build folder for this run only (`--game-dir`, `CASTANEA_GAME_DIR`).
    pub dev_override: Option<PathBuf>,
    /// Look for a newer build at the first frame; off for screenshots.
    pub auto_check: bool,
    pub checked: bool,
    pub session: Option<Session>,
    /// The graphics backend is being decided for a start (the probe may run); Play waits.
    pub starting: bool,
    /// A game was just started and "When the game starts" has not been applied to the window yet.
    pub start_action_due: bool,
    /// The window was minimised because the game started; it is brought back when the game ends.
    pub minimised_for_game: bool,
    /// The last start's backend, for About.
    pub backend_last: Option<castanea_launcher_core::backend::LastLaunch>,
    pub notes: Vec<Note>,
    pub index: Index,
    pub index_warnings: Vec<String>,
    pub index_loaded: bool,
    pub mods: Vec<InstalledMod>,
    pub browse: BrowseState,
    pub confirm: Option<Confirm>,
    pub jobs: Jobs,
    pub images: HashMap<String, ImageState>,
    /// Version and plain-text notes of the release being downloaded.
    pub pending_notes: Option<(String, String)>,
    /// The "What's new" dialog is open.
    pub show_notes: bool,
    /// The release server address being typed in Settings.
    pub release_url_input: String,
    pub new_index: String,
    pub advanced_open: bool,
    /// Scrolls the open Advanced section into view once.
    pub reveal_advanced: bool,
    /// Scrolls the Storage and reset section into view once (screenshots).
    pub reveal_storage: Option<egui::Align>,
    pub focus_search: bool,
    pub news: NewsState,
    pub storage: StorageState,
    pub gallery: crate::gallery::GalleryState,
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
            os_name: castanea_launcher_core::osname::detect(),
            page: Page::Home,
            mods_view: ModsView::Installed,
            installed: None,
            update: Update::Idle,
            update_progress: Slot::default(),
            launcher: LauncherUpdate::Idle,
            launcher_progress: Slot::default(),
            launcher_checked: false,
            launcher_layout: None,
            launcher_notes: None,
            frames_drawn: 0,
            dev_override,
            auto_check: true,
            checked: false,
            session: None,
            starting: false,
            start_action_due: false,
            minimised_for_game: false,
            backend_last: None,
            notes: Vec::new(),
            index: Index::default(),
            index_warnings: Vec::new(),
            index_loaded: false,
            mods: Vec::new(),
            browse: BrowseState::default(),
            confirm: None,
            jobs: Jobs::default(),
            images: HashMap::new(),
            pending_notes: None,
            show_notes: false,
            release_url_input: String::new(),
            new_index: String::new(),
            advanced_open: false,
            reveal_advanced: false,
            reveal_storage: None,
            focus_search: false,
            news: NewsState::default(),
            storage: StorageState::default(),
            gallery: crate::gallery::GalleryState::default(),
        };
        app.load_news_cache();
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
        app.release_url_input = app.settings.release_url.clone();
        app.backend_last = castanea_launcher_core::backend::BackendState::load(&app.paths).last;
        app.refresh_installed();
        app.reload_mods();
        app.adopt_running();
        app
    }

    pub fn client() -> Client {
        Client::new(UrlPolicy::default())
    }

    pub fn go(&mut self, page: Page) {
        if page == Page::Screenshots && self.page != page {
            self.gallery.stale = true;
        }
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
                Done::Backend(result) => self.on_backend(result),
                Done::Checked(result) => self.on_checked(ctx, result),
                Done::Downloaded(result) => self.on_downloaded(ctx, result),
                Done::Applied(result) => self.on_applied(result),
                Done::LauncherChecked(result) => self.on_launcher_checked(ctx, result),
                Done::LauncherApplied(result) => self.on_launcher_applied(result),
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
                Done::News { lang, result } => self.on_news(lang, result),
                Done::ShotsScanned(scan) => self.on_shots_scanned(scan),
                Done::ShotPixels(result) => self.on_shot_pixels(ctx, result),
                Done::StorageSize { row, info } => self.on_storage_info(row, info),
                Done::StorageDeleted { what, result } => self.on_storage_deleted(ctx, what, result),
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
        let before = std::mem::take(&mut self.tr.code);
        self.tr = Strings::load(&self.settings.language);
        if before != self.tr.code {
            self.load_news_cache();
        }
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

/// Opens a web page in the default browser and reports a failure as a note.
pub fn open_url(app: &mut App, url: &str) {
    #[cfg(windows)]
    let program = "explorer";
    #[cfg(not(windows))]
    let program = "xdg-open";
    if let Err(error) = std::process::Command::new(program).arg(url).spawn() {
        app.error(format!("{url}: {error}"));
    }
}

/// Opens a folder and reports a failure as a note.
pub fn open_or_note(app: &mut App, path: &std::path::Path) {
    if let Err(error) = open_folder(path) {
        app.error(format!("{}: {error}", path.display()));
    }
}
