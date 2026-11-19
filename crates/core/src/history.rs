//! What was played last, for the Continue card (docs/quick-start.md): the launcher's own record of the starts it made, and
//! the game's saves folder, read-only, for the worlds it cannot know from the record.
//!
//! The launcher knows when it started the game and, when it started it into a world or onto a server, which one. A start at the
//! game's menu is resolved afterwards: the world whose files changed while that game ran (the game's own `last played` rule, the
//! newest file below the world folder) is the one that was played. A server joined from the menu leaves no trace the launcher can
//! read, so only starts that name the server (Continue, `--join` in the extra arguments) are remembered.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::json_file;
use crate::launch::start::{Start, valid_world};
use crate::paths::Paths;

/// Starts kept in the record, oldest dropped first.
pub const KEEP: usize = 100;
/// The game's saves folder below its working directory, and the file that makes a folder a world (castanea `planet/save`).
pub const SAVES: &str = "saves";
pub const WORLD_META: &str = "world.meta";
/// The picture the game keeps in a world's folder.
pub const PREVIEW: &str = "preview.png";
/// Folder levels below a world that its activity time looks into (the game's own depth).
const DEPTH: usize = 3;
/// Slack around a play session when matching a world's file times to it.
const SLACK_MS: i64 = 5_000;

/// One start of the game by the launcher; times are milliseconds since 1970.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub started: i64,
    /// When the launcher saw the game end; none while it runs or when the launcher was closed meanwhile.
    #[serde(default)]
    pub ended: Option<i64>,
    #[serde(default)]
    pub version: String,
    #[serde(flatten)]
    pub start: Start,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct History {
    /// Oldest first.
    pub entries: Vec<Entry>,
}

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

impl History {
    /// The record; a missing or broken file is an empty one (the record is a convenience, never a reason to fail).
    pub fn load(paths: &Paths) -> Self {
        json_file::load_or_default(&paths.history_file()).unwrap_or_default()
    }

    pub fn save(&self, paths: &Paths) -> crate::Result<()> {
        json_file::save(&paths.history_file(), self)
    }

    pub fn record_start(&mut self, entry: Entry) {
        self.entries.push(entry);
        let extra = self.entries.len().saturating_sub(KEEP);
        self.entries.drain(..extra);
    }

    /// Marks the start that began at `started` as ended at `ended`.
    pub fn record_end(&mut self, started: i64, ended: i64) {
        if let Some(entry) = self.entries.iter_mut().rev().find(|e| e.started == started) {
            entry.ended = Some(ended.max(started));
        }
    }
}

/// Records a start in the file and returns its key (the start time).
pub fn record_start(paths: &Paths, start: Start, version: &str) -> crate::Result<i64> {
    let mut history = History::load(paths);
    let started = now_ms();
    history.record_start(Entry { started, ended: None, version: version.to_owned(), start });
    history.save(paths)?;
    Ok(started)
}

/// Records that the start with this key ended now.
pub fn record_end(paths: &Paths, started: i64) -> crate::Result<()> {
    let mut history = History::load(paths);
    history.record_end(started, now_ms());
    history.save(paths)
}

/// What the Continue card shows and starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Played {
    /// `World` or `Join`, never `Menu`.
    pub start: Start,
    /// The world's folder name, or the server's saved name (else its address).
    pub label: String,
    /// When it was played last, milliseconds since 1970.
    pub when_ms: i64,
    /// The game version of that start, when the record has it.
    pub version: Option<String>,
    /// The start and end of that play session, to find its screenshots.
    pub window: Option<(i64, i64)>,
    /// The world's own picture, when it has one.
    pub preview: Option<PathBuf>,
}

/// The newest change below `dir` (to `DEPTH` levels, links skipped), milliseconds since 1970: the game's "last played".
pub fn activity_ms(dir: &Path) -> Option<i64> {
    fn walk(dir: &Path, depth: usize, newest: &mut Option<i64>) {
        for entry in fs::read_dir(dir).ok().into_iter().flatten().flatten() {
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                if depth > 0 {
                    walk(&entry.path(), depth - 1, newest);
                }
            } else if let Ok(time) = entry.metadata().and_then(|m| m.modified()) {
                let ms = time.duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(0));
                *newest = Some(newest.map_or(ms, |old| old.max(ms)));
            }
        }
    }
    let mut newest = None;
    walk(dir, DEPTH, &mut newest);
    newest
}

