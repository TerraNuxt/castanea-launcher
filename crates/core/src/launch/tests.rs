use std::fs;
use std::path::{Path, PathBuf};

use super::plan::split_args;
use super::prepare::{self, link_assets, set_game_setting};
use super::*;
use crate::settings::Graphics;

fn build(root: &Path) -> Build {
    Build { version: "0.1.0".into(), root: root.to_path_buf(), binary: root.join("castanea"), ..Build::default() }
}

fn args(plan: &Plan) -> Vec<&str> {
    plan.args.iter().map(String::as_str).collect()
}

#[test]
fn default_options_have_no_arguments_and_the_game_dir_as_cwd() {
    let plan = plan(&GameOptions::default(), &build(Path::new("/games/v1")), PathBuf::from("/data/game")).unwrap();
    assert_eq!(plan.program, Path::new("/games/v1/castanea"));
    assert!(plan.args.is_empty());
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
        ..GameOptions::default()
    };
    let plan = plan(&options, &build(Path::new("/g")), PathBuf::from("/i")).unwrap();
    assert_eq!(
        args(&plan),
        ["--name", "Anna", "--gamemode", "creative", "--assist", "hunger_rate=0.5", "--biome", "deep ocean"]
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
fn preparing_links_assets_and_writes_backend_url_keeping_other_settings() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("build");
    fs::create_dir_all(root.join("assets/engine")).unwrap();
    let game = dir.path().join("inst");
    fs::create_dir_all(&game).unwrap();
    fs::write(game.join("settings.json"), r#"{"fov": 80, "backend_url": "https://old"}"#).unwrap();
    prepare::prepare(&game, &build(&root), "https://relay.example.org").unwrap();
    assert!(game.join("assets/engine").is_dir());
    assert!(game.join("mods").is_dir());
    let settings: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(game.join("settings.json")).unwrap()).unwrap();
    assert_eq!(settings["fov"], 80);
    assert_eq!(settings["backend_url"], "https://relay.example.org");

    let other = dir.path().join("build2/assets");
    fs::create_dir_all(&other).unwrap();
    link_assets(&game.join("assets"), &other).unwrap();
    assert!(!game.join("assets/engine").exists(), "the link moved to the other build");
    assert!(root.join("assets/engine").is_dir(), "the old build is untouched");

    let real = dir.path().join("real");
    fs::create_dir_all(real.join("assets")).unwrap();
    assert!(link_assets(&real.join("assets"), &other).is_err(), "a real folder is never replaced");
    fs::write(dir.path().join("bad.json"), "[1]").unwrap();
    assert!(set_game_setting(&dir.path().join("bad.json"), "k", "v").is_err());
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
