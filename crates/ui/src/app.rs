//! The launcher's state and its actions; the tabs draw it (`tabs/`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

use castanea_launcher_core::download::{Client, UrlPolicy};
use castanea_launcher_core::index::client::{self as index_client, ModIndex, StaticIndex};
use castanea_launcher_core::index::{Index, install as index_install};
use castanea_launcher_core::install::github::GithubReleases;
use castanea_launcher_core::install::{GameSource, Registry, RemoteBuild, detect, token};
use castanea_launcher_core::launch::{self, Outcome, Running};
use castanea_launcher_core::mods::{InstalledMod, installed};
use castanea_launcher_core::paths::Paths;
use castanea_launcher_core::profile::{Profile, Profiles};
use castanea_launcher_core::settings::{Settings, SourceConfig};
use eframe::egui;

use crate::i18n::Strings;
use crate::jobs::{Done, Jobs};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Play,
    Mods,
    Versions,
    Settings,
    About,
}

impl Tab {
    pub const ALL: [Self; 5] = [Self::Play, Self::Mods, Self::Versions, Self::Settings, Self::About];

    pub fn key(self) -> &'static str {
        match self {
            Self::Play => "tab.play",
            Self::Mods => "tab.mods",
            Self::Versions => "tab.versions",
            Self::Settings => "tab.settings",
            Self::About => "tab.about",
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
    RemoveBuild { id: String },
    RemoveProfile { id: String },
}

pub enum Note {
    Info(String),
    Error(String),
}

pub struct Game {
    pub running: Running,
    pub command: String,
    pub profile: String,
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
    pub profiles: Profiles,
    pub registry: Registry,
    pub tr: Strings,
    pub tab: Tab,
    pub mods_view: ModsView,
    pub profile_id: String,
    pub editing_profile: bool,
    pub game: Option<Game>,
    pub last_outcome: Option<Outcome>,
    pub log: Vec<String>,
    pub log_total: u64,
    pub notes: Vec<Note>,
    pub index: Index,
    pub index_warnings: Vec<String>,
    pub index_loaded: bool,
    pub releases: Vec<(usize, RemoteBuild)>,
    pub release_errors: Vec<String>,
    pub releases_loaded: bool,
    pub mods: Vec<InstalledMod>,
    pub browse: BrowseState,
    pub confirm: Option<Confirm>,
    pub jobs: Jobs,
    pub images: HashMap<String, ImageState>,
    pub token_input: String,
    pub new_index: String,
    pub new_source: String,
    pub focus_search: bool,
}

impl App {
    pub fn new(paths: Paths) -> Self {
        let mut notes = Vec::new();
        let mut note = |result: Result<(), String>| {
            if let Err(error) = result {
                notes.push(Note::Error(error));
            }
        };
        let settings = Settings::load(&paths).unwrap_or_else(|e| {
            note(Err(e.to_string()));
            Settings::default()
        });
        let profiles = Profiles::load(&paths).unwrap_or_else(|e| {
            note(Err(e.to_string()));
            Profiles { profiles: vec![Profile::default()] }
        });
        let registry = detect::refresh(&paths).unwrap_or_else(|e| {
            note(Err(e.to_string()));
            Registry::default()
        });
        let profile_id = settings
            .last_profile
            .clone()
            .filter(|id| profiles.get(id).is_some())
            .unwrap_or_else(|| profiles.profiles[0].id.clone());
        let mut app = Self {
            tr: Strings::load(&settings.language),
            paths,
            settings,
            profiles,
            registry,
            tab: Tab::Play,
            mods_view: ModsView::Installed,
            profile_id,
            editing_profile: false,
            game: None,
            last_outcome: None,
            log: Vec::new(),
            log_total: 0,
            notes,
            index: Index::default(),
            index_warnings: Vec::new(),
            index_loaded: false,
            releases: Vec::new(),
            release_errors: Vec::new(),
            releases_loaded: false,
            mods: Vec::new(),
            browse: BrowseState::default(),
            confirm: None,
            jobs: Jobs::default(),
            images: HashMap::new(),
            token_input: String::new(),
            new_index: String::new(),
            new_source: String::new(),
            focus_search: false,
        };
        app.reload_mods();
        app
    }

    pub fn client() -> Client {
        Client::new(UrlPolicy::default())
    }

    pub fn profile(&self) -> &Profile {
        self.profiles.get(&self.profile_id).unwrap_or(&self.profiles.profiles[0])
    }

    pub fn profile_mut(&mut self) -> &mut Profile {
        let id = self.profile_id.clone();
        if self.profiles.get(&id).is_none() {
            self.profile_id = self.profiles.profiles[0].id.clone();
        }
        let id = self.profile_id.clone();
        self.profiles.get_mut(&id).expect("profile exists")
    }

    /// The version of the build the current profile would start, for compatibility checks.
    pub fn game_version(&self) -> Option<String> {
        launch::pick_build(&self.registry, self.profile()).ok().map(|b| b.version.clone())
    }

    pub fn error(&mut self, text: impl Into<String>) {
        self.notes.push(Note::Error(text.into()));
    }

    pub fn info(&mut self, text: impl Into<String>) {
        self.notes.push(Note::Info(text.into()));
    }