/// The worlds in `saves` with their last activity, newest first. A world is a folder with a valid name and a `world.meta`.
pub fn worlds(saves: &Path) -> Vec<(String, i64)> {
    let mut found: Vec<(String, i64)> = fs::read_dir(saves)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_owned();
            let dir = entry.path();
            if !valid_world(&name) || !entry.file_type().ok()?.is_dir() || !dir.join(WORLD_META).is_file() {
                return None;
            }
            Some((name, activity_ms(&dir)?))
        })
        .collect();
    found.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    found
}

/// The world whose files changed between `from` and `to` (the newest such change wins).
pub fn world_in_window(saves: &Path, from: i64, to: i64) -> Option<(String, i64)> {
    worlds(saves).into_iter().find(|(_, at)| (from..=to).contains(at))
}

/// The name a saved server has in the game's `servers.txt` (`name<TAB>address` per line), if the address is saved.
pub fn server_name(game_dir: &Path, address: &str) -> Option<String> {
    let text = fs::read_to_string(game_dir.join("servers.txt")).ok()?;
    text.lines().find_map(|line| {
        let (name, saved) = line.split_once('\t')?;
        let name: String = name.chars().filter(|c| !c.is_control()).collect();
        (saved.trim() == address && !name.trim().is_empty()).then(|| name.trim().to_owned())
    })
}

fn preview_of(saves: &Path, world: &str) -> Option<PathBuf> {
    let path = saves.join(world).join(PREVIEW);
    path.is_file().then_some(path)
}

/// The last world or server played: the newest start of the record that can be resolved (a world that still exists, a
/// server address, or the world a menu start played); without any, the world the game itself saw played last. None when
/// nothing was played yet. `now` closes the window of a start that has no end.
pub fn last_played(history: &History, game_dir: &Path, now: i64) -> Option<Played> {
    let saves = game_dir.join(SAVES);
    let mut newer_start = None;
    for entry in history.entries.iter().rev() {
        let end = entry.ended.or(newer_start).unwrap_or(now).max(entry.started);
        newer_start = Some(entry.started);
        let window = Some((entry.started, end));
        let version = Some(entry.version.clone()).filter(|v| !v.is_empty());
        let world_found = |name: &str, when: i64| Played {
            start: Start::World(name.to_owned()),
            label: name.to_owned(),
            when_ms: when,
            version: version.clone(),
            window,
            preview: preview_of(&saves, name),
        };
        match &entry.start {
            Start::World(name) if saves.join(name).join(WORLD_META).is_file() => {
                let when = activity_ms(&saves.join(name)).filter(|at| *at >= entry.started).unwrap_or(entry.started);
                return Some(world_found(name, when.min(end.max(entry.started))));
            }
            Start::World(_) => {}
            Start::Join(address) => {
                let label = server_name(game_dir, address).unwrap_or_else(|| address.clone());
                return Some(Played {
                    start: Start::Join(address.clone()),
                    label,
                    when_ms: entry.ended.unwrap_or(entry.started),
                    version,
                    window,
                    preview: None,
                });
            }
            Start::Menu => {
                if let Some((name, at)) = world_in_window(&saves, entry.started - SLACK_MS, end + SLACK_MS) {
                    return Some(world_found(&name, at));
                }
            }
        }
    }
    let (name, at) = worlds(&saves).into_iter().next()?;
    Some(Played {
        start: Start::World(name.clone()),
        label: name.clone(),
        when_ms: at,
        version: None,
        window: None,
        preview: preview_of(&saves, &name),
    })
}

/// How long ago something was, for a line such as `2 h ago`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ago {
    JustNow,
    Minutes(i64),
    Hours(i64),
    Days(i64),
    /// A month or more: the date is shown instead.
    Date,
}

pub fn ago(now_ms: i64, then_ms: i64) -> Ago {
    let minutes = (now_ms - then_ms).max(0) / 60_000;
    match minutes {
        0 => Ago::JustNow,
        1..=59 => Ago::Minutes(minutes),
        60..=1439 => Ago::Hours(minutes / 60),
        1440..=43_199 => Ago::Days(minutes / 1440),
        _ => Ago::Date,
    }
}

#[cfg(test)]
mod tests;
