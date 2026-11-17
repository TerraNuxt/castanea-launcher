//! The launcher's state and its actions; the pages draw it (`pages/`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use castanea_launcher_core::download::{Client, UrlPolicy};
use castanea_launcher_core::index::client::{self as index_client, ModIndex, StaticIndex};
use castanea_launcher_core::index::{Index, install as index_install};
use castanea_launcher_core::install::github::GithubReleases;
use castanea_launcher_core::install::{GameSource, Registry, RemoteBuild, detect, token};
use castanea_launcher_core::launch::{self, Outcome, Running};
use castanea_launcher_core::mods::{InstalledMod, installed};
use castanea_launcher_core::paths::Paths;
use castanea_launcher_core::profile::{Profile, Profiles, StartIn};
use castanea_launcher_core::settings::{Settings, SourceConfig};
use eframe::egui;

use crate::i18n::Strings;
use crate::icons;
use crate::jobs::{Done, Jobs};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Home,
    Mods,
    Profiles,
    Versions,
    Log,
    Notifications,
    Settings,
    About,
}

impl Page {
    /// Sidebar order, which is also the order of Ctrl+1 … Ctrl+8.
    pub const ALL: [Self; 8] = [
        Self::Home,
        Self::Mods,
        Self::Profiles,
        Self::Versions,
        Self::Log,
        Self::Notifications,
        Self::Settings,
        Self::About,
    ];
    /// The pages at the top of the sidebar; the others sit at the bottom.
    pub const TOP: usize = 5;

