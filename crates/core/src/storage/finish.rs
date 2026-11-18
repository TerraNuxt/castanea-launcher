//! Windows: a running exe cannot delete itself, so the uninstall hands over to a copy of the launcher in the temp
//! folder (`--finish-uninstall <exe> <pid>`). It waits for the pid to end, removes what the plan lists, and removes
//! itself through a short-lived `cmd`, or at the next reboot when that cannot start.

use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use super::remove::{Report, Scope};
use super::uninstall::{finish_steps, plan, shared_folders, shortcut_folders};
use crate::paths::Paths;
use crate::{Error, Result};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const DETACHED_PROCESS: u32 = 0x0000_0008;
const MOVEFILE_DELAY_UNTIL_REBOOT: u32 = 4;
/// How long the helper waits for the launcher to exit.
const WAIT: Duration = Duration::from_secs(60);

unsafe extern "system" {
    fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
}

/// Copies this executable to the temp folder and starts the copy detached; the caller exits right after.
pub fn start_finisher(exe: &Path) -> Result<()> {
    let me = std::env::current_exe().map_err(|e| Error::io(exe, e))?;
    let copy: PathBuf = std::env::temp_dir().join(format!("castanea-uninstall-{}.exe", std::process::id()));
    std::fs::copy(&me, &copy).map_err(|e| Error::io(&copy, e))?;
    Command::new(&copy)
        .arg("--finish-uninstall")
        .arg(exe)
        .arg(std::process::id().to_string())
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .spawn()
        .map(|_| ())
        .map_err(|e| Error::io(&copy, e))
}

/// The helper's work: returns what could not be removed (the helper has no window, so it only logs to stderr).
pub fn finish_uninstall(exe: &Path, pid: u32) -> Report {
    let start = Instant::now();
    while alive(pid) && start.elapsed() < WAIT {
        std::thread::sleep(Duration::from_millis(300));
    }
    let Some(paths) = Paths::for_user() else { return Report::default() };
    let scope = Scope::new(&paths, &[]);
    let list = plan(&paths, exe, &shared_folders(), &shortcut_folders());
    let report = finish_steps(&list, &paths, &scope, 40, Duration::from_millis(500));
    remove_self();
    report
}

/// Whether process `pid` still runs, asked of `tasklist` (no extra Windows API needed).
fn alive(pid: u32) -> bool {
    let out = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    out.is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains(&format!("\"{pid}\"")))
}

/// Deletes this helper: a `cmd` that waits a moment and deletes it, else a delete at the next reboot.
fn remove_self() {
    let Ok(me) = std::env::current_exe() else { return };
    let line = format!("ping -n 3 127.0.0.1 >nul & del /f /q \"{}\"", me.display());
    let spawned =
        Command::new("cmd").args(["/c", &line]).creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS).spawn().is_ok();
    if !spawned {
        let wide: Vec<u16> = me.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: `wide` is a NUL-terminated UTF-16 path that outlives the call; a null target means delete.
        unsafe { MoveFileExW(wide.as_ptr(), std::ptr::null(), MOVEFILE_DELAY_UNTIL_REBOOT) };
    }
}
