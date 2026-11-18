use std::fs;
use std::path::{Path, PathBuf};

use super::plan::split_args;
use super::prepare::{self, link_assets};
use super::*;
use crate::settings::Graphics;

fn build(root: &Path) -> Build {
    Build { version: "0.1.0".into(), root: root.to_path_buf(), binary: root.join("castanea"), ..Build::default() }
}

fn args(plan: &Plan) -> Vec<&str> {
    plan.args.iter().map(String::as_str).collect()
}

#[test]
fn default_options_only_ask_for_the_start_screen_and_the_game_dir_as_cwd() {
    let plan = plan(&GameOptions::default(), &build(Path::new("/games/v1")), PathBuf::from("/data/game")).unwrap();
    assert_eq!(plan.program, Path::new("/games/v1/castanea"));
    assert_eq!(args(&plan), ["--menu"]);
    assert!(plan.env.is_empty());
    assert_eq!(plan.cwd, Path::new("/data/game"));
}

#[test]
fn options_give_name_extra_args_and_environment() {
    let options = GameOptions {
        player_name: "Anna".into(),
        extra_args: r#"--gamemode creative --assist "hunger_rate=0.5" --biome 'deep ocean'"#.into(),
        graphics: Graphics::Wgpu,
        window: "1280x720".into(),
        env: vec![("RUST_LOG".into(), "info".into()), ("CASTANEA_BACKEND".into(), "vulkan".into())],
    };
    let plan = plan(&options, &build(Path::new("/g")), PathBuf::from("/i")).unwrap();
    assert_eq!(
        args(&plan),
        ["--menu", "--name", "Anna", "--gamemode", "creative", "--assist", "hunger_rate=0.5", "--biome", "deep ocean"]
    );
    assert_eq!(
        plan.env,
        [
            ("CASTANEA_WINDOW".to_owned(), "1280x720".to_owned()),
            ("RUST_LOG".to_owned(), "info".to_owned()),
            ("CASTANEA_BACKEND".to_owned(), "vulkan".to_owned())
        ],
        "a variable overrides the graphics choice"
    );
    assert!(plan.display().contains("\"deep ocean\""));
}

#[test]
fn bad_values_are_refused() {
    let bad = [
        GameOptions { player_name: "--world".into(), ..GameOptions::default() },
        GameOptions { window: "big".into(), ..GameOptions::default() },
        GameOptions { extra_args: "\"open".into(), ..GameOptions::default() },
        GameOptions { env: vec![("A=B".into(), "c".into())], ..GameOptions::default() },
    ];
    for options in bad {
        assert!(plan(&options, &build(Path::new("/g")), "/i".into()).is_err(), "{options:?}");
    }
}