    pub fn key(self) -> &'static str {
        match self {
            Self::Home => "nav.home",
            Self::Mods => "nav.mods",
            Self::Profiles => "nav.profiles",
            Self::Versions => "nav.versions",
            Self::Log => "nav.log",
            Self::Notifications => "nav.notifications",
            Self::Settings => "nav.settings",
            Self::About => "nav.about",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Home => icons::HOUSE,
            Self::Mods => icons::PUZZLE,
            Self::Profiles => icons::STACK,
            Self::Versions => icons::CUBE,
            Self::Log => icons::TERMINAL,
            Self::Notifications => icons::BELL,
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
    RemoveBuild { id: String },
    RemoveProfile { id: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteKind {
    Info,
    Error,
}

/// A message for the notifications page; new ones also show as a toast.
pub struct Note {
    pub kind: NoteKind,
    pub text: String,
    pub created: Instant,
    pub seen: bool,
}

/// One start of the game for one profile: running, or ended with an outcome. Each profile has at most one.
pub struct Session {
    pub profile: String,
    pub name: String,
    pub command: String,
    pub running: Option<Running>,
    pub log: Vec<String>,
    pub log_total: u64,
    pub outcome: Option<Outcome>,
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
    pub page: Page,
    pub page_changed: Option<f64>,
    pub mods_view: ModsView,
    pub profile_id: String,
    pub sessions: Vec<Session>,
    pub log_profile: Option<String>,
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
    pub join_address: String,
    pub focus_search: bool,
}

pub fn now_seconds() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl App {
    pub fn new(paths: Paths) -> Self {
        let mut errors = Vec::new();
        let mut note = |error: String| errors.push(error);
        let settings = Settings::load(&paths).unwrap_or_else(|e| {
            note(e.to_string());
            Settings::default()
        });
        let profiles = Profiles::load(&paths).unwrap_or_else(|e| {
            note(e.to_string());
            Profiles { profiles: vec![Profile::default()] }
        });
        let registry = detect::refresh(&paths).unwrap_or_else(|e| {
            note(e.to_string());
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
            page: Page::Home,
            page_changed: None,
            mods_view: ModsView::Installed,
            profile_id,
            sessions: Vec::new(),
            log_profile: None,
            notes: Vec::new(),
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
            join_address: String::new(),
            focus_search: false,
        };
        for error in errors {
            app.error(error);
        }
        app.reload_mods();
        app
    }

    pub fn client() -> Client {
        Client::new(UrlPolicy::default())
    }

    pub fn go(&mut self, page: Page) {
        if self.page != page {
            self.page = page;
            self.page_changed = None;
        }
        if page == Page::Notifications {
            self.notes.iter_mut().for_each(|n| n.seen = true);
        }
    }

    pub fn profile(&self) -> &Profile {
        self.profiles.get(&self.profile_id).unwrap_or(&self.profiles.profiles[0])
    }

    pub fn profile_mut(&mut self) -> &mut Profile {
        if self.profiles.get(&self.profile_id).is_none() {
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
        self.notes.push(Note { kind: NoteKind::Error, text: text.into(), created: Instant::now(), seen: false });
    }

    pub fn info(&mut self, text: impl Into<String>) {
        self.notes.push(Note { kind: NoteKind::Info, text: text.into(), created: Instant::now(), seen: false });
    }

    pub fn unread(&self) -> usize {
        self.notes.iter().filter(|n| !n.seen).count()
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
        if id == self.profile_id {
            return;
        }
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

    pub fn session(&self, profile: &str) -> Option<&Session> {
        self.sessions.iter().find(|s| s.profile == profile)
    }

    pub fn is_running(&self, profile: &str) -> bool {
        self.session(profile).is_some_and(|s| s.running.is_some())
    }

    pub fn any_running(&self) -> bool {
        self.sessions.iter().any(|s| s.running.is_some())
    }

    /// Starts the current profile.
    pub fn play(&mut self) {
        let id = self.profile_id.clone();
        self.play_profile(&id, None);
    }

    /// Starts a profile, optionally just this once with another start (joining an address from the home page).
    pub fn play_profile(&mut self, id: &str, start: Option<StartIn>) {
        if self.is_running(id) {
            return;
        }
        let Some(mut profile) = self.profiles.get(id).cloned() else { return };
        if let Some(start) = start {
            profile.start = start;
        }
        match launch::launch(&self.paths, &self.registry, &profile, self.settings.log_lines) {
            Ok((plan, running)) => {
                self.sessions.retain(|s| s.profile != id);
                self.sessions.push(Session {
                    profile: id.to_owned(),
                    name: profile.name.clone(),
                    command: plan.display(),
                    running: Some(running),
                    log: Vec::new(),
                    log_total: 0,
                    outcome: None,
                });
                self.log_profile = Some(id.to_owned());
                if let Some(saved) = self.profiles.get_mut(id) {
                    saved.last_played = Some(now_seconds());
                }
                self.save_profiles();
            }
            Err(error) => self.error(error.to_string()),
        }
    }

    /// Copies new output and notices exits; called every frame while a game runs.
    pub fn poll_sessions(&mut self) {
        let mut ended = Vec::new();
        let mut failed = Vec::new();
        for session in &mut self.sessions {
            let Some(running) = &mut session.running else { continue };
            if let Ok(log) = running.log.lock()
                && log.total != session.log_total
            {
                session.log = log.lines.iter().cloned().collect();
                session.log_total = log.total;
            }
            match running.poll() {
                Ok(None) => {}
                Ok(Some(outcome)) => {
                    if let Ok(log) = running.log.lock() {
                        session.log = log.lines.iter().cloned().collect();
                    }
                    ended.push((session.name.clone(), outcome.clone()));
                    session.outcome = Some(outcome);
                    session.running = None;
                }
                Err(error) => {
                    failed.push(error.to_string());
                    session.running = None;
                }
            }
        }
        for (name, outcome) in ended {
            let code = outcome.code.map_or("-".to_owned(), |c| c.to_string());
            if outcome.crashed {
                let text = self.tr.fill("notes.crashed", &[("profile", &name), ("code", &code)]);
                self.error(text);
            } else {
                let text = self.tr.fill("notes.ended", &[("profile", &name)]);
                self.info(text);
            }
        }
        for error in failed {
            self.error(error);
        }
    }

    pub fn stop(&mut self, profile: &str) {
        let mut result = None;
        if let Some(session) = self.sessions.iter_mut().find(|s| s.profile == profile)
            && let Some(mut running) = session.running.take()
        {
            match running.stop() {
                Ok(outcome) => {
                    if let Ok(log) = running.log.lock() {
                        session.log = log.lines.iter().cloned().collect();
                    }
                    session.outcome = Some(outcome);
                }
                Err(error) => result = Some(error.to_string()),
            }
        }
        if let Some(error) = result {
            self.error(error);
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
