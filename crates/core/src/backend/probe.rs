//! The GPU probe: `castanea --probe-gpu` prints one JSON line and exits (docs/game-launch.md).

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

pub const PROBE_FLAG: &str = "--probe-gpu";
/// The flag's name as the game's binary spells it (an argument parser may store it without the dashes).
const FLAG_NAME: &[u8] = b"probe-gpu";
const OUTPUT_LIMIT: u64 = 64 * 1024;

/// One backend's result in the probe's JSON line.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BackendReport {
    pub ok: bool,
    pub adapter: String,
    pub driver: String,
    pub error: Option<String>,
}

/// The probe's JSON line. A missing `wgpu` means the game did not test it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub vulkan: BackendReport,
    #[serde(default)]
    pub wgpu: Option<BackendReport>,
}

/// Why a game counts as having no probe.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum Unsupported {
    /// The binary does not contain the flag at all (an older game); it was not even started.
    NoFlag,
    /// Exit code 0, 1 or 2 without a JSON line: the flag is unknown.
    Refused(i32),
    Timeout,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Probe {
    Report(Report),
    Unsupported(Unsupported),
    /// The probe process died (a signal, a crash code) without a JSON line: the driver took it down.
    Crashed(Option<i32>),
}

/// Runs the probe of `binary` in `cwd` with `env`, no window, for at most `timeout`.
pub fn run(binary: &Path, cwd: &Path, env: &[(String, String)], timeout: Duration) -> Result<Probe> {
    if !contains_flag(binary) {
        return Ok(Probe::Unsupported(Unsupported::NoFlag));
    }
    let mut command = Command::new(binary);
    command.arg(PROBE_FLAG).current_dir(cwd).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    for (key, value) in env.iter().filter(|(k, _)| !k.trim().is_empty()) {
        command.env(key.trim(), value);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command.spawn().map_err(|error| Error::io(binary, error))?;
    let stdout = child.stdout.take();
    // A grandchild may keep the pipe open after the game is gone, so the text is waited for with a limit.
    let (sender, receiver) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let mut text = Vec::new();
        if let Some(out) = stdout {
            let _ = out.take(OUTPUT_LIMIT).read_to_end(&mut text);
        }
        let _ = sender.send(String::from_utf8_lossy(&text).into_owned());
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait().map_err(|error| Error::io(binary, error))? {
            Some(status) => break Some(status),
            None if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            None => thread::sleep(Duration::from_millis(20)),
        }
    };
    let Some(status) = status else { return Ok(Probe::Unsupported(Unsupported::Timeout)) };
    let text = receiver.recv_timeout(Duration::from_secs(1)).unwrap_or_default();
    Ok(interpret(status.code(), &text))
}

/// The JSON line wins; without one the exit says unknown flag (0, 1 or 2) or a crash (anything else).
pub fn interpret(code: Option<i32>, stdout: &str) -> Probe {
    let line = stdout.lines().rev().map(str::trim).filter(|l| l.starts_with('{')).find_map(parse);
    if let Some(report) = line {
        return Probe::Report(report);
    }
    match code {
        Some(code @ 0..=2) => Probe::Unsupported(Unsupported::Refused(code)),
        other => Probe::Crashed(other),
    }
}

fn parse(line: &str) -> Option<Report> {
    serde_json::from_str(line).ok()
}

/// Whether the binary mentions the probe flag. An older game that would ignore it and open its window is never run.
/// A file that cannot be read is given the benefit of the doubt.
fn contains_flag(binary: &Path) -> bool {
    let Ok(mut file) = std::fs::File::open(binary) else { return true };
    let mut buffer = vec![0u8; 1 << 20];
    let mut carry: Vec<u8> = Vec::new();
    loop {
        let Ok(read) = file.read(&mut buffer) else { return true };
        if read == 0 {
            return false;
        }
        carry.extend_from_slice(&buffer[..read]);
        if carry.windows(FLAG_NAME.len()).any(|w| w == FLAG_NAME) {
            return true;
        }
        let start = carry.len().saturating_sub(FLAG_NAME.len() - 1);
        carry.drain(..start);
    }
}
