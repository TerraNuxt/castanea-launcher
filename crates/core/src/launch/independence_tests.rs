//! The game does not depend on the launcher: closing the launcher side leaves it running, and a new start finds it again.

use super::detach::{self, CREATE_BREAKAWAY_FROM_JOB, CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS};
use super::*;
use crate::settings::GameOptions;

#[test]
fn windows_flags_detach_the_game_and_skip_create_no_window() {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let with = detach::windows_creation_flags(true);
    let without = detach::windows_creation_flags(false);
    assert_eq!(with, DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB);
    assert_eq!(without, DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    assert_eq!(with & CREATE_NO_WINDOW, 0, "CREATE_NO_WINDOW is ignored with DETACHED_PROCESS");
}

#[test]
fn a_plain_start_asks_for_the_start_screen() {
    let build = Build { binary: "/g/castanea".into(), ..Build::default() };
    let plan_for = |name: &str, extra: &str| {
        let options = GameOptions { player_name: name.into(), extra_args: extra.into(), ..GameOptions::default() };
        plan(&options, &build, "/d".into()).unwrap().args
    };
    assert_eq!(plan_for("", ""), ["--menu"], "no arguments at all would also open it, but --name must not hide it");
    assert_eq!(plan_for("Anna", ""), ["--menu", "--name", "Anna"]);
    assert_eq!(plan_for("Anna", "--gamemode creative"), ["--menu", "--name", "Anna", "--gamemode", "creative"]);
    for direct in ["--world lake", "--join 10.0.0.2", "--host", "--menu", "12345"] {
        assert!(!plan_for("Anna", direct).starts_with(&["--menu".to_owned(), "--name".to_owned()]), "{direct}");
    }
    assert_eq!(plan_for("", "--world lake"), ["--world", "lake"], "an explicit world is started directly");
}

#[cfg(target_os = "linux")]
mod unix {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::time::{Duration, Instant};

    use super::*;

    fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
        let end = Instant::now() + Duration::from_secs(10);
        while !done() {
            assert!(Instant::now() < end, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// A game that writes to stdout and stderr after the launcher side is gone, then runs until killed.
    fn long_game(dir: &Path) -> Build {
        let root = dir.join("build");
        fs::create_dir_all(root.join("assets")).unwrap();
        let binary = root.join("castanea");
        let script = "#!/bin/sh\necho up\ntouch up.marker\nwhile [ ! -e go.marker ]; do sleep 0.1; done\n\
            echo late-out; echo late-err >&2; touch alive.marker\nwhile [ ! -e quit.marker ]; do sleep 0.1; done\n";
        fs::write(&binary, script).unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
        Build { version: "0.1.0".into(), root: root.clone(), binary, ..Build::default() }
    }

    fn alive(pid: u32) -> bool {
        // A child this test dropped without waiting stays a zombie until the test ends; that counts as gone.
        fs::read_to_string(format!("/proc/{pid}/stat"))
            .is_ok_and(|stat| !stat.rsplit(')').next().unwrap_or("").trim_start().starts_with('Z'))
    }

    fn finish_game(game_dir: &Path, pid: u32) {
        fs::write(game_dir.join("quit.marker"), "").unwrap();
        wait_for("the game to end", || !alive(pid));
    }

    #[test]
    fn closing_the_launcher_side_leaves_the_game_running_and_writing() {
        let dir = tempfile::tempdir().unwrap();
        let build = long_game(dir.path());
        let paths = crate::paths::Paths::portable(dir.path());
        let game_dir = paths.game();
        let (_plan, running) = launch(&paths, &build, &GameOptions::default(), 100).unwrap();
        let pid = running.pid();
        wait_for("the game to start", || game_dir.join("up.marker").exists());
        #[cfg(target_os = "linux")]
        {
            let stat = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
            let fields: Vec<&str> = stat.rsplit(')').next().unwrap().split_whitespace().collect();
            assert_eq!(fields[3], pid.to_string(), "the game leads its own session");
            assert_ne!(fields[3], unsafe { libc::getsid(0) }.to_string());
        }
        // Closing the launcher: the readers, the buffers and the handle are all gone.
        drop(running);
        // Writing to stdout and stderr must not kill it (a closed pipe would).
        fs::write(game_dir.join("go.marker"), "").unwrap();
        wait_for("a write after the close", || game_dir.join("alive.marker").exists());
        assert!(alive(pid), "the game is still running");
        let log = fs::read_to_string(game_dir.join("logs/latest.log")).unwrap();
        assert!(log.contains("late-out") && log.contains("late-err"), "{log}");
        finish_game(&game_dir, pid);
    }

    #[test]
    fn the_next_launcher_start_finds_the_running_game_and_can_stop_it() {
        let dir = tempfile::tempdir().unwrap();
        let build = long_game(dir.path());
        let paths = crate::paths::Paths::portable(dir.path());
        let game_dir = paths.game();
        let (_plan, running) = launch(&paths, &build, &GameOptions::default(), 100).unwrap();
        let pid = running.pid();
        wait_for("the game to start", || game_dir.join("up.marker").exists());
        drop(running);

        let (mut adopted, command) = Running::adopt(&game_dir, 100).expect("the game is still running");
        assert_eq!(adopted.pid(), pid);
        assert!(command.contains("castanea"), "{command}");
        wait_for("its output", || adopted.log.lock().unwrap().lines.iter().any(|l| l == "up"));
        assert!(adopted.poll().unwrap().is_none());
        let outcome = adopted.stop().unwrap();
        assert!(!alive(pid), "stop ends an adopted game");
        assert_eq!(outcome.code, None);
        assert!(Running::adopt(&game_dir, 100).is_none(), "nothing left to find");
        assert!(!detach::Record::path(&game_dir).exists());
    }

    #[test]
    fn a_finished_game_or_a_reused_pid_is_not_adopted() {
        let dir = tempfile::tempdir().unwrap();
        let game_dir = dir.path().join("game");
        fs::create_dir_all(&game_dir).unwrap();
        let record = detach::Record {
            pid: std::process::id(),
            program: "/no/such/castanea".into(),
            started: 1,
            log_file: game_dir.join("latest.log"),
            command: "x".into(),
        };
        record.save(&game_dir);
        assert!(Running::adopt(&game_dir, 10).is_none(), "the pid belongs to another program");
        assert!(!detach::Record::path(&game_dir).exists(), "the stale file is removed");
        assert!(Running::adopt(&game_dir, 10).is_none(), "no file, no game");
    }
}
