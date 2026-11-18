//! Keeping the game independent of the launcher: its own session or process group, no pipes to the launcher, and a
//! `running.json` that lets the next launcher start find a game that is still running.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::json_file;

pub const RUNNING_FILE: &str = "running.json";

pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
pub const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
pub const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;

/// The Windows creation flags of the game: no console window (not even for a console program or for a console child the game
/// starts: with `DETACHED_PROCESS` such a child opened its own window), no Ctrl+C or Ctrl+Break shared with the launcher, and out of the
/// launcher's job object when that allows it. `CREATE_NO_WINDOW` and `DETACHED_PROCESS` cannot be combined.
pub fn windows_creation_flags(breakaway: bool) -> u32 {
    let flags = CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP;
    if breakaway { flags | CREATE_BREAKAWAY_FROM_JOB } else { flags }
}

/// Makes the child its own session leader (Unix) or detached process group (Windows). `breakaway` is for Windows only.
pub fn detach(command: &mut Command, breakaway: bool) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let _ = breakaway;
        // SAFETY: `setsid` is async-signal-safe and touches no memory of the forked child.
        unsafe {
            command.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(windows_creation_flags(breakaway));
    }
    #[cfg(not(any(unix, windows)))]
    let _ = (command, breakaway);
}

/// What `running.json` remembers about a started game.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub pid: u32,
    pub program: PathBuf,
    /// Seconds since the Unix epoch.
    pub started: u64,
    pub log_file: PathBuf,
    pub command: String,
}

impl Record {
    pub fn path(game_dir: &Path) -> PathBuf {
        game_dir.join(RUNNING_FILE)
    }

    pub fn save(&self, game_dir: &Path) {
        let _ = json_file::save(&Self::path(game_dir), self);
    }

    pub fn load(game_dir: &Path) -> Option<Self> {
        serde_json::from_str(&fs::read_to_string(Self::path(game_dir)).ok()?).ok()
    }

    pub fn remove(game_dir: &Path) {
        let _ = fs::remove_file(Self::path(game_dir));
    }
}

/// Is process `pid` alive and is it `program`? A reused pid of another program is not.
pub fn is_alive(pid: u32, program: &Path) -> bool {
    let name = program.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    #[cfg(target_os = "linux")]
    {
        let _ = name;
        let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) else { return false };
        // The state letter follows the `(name)`; a zombie is over.
        if stat.rsplit(')').next().is_some_and(|rest| rest.trim_start().starts_with('Z')) {
            return false;
        }
        let want = program.to_string_lossy();
        let exe = fs::read_link(format!("/proc/{pid}/exe"))
            .map(|e| e.to_string_lossy().trim_end_matches(" (deleted)").to_owned());
        // A script runs as its interpreter, so the program is also looked for in the first two words of the command line.
        let cmdline = fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
        let words: Vec<&[u8]> = cmdline.split(|&b| b == 0).take(2).collect();
        exe.is_ok_and(|e| e == want) || words.iter().any(|w| *w == want.as_bytes())
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        let out = Command::new("ps").args(["-p", &pid.to_string(), "-o", "comm="]).output();
        out.is_ok_and(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).contains(&name))
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let out = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .creation_flags(0x0800_0000)
            .output();
        out.is_ok_and(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains(&name.to_lowercase()))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (pid, name);
        false
    }
}

/// Ends process `pid` by force; used only for a game this launcher run did not start itself.
pub fn kill(pid: u32) {
    #[cfg(unix)]
    if let Ok(pid) = libc::pid_t::try_from(pid) {
        // SAFETY: `kill` only sends a signal; the pid was checked to be the game just before.
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill").args(["/F", "/PID", &pid.to_string()]).creation_flags(0x0800_0000).status();
    }
    #[cfg(not(any(unix, windows)))]
    let _ = pid;
}
