//! A running game: output to `logs/latest.log` and a buffer for the UI, the exit, and crash reports.

use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::detach;
use super::plan::Plan;
use super::tail::Tail;
use crate::{Error, Result};

pub const LOG_DIR: &str = "logs";
pub const CRASH_DIR: &str = "crash";
const KEEP_LOGS: usize = 10;
/// How much of the old output an adopted game shows.
const ADOPT_TAIL_BYTES: u64 = 64 * 1024;

/// The last lines of output, shared with the reader threads.
#[derive(Debug, Default)]
pub struct LogBuffer {
    pub lines: VecDeque<String>,
    pub limit: usize,
    /// Counts every line ever added, so the UI sees new lines even when old ones drop out.
    pub total: u64,
}

impl LogBuffer {
    pub fn push(&mut self, line: String) {
        self.lines.push_back(line);
        self.total += 1;
        while self.lines.len() > self.limit.max(1) {
            self.lines.pop_front();
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub code: Option<i32>,
    /// A non-zero exit, a signal, or a new crash report.
    pub crashed: bool,
    pub crash_reports: Vec<PathBuf>,
    pub seconds: u64,
    /// The shared library the loader could not find (`libasound.so.2`), read from the output.
    pub missing_library: Option<String>,
}

/// The library name in an `error while loading shared libraries: <lib>: cannot open ...` line.
pub fn missing_library<'a>(lines: impl IntoIterator<Item = &'a str>) -> Option<String> {
    const MARK: &str = "error while loading shared libraries:";
    lines.into_iter().find_map(|line| {
        let rest = line[line.find(MARK)? + MARK.len()..].trim_start();
        let name = rest.split(':').next()?.trim();
        (!name.is_empty()).then(|| name.to_owned())
    })
}

/// The package that provides a library, for a hint; `None` when the name is not known.
pub fn library_package(library: &str) -> Option<&'static str> {
    const TABLE: [(&str, &str); 8] = [
        ("libasound.so", "alsa-lib / libasound2"),
        ("libvulkan.so", "vulkan-loader / libvulkan1"),
        ("libxkbcommon", "libxkbcommon"),
        ("libwayland", "wayland / libwayland-client0"),
        ("libudev.so", "systemd (libudev) / libudev1"),
        ("libX11", "libX11 / libx11-6"),
        ("libxcb", "libxcb / libxcb1"),
        ("libGL", "libGL / libgl1"),
    ];
    TABLE.iter().find(|(prefix, _)| library.starts_with(prefix)).map(|(_, package)| *package)
}

/// The game: a child of this launcher run, or one started by an earlier run that is still going.
enum Handle {
    Child(Child),
    Adopted { pid: u32, program: PathBuf },
}

pub struct Running {
    handle: Handle,
    pub log: Arc<Mutex<LogBuffer>>,
    pub log_file: PathBuf,
    game_dir: PathBuf,
    crashes_before: HashSet<PathBuf>,
    started: Instant,
    tail: Tail,
}

/// The command of the game: its own session or process group, stdin closed, output straight into the log file.
fn command(plan: &Plan, out: fs::File, err: fs::File, breakaway: bool) -> Command {
    let mut command = Command::new(&plan.program);
    command.args(&plan.args).current_dir(&plan.cwd).stdin(Stdio::null()).stdout(out).stderr(err);
    for (key, value) in &plan.env {
        command.env(key, value);
    }
    detach::detach(&mut command, breakaway);
    command
}