#[test]
fn extra_arguments_split_like_a_shell() {
    assert_eq!(split_args("  a  b ").unwrap(), ["a", "b"]);
    assert_eq!(split_args(r#"a"b c"d"#).unwrap(), ["ab cd"]);
    assert_eq!(split_args(r#"'it''s' "" x"#).unwrap(), ["its", "", "x"]);
    assert_eq!(split_args(r"C:\games\castanea a\ b").unwrap(), [r"C:\games\castanea", "a b"]);
    assert_eq!(split_args(r#""say \"hi\"""#).unwrap(), [r#"say "hi""#]);
    assert!(split_args("'open").is_err());
    assert!(split_args("").unwrap().is_empty());
}

#[test]
fn preparing_links_assets_and_leaves_the_game_settings_alone() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("build");
    fs::create_dir_all(root.join("assets/engine")).unwrap();
    let game = dir.path().join("inst");
    fs::create_dir_all(&game).unwrap();
    fs::write(game.join("settings.json"), r#"{"fov": 80, "backend_url": "https://old"}"#).unwrap();
    prepare::prepare(&game, &build(&root)).unwrap();
    assert!(game.join("assets/engine").is_dir());
    assert!(game.join("mods").is_dir());
    let settings = fs::read_to_string(game.join("settings.json")).unwrap();
    assert_eq!(settings, r#"{"fov": 80, "backend_url": "https://old"}"#, "byte for byte as the game left it");

    let other = dir.path().join("build2/assets");
    fs::create_dir_all(&other).unwrap();
    link_assets(&game.join("assets"), &other).unwrap();
    assert!(!game.join("assets/engine").exists(), "the link moved to the other build");
    assert!(root.join("assets/engine").is_dir(), "the old build is untouched");

    let real = dir.path().join("real");
    fs::create_dir_all(real.join("assets")).unwrap();
    assert!(link_assets(&real.join("assets"), &other).is_err(), "a real folder is never replaced");
}

#[cfg(unix)]
fn fake_game(dir: &Path, script: &str) -> Build {
    use std::os::unix::fs::PermissionsExt;
    let root = dir.join("build");
    fs::create_dir_all(root.join("assets")).unwrap();
    let binary = root.join("castanea");
    fs::write(&binary, format!("#!/bin/sh\n{script}\n")).unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    build(&root)
}

#[cfg(unix)]
#[test]
fn a_run_captures_output_and_a_clean_exit_is_no_crash() {
    let dir = tempfile::tempdir().unwrap();
    let build = fake_game(dir.path(), "echo \"args: $*\"; echo \"backend: $CASTANEA_BACKEND\"; echo oops >&2; pwd");
    let paths = crate::paths::Paths::portable(dir.path());
    let options = GameOptions { extra_args: "--world x".into(), graphics: Graphics::Vulkan, ..GameOptions::default() };
    let (_plan, mut running) = launch(&paths, &build, &options, 100).unwrap();
    let outcome = loop {
        if let Some(outcome) = running.poll().unwrap() {
            break outcome;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert_eq!(outcome.code, Some(0));
    assert!(!outcome.crashed);
    let lines: Vec<String> = running.log.lock().unwrap().lines.iter().cloned().collect();
    assert!(lines.contains(&"args: --world x".to_owned()), "{lines:?}");
    assert!(lines.contains(&"backend: vulkan".to_owned()));
    assert!(lines.contains(&"oops".to_owned()));
    let game_dir = paths.game().canonicalize().unwrap();
    assert!(lines.iter().any(|l| Path::new(l).canonicalize().ok().as_deref() == Some(game_dir.as_path())), "{lines:?}");
    let log = fs::read_to_string(&running.log_file).unwrap();
    assert!(log.contains("oops") && log.starts_with("# "));
}

#[cfg(unix)]
#[test]
fn a_crash_report_or_a_bad_exit_counts_as_a_crash_and_logs_rotate() {
    let dir = tempfile::tempdir().unwrap();
    let build = fake_game(dir.path(), "mkdir -p crash; echo boom > crash/crash-1.txt; exit 101");
    let paths = crate::paths::Paths::portable(dir.path());
    let options = GameOptions::default();
    for _ in 0..2 {
        let (_plan, mut running) = launch(&paths, &build, &options, 100).unwrap();
        let outcome = loop {
            if let Some(outcome) = running.poll().unwrap() {
                break outcome;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        assert!(outcome.crashed);
        assert_eq!(outcome.code, Some(101));
    }
    let logs = fs::read_dir(paths.game().join("logs")).unwrap().count();
    assert_eq!(logs, 2, "latest.log and one older log");
}

#[cfg(unix)]
#[test]
fn stopping_a_running_game_ends_it() {
    let dir = tempfile::tempdir().unwrap();
    let build = fake_game(dir.path(), "echo started; exec sleep 30");
    let paths = crate::paths::Paths::portable(dir.path());
    let (_plan, mut running) = launch(&paths, &build, &GameOptions::default(), 100).unwrap();
    assert!(running.poll().unwrap().is_none());
    let outcome = running.stop().unwrap();
    assert!(outcome.crashed, "killed is not a clean exit");
    assert!(outcome.seconds < 10);
}

#[test]
fn the_log_buffer_keeps_the_last_lines() {
    let mut log = run::LogBuffer { limit: 2, ..Default::default() };
    for n in 0..5 {
        log.push(n.to_string());
    }
    assert_eq!(log.lines, ["3", "4"]);
    assert_eq!(log.total, 5);
}

#[cfg(unix)]
#[test]
fn a_launch_never_touches_backend_url_in_the_settings_or_the_environment() {
    let dir = tempfile::tempdir().unwrap();
    let build = fake_game(dir.path(), "echo \"url: $CASTANEA_BACKEND_URL $BACKEND_URL\"");
    let paths = crate::paths::Paths::portable(dir.path());
    let before = r#"{"fov": 80, "backend_url": "https://relay.example.org"}"#;
    fs::create_dir_all(paths.game()).unwrap();
    fs::write(paths.game().join("settings.json"), before).unwrap();
    let options = GameOptions { graphics: Graphics::Wgpu, ..GameOptions::default() };
    let (plan, mut running) = launch(&paths, &build, &options, 100).unwrap();
    assert!(
        plan.env.iter().all(|(k, v)| !k.to_lowercase().contains("backend_url") && !v.contains("relay")),
        "{plan:?}"
    );
    assert!(plan.args.iter().all(|a| !a.contains("backend") && !a.contains("relay")), "{plan:?}");
    while running.poll().unwrap().is_none() {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(fs::read_to_string(paths.game().join("settings.json")).unwrap(), before);
    let fresh = tempfile::tempdir().unwrap();
    let paths = crate::paths::Paths::portable(fresh.path());
    let (_plan, mut running) = launch(&paths, &fake_game(fresh.path(), "true"), &options, 100).unwrap();
    while running.poll().unwrap().is_none() {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(!paths.game().join("settings.json").exists(), "no settings file is created either");
}

#[cfg(unix)]
#[test]
fn a_missing_shared_library_is_named_in_the_outcome() {
    let dir = tempfile::tempdir().unwrap();
    let script = "echo \"$0: error while loading shared libraries: libasound.so.2: cannot open shared object file\" >&2; exit 127";
    let build = fake_game(dir.path(), script);
    let paths = crate::paths::Paths::portable(dir.path());
    let (_plan, mut running) = launch(&paths, &build, &GameOptions::default(), 100).unwrap();
    let outcome = loop {
        if let Some(outcome) = running.poll().unwrap() {
            break outcome;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert!(outcome.crashed);
    assert_eq!(outcome.code, Some(127));
    assert_eq!(outcome.missing_library.as_deref(), Some("libasound.so.2"));
}

#[test]
fn the_loader_error_is_parsed_and_known_libraries_have_a_package() {
    let line = "/x/castanea: error while loading shared libraries: libfoo.so.1: cannot open shared object file";
    assert_eq!(missing_library(["started", line]).as_deref(), Some("libfoo.so.1"));
    assert_eq!(missing_library(["all fine", "error while loading"]), None);
    assert_eq!(library_package("libasound.so.2"), Some("alsa-lib / libasound2"));
    assert_eq!(library_package("libfoo.so.1"), None);
}