    pub fn save_settings(&mut self) {
        if let Err(error) = self.settings.save(&self.paths) {
            self.error(error.to_string());
        }
    }

    pub fn save_profiles(&mut self) {
        if let Err(error) = self.profiles.save(&self.paths) {
            self.error(error.to_string());
        }
    }

    pub fn save_registry(&mut self) {
        if let Err(error) = self.registry.save(&self.paths) {
            self.error(error.to_string());
        }
    }

    pub fn set_profile(&mut self, id: String) {
        self.profile_id = id.clone();
        self.settings.last_profile = Some(id);
        self.save_settings();
        self.reload_mods();
    }

    pub fn reload_mods(&mut self) {
        let dir = self.profile().mods_dir(&self.paths);
        match installed::list(&dir) {
            Ok(list) => self.mods = list,
            Err(error) => {
                self.mods.clear();
                self.error(error.to_string());
            }
        }
    }

    pub fn mods_dir(&self) -> PathBuf {
        self.profile().mods_dir(&self.paths)
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

    pub fn load_releases(&mut self, ctx: &egui::Context) {
        self.releases_loaded = true;
        if self.settings.offline {
            self.releases.clear();
            return;
        }
        let token = token::read(&self.paths);
        let sources = self.settings.sources.clone();
        self.jobs.spawn(ctx, move || {
            let client = App::client();
            let mut builds = Vec::new();
            let mut errors = Vec::new();
            for (n, source) in sources.iter().enumerate() {
                let SourceConfig::Github { repo } = source;
                match GithubReleases::new(repo, token.clone()).list(&client) {
                    Ok(list) => builds.extend(list.into_iter().map(|b| (n, b))),
                    Err(error) => errors.push(error.to_string()),
                }
            }
            Done::Releases { builds, errors }
        });
    }

    pub fn install_release(&mut self, ctx: &egui::Context, source: usize, build: RemoteBuild) {
        let Some(SourceConfig::Github { repo }) = self.settings.sources.get(source).cloned() else { return };
        let token = token::read(&self.paths);
        let (versions, downloads) = (self.paths.versions(), self.paths.downloads());
        let mut progress = self.jobs.reporter(ctx, build.title.clone());
        let cancel = self.jobs.cancel.clone();
        cancel.store(false, Ordering::Relaxed);
        self.jobs.spawn(ctx, move || {
            let source = GithubReleases::new(&repo, token);
            let result = source.install(&App::client(), &build, &versions, &downloads, &mut progress, &cancel);
            Done::BuildInstalled(result.map_err(|e| e.to_string()))
        });
    }

    pub fn install_mod(&mut self, ctx: &egui::Context, id: &str) {
        let game = self.game_version();
        let Some(item) = self.index.get(id).cloned() else { return };
        let Some(version) = item.best_version(game.as_deref()).cloned() else { return };
        let (mods, downloads) = (self.mods_dir(), self.paths.downloads());
        let mut progress = self.jobs.reporter(ctx, format!("{} {}", item.name, version.version));
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
    pub fn collect_jobs(&mut self) {
        for done in self.jobs.finished() {
            match done {
                Done::Index { index, warnings } => {
                    self.index = index;
                    self.index_warnings = warnings;
                }
                Done::Releases { builds, errors } => {
                    self.releases = builds;
                    self.release_errors = errors;
                }
                Done::BuildInstalled(result) => {
                    self.jobs.clear_progress();
                    match result {
                        Ok(build) => {
                            let text = self.tr.fill("versions.installed", &[("version", &build.id)]);
                            self.registry.put(build);
                            self.save_registry();
                            self.info(text);
                        }
                        Err(error) => self.error(error),
                    }
                }
                Done::ModInstalled { id, result } => {
                    self.jobs.clear_progress();
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

    pub fn play(&mut self) {
        if self.game.is_some() {
            return;
        }
        let profile = self.profile().clone();
        match launch::launch(&self.paths, &self.registry, &profile, self.settings.log_lines) {
            Ok((plan, running)) => {
                self.log.clear();
                self.log_total = 0;
                self.last_outcome = None;
                self.game = Some(Game { running, command: plan.display(), profile: profile.id });
            }
            Err(error) => self.error(error.to_string()),
        }
    }

    /// Copies new output and notices the exit; called every frame while a game runs.
    pub fn poll_game(&mut self) {
        let Some(game) = &mut self.game else { return };
        if let Ok(log) = game.running.log.lock()
            && log.total != self.log_total
        {
            self.log = log.lines.iter().cloned().collect();
            self.log_total = log.total;
        }
        match game.running.poll() {
            Ok(None) => {}
            Ok(Some(outcome)) => {
                if let Ok(log) = game.running.log.lock() {
                    self.log = log.lines.iter().cloned().collect();
                }
                self.last_outcome = Some(outcome);
                self.game = None;
            }
            Err(error) => {
                self.game = None;
                self.error(error.to_string());
            }
        }
    }

    pub fn stop_game(&mut self) {
        if let Some(game) = &mut self.game {
            match game.running.stop() {
                Ok(outcome) => self.last_outcome = Some(outcome),
                Err(error) => self.notes.push(Note::Error(error.to_string())),
            }
            self.game = None;
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