fn now_seconds() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl Running {
    /// Starts the plan; the previous `latest.log` is kept with a time stamp. The game writes the log file itself and is not
    /// tied to this process in any way: closing the launcher leaves it running.
    pub fn start(plan: &Plan, log_limit: usize) -> Result<Self> {
        let logs = plan.cwd.join(LOG_DIR);
        fs::create_dir_all(&logs).map_err(|error| Error::io(&logs, error))?;
        let log_file = logs.join("latest.log");
        rotate(&logs, &log_file);
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
            .map_err(|error| Error::io(&log_file, error))?;
        let _ = writeln!(file, "# {}\n# in {}", plan.display(), plan.cwd.display());
        let from = file.metadata().map_or(0, |m| m.len());
        let crashes_before = crash_files(&plan.cwd);
        let clone = |file: &fs::File| file.try_clone().map_err(|error| Error::io(&log_file, error));
        let spawn = |breakaway| -> Result<Child> {
            let mut command = command(plan, clone(&file)?, clone(&file)?, breakaway);
            command.spawn().map_err(|error| Error::io(&plan.program, error))
        };
        // A job object that forbids breakaway refuses the flag; the game then starts without it.
        let child = match spawn(true) {
            Err(_) if cfg!(windows) => spawn(false)?,
            other => other?,
        };
        drop(file);
        let log = Arc::new(Mutex::new(LogBuffer { limit: log_limit, ..LogBuffer::default() }));
        let tail = Tail::start(log_file.clone(), from, log.clone());
        let record = detach::Record {
            pid: child.id(),
            program: plan.program.clone(),
            started: now_seconds(),
            log_file: log_file.clone(),
            command: plan.display(),
        };
        record.save(&plan.cwd);
        Ok(Self {
            handle: Handle::Child(child),
            log,
            log_file,
            game_dir: plan.cwd.clone(),
            crashes_before,
            started: Instant::now(),
            tail,
        })
    }

    /// The game of an earlier launcher run in `game_dir`, when it is still running; its recent output is shown.
    pub fn adopt(game_dir: &Path, log_limit: usize) -> Option<(Self, String)> {
        let record = detach::Record::load(game_dir)?;
        if !detach::is_alive(record.pid, &record.program) {
            detach::Record::remove(game_dir);
            return None;
        }
        let log = Arc::new(Mutex::new(LogBuffer { limit: log_limit, ..LogBuffer::default() }));
        let size = fs::metadata(&record.log_file).map_or(0, |m| m.len());
        let tail = Tail::start(record.log_file.clone(), size.saturating_sub(ADOPT_TAIL_BYTES), log.clone());
        let age = Duration::from_secs(now_seconds().saturating_sub(record.started));
        let running = Self {
            handle: Handle::Adopted { pid: record.pid, program: record.program },
            log,
            log_file: record.log_file,
            game_dir: game_dir.to_path_buf(),
            crashes_before: crash_files(game_dir),
            started: Instant::now().checked_sub(age).unwrap_or_else(Instant::now),
            tail,
        };
        Some((running, record.command))
    }

    pub fn pid(&self) -> u32 {
        match &self.handle {
            Handle::Child(child) => child.id(),
            Handle::Adopted { pid, .. } => *pid,
        }
    }

    /// `Some` once the game has exited; it waits for the last output then.
    pub fn poll(&mut self) -> Result<Option<Outcome>> {
        let status = match &mut self.handle {
            Handle::Child(child) => match child.try_wait().map_err(|error| Error::io(&self.game_dir, error))? {
                Some(status) => Some(status),
                None => return Ok(None),
            },
            Handle::Adopted { pid, program } => {
                if detach::is_alive(*pid, program) {
                    return Ok(None);
                }
                None
            }
        };
        Ok(Some(self.finish(status)))
    }

    /// Ends the game (kill), then reports as `poll` does. Only the stop button calls this, never closing the launcher.
    pub fn stop(&mut self) -> Result<Outcome> {
        let status = match &mut self.handle {
            Handle::Child(child) => {
                let _ = child.kill();
                Some(child.wait().map_err(|error| Error::io(&self.game_dir, error))?)
            }
            Handle::Adopted { pid, program } => {
                detach::kill(*pid);
                for _ in 0..50 {
                    if !detach::is_alive(*pid, program) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                None
            }
        };
        Ok(self.finish(status))
    }

    fn finish(&mut self, status: Option<ExitStatus>) -> Outcome {
        self.tail.finish();
        detach::Record::remove(&self.game_dir);
        let crash_reports: Vec<PathBuf> = {
            let mut new: Vec<PathBuf> = crash_files(&self.game_dir).difference(&self.crashes_before).cloned().collect();
            new.sort();
            new
        };
        let missing_library =
            self.log.lock().ok().and_then(|log| missing_library(log.lines.iter().map(String::as_str)));
        Outcome {
            code: status.and_then(|s| s.code()),
            crashed: status.is_some_and(|s| !s.success()) || !crash_reports.is_empty(),
            crash_reports,
            seconds: self.started.elapsed().as_secs(),
            missing_library,
        }
    }
}

fn crash_files(game_dir: &Path) -> HashSet<PathBuf> {
    fs::read_dir(game_dir.join(CRASH_DIR))
        .map(|entries| entries.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect())
        .unwrap_or_default()
}

/// Renames `latest.log` to `<seconds>.log` and keeps the newest `KEEP_LOGS` of those.
fn rotate(logs: &Path, latest: &Path) {
    if latest.is_file() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let _ = fs::rename(latest, logs.join(format!("{stamp}.log")));
    }
    let Ok(entries) = fs::read_dir(logs) else { return };
    let mut old: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_stem().and_then(|s| s.to_str()).is_some_and(|s| s.bytes().all(|b| b.is_ascii_digit())))
        .collect();
    old.sort();
    while old.len() > KEEP_LOGS {
        let _ = fs::remove_file(old.remove(0));
    }
}
