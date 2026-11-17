//! A running game: output to `logs/latest.log` and a buffer for the UI, the exit, and crash reports.

use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use super::plan::Plan;
use crate::{Error, Result};

pub const LOG_DIR: &str = "logs";
pub const CRASH_DIR: &str = "crash";
const KEEP_LOGS: usize = 10;

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
}

pub struct Running {
    child: Child,
    pub log: Arc<Mutex<LogBuffer>>,
    pub log_file: PathBuf,
    game_dir: PathBuf,
    crashes_before: HashSet<PathBuf>,
    started: Instant,
    readers: Vec<thread::JoinHandle<()>>,
}

impl Running {
    /// Starts the plan; the previous `latest.log` is kept with a time stamp.
    pub fn start(plan: &Plan, log_limit: usize) -> Result<Self> {
        let logs = plan.cwd.join(LOG_DIR);
        fs::create_dir_all(&logs).map_err(|error| Error::io(&logs, error))?;
        let log_file = logs.join("latest.log");
        rotate(&logs, &log_file);
        let mut file = fs::File::create(&log_file).map_err(|error| Error::io(&log_file, error))?;
        let _ = writeln!(file, "# {}\n# in {}", plan.display(), plan.cwd.display());
        let file = Arc::new(Mutex::new(file));
        let crashes_before = crash_files(&plan.cwd);

        let mut command = Command::new(&plan.program);
        command.args(&plan.args).current_dir(&plan.cwd).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        for (key, value) in &plan.env {
            command.env(key, value);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command.spawn().map_err(|error| Error::io(&plan.program, error))?;
        let log = Arc::new(Mutex::new(LogBuffer { limit: log_limit, ..LogBuffer::default() }));
        let mut readers = Vec::new();
        if let Some(out) = child.stdout.take() {
            readers.push(reader(out, log.clone(), file.clone()));
        }
        if let Some(err) = child.stderr.take() {
            readers.push(reader(err, log.clone(), file.clone()));
        }
        Ok(Self { child, log, log_file, game_dir: plan.cwd.clone(), crashes_before, started: Instant::now(), readers })
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// `Some` once the game has exited; it waits for the last output then.
    pub fn poll(&mut self) -> Result<Option<Outcome>> {
        let status = self.child.try_wait().map_err(|error| Error::io(&self.game_dir, error))?;
        Ok(status.map(|status| self.finish(status)))
    }

    /// Ends the game (kill), then reports as `poll` does.
    pub fn stop(&mut self) -> Result<Outcome> {
        let _ = self.child.kill();
        let status = self.child.wait().map_err(|error| Error::io(&self.game_dir, error))?;
        Ok(self.finish(status))
    }

    fn finish(&mut self, status: ExitStatus) -> Outcome {
        for reader in self.readers.drain(..) {
            let _ = reader.join();
        }
        let crash_reports: Vec<PathBuf> = {
            let mut new: Vec<PathBuf> = crash_files(&self.game_dir).difference(&self.crashes_before).cloned().collect();
            new.sort();
            new
        };
        Outcome {
            code: status.code(),
            crashed: !status.success() || !crash_reports.is_empty(),
            crash_reports,
            seconds: self.started.elapsed().as_secs(),
        }
    }
}

fn reader(stream: impl Read + Send + 'static, log: Arc<Mutex<LogBuffer>>, file: Arc<Mutex<fs::File>>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut lines = BufReader::new(stream);
        let mut raw = Vec::new();
        loop {
            raw.clear();
            match lines.read_until(b'\n', &mut raw) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let line = String::from_utf8_lossy(&raw).trim_end_matches(['\n', '\r']).to_owned();
                    if let Ok(mut file) = file.lock() {
                        let _ = writeln!(file, "{line}");
                    }
                    if let Ok(mut log) = log.lock() {
                        log.push(line);
                    }
                }
            }
        }
    })
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
