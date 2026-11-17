use std::fs;
use std::path::{Path, PathBuf};

use super::plan::split_args;
use super::prepare::{self, link_assets, set_game_setting};
use super::*;
use crate::install::Origin;
use crate::profile::{Graphics, StartIn};

fn build(root: &Path) -> Build {
    Build {
        id: "v1".into(),
        version: "0.1.0".into(),
        root: root.to_path_buf(),
        binary: root.join("castanea"),
        origin: Origin::Local,
        sha256: None,
        managed: false,
    }
}

fn args(plan: &Plan) -> Vec<&str> {
    plan.args.iter().map(String::as_str).collect()
}

#[test]
fn the_menu_profile_has_no_arguments_and_the_game_dir_as_cwd() {
    let plan = plan(&Profile::default(), &build(Path::new("/games/v1")), PathBuf::from("/inst/default")).unwrap();
    assert_eq!(plan.program, Path::new("/games/v1/castanea"));
    assert!(plan.args.is_empty());
    assert!(plan.env.is_empty());
    assert_eq!(plan.cwd, Path::new("/inst/default"));
}

#[test]
fn a_world_profile_gives_seed_world_host_name_and_extra_args() {
    let profile = Profile {
        start: StartIn::World { name: "lake".into(), seed: "0x5eed".into(), host_port: Some(47900) },
        player_name: "Anna".into(),
        extra_args: r#"--gamemode creative --assist "hunger_rate=0.5" --biome 'deep ocean'"#.into(),
        graphics: Graphics::Wgpu,
        window: "1280x720".into(),
        env: vec![("RUST_LOG".into(), "info".into()), ("CASTANEA_BACKEND".into(), "vulkan".into())],
        ..Profile::default()
    };
    let plan = plan(&profile, &build(Path::new("/g")), PathBuf::from("/i")).unwrap();
    assert_eq!(
        args(&plan),
        [
            "0x5eed", "--world", "lake", "--host", "47900", "--name", "Anna", "--gamemode", "creative", "--assist",
            "hunger_rate=0.5", "--biome", "deep ocean"
        ]
    );
    assert_eq!(
        plan.env,
        [
            ("CASTANEA_WINDOW".to_owned(), "1280x720".to_owned()),
            ("RUST_LOG".to_owned(), "info".to_owned()),
            ("CASTANEA_BACKEND".to_owned(), "vulkan".to_owned())
        ],
        "a profile variable overrides the graphics choice"
    );
    assert!(plan.display().contains("\"deep ocean\""));
}

#[test]
fn a_join_profile_and_bad_values() {
    let join = Profile { start: StartIn::Join { address: "10.0.0.5:47800".into() }, ..Profile::default() };
    assert_eq!(args(&plan(&join, &build(Path::new("/g")), "/i".into()).unwrap()), ["--join", "10.0.0.5:47800"]);
    let bad = [
        Profile { start: StartIn::Join { address: "--world".into() }, ..Profile::default() },
        Profile { start: StartIn::Join { address: " ".into() }, ..Profile::default() },
        Profile { start: StartIn::World { name: "--x".into(), seed: String::new(), host_port: None }, ..Profile::default() },
        Profile { start: StartIn::World { name: "a".into(), seed: "-1 2".into(), host_port: None }, ..Profile::default() },
        Profile { window: "big".into(), ..Profile::default() },
        Profile { extra_args: "\"open".into(), ..Profile::default() },
        Profile { env: vec![("A=B".into(), "c".into())], ..Profile::default() },
    ];
    for profile in bad {
        assert!(plan(&profile, &build(Path::new("/g")), "/i".into()).is_err(), "{profile:?}");
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
fn the_newest_present_build_is_picked_unless_the_profile_names_one() {
    let dir = tempfile::tempdir().unwrap();
    let mut registry = Registry::default();
    for (id, version) in [("a", "0.9.0"), ("b", "0.10.0"), ("c", "local"), ("gone", "9.0.0")] {
        let root = dir.path().join(id);
        if id != "gone" {
            fs::create_dir_all(root.join("assets")).unwrap();
            fs::write(root.join("castanea"), "x").unwrap();
        }
        registry.put(Build { id: id.into(), version: version.into(), ..build(&root) });
    }
    assert_eq!(pick_build(&registry, &Profile::default()).unwrap().id, "b");
    let pinned = Profile { build: Some("c".into()), ..Profile::default() };
    assert_eq!(pick_build(&registry, &pinned).unwrap().id, "c");
    let missing = Profile { build: Some("gone".into()), ..Profile::default() };
    assert!(pick_build(&registry, &missing).is_err());
    assert!(pick_build(&Registry::default(), &Profile::default()).is_err());
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
    let settings: serde_json::Value = serde_json::from_str(&fs::read_to_string(game.join("settings.json")).unwrap()).unwrap();
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
    let mut registry = Registry::default();
    registry.put(build);
    let paths = crate::paths::Paths::portable(dir.path());
    let profile = Profile { extra_args: "--world x".into(), graphics: Graphics::Vulkan, ..Profile::default() };
    let (_plan, mut running) = launch(&paths, &registry, &profile, 100).unwrap();
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
    let game_dir = profile.game_dir(&paths).canonicalize().unwrap();
    assert!(lines.iter().any(|l| Path::new(l).canonicalize().ok().as_deref() == Some(game_dir.as_path())), "{lines:?}");
    let log = fs::read_to_string(&running.log_file).unwrap();
    assert!(log.contains("oops") && log.starts_with("# "));
}

#[cfg(unix)]
#[test]
fn a_crash_report_or_a_bad_exit_counts_as_a_crash_and_logs_rotate() {
    let dir = tempfile::tempdir().unwrap();
    let build = fake_game(dir.path(), "mkdir -p crash; echo boom > crash/crash-1.txt; exit 101");
    let mut registry = Registry::default();
    registry.put(build);
    let paths = crate::paths::Paths::portable(dir.path());
    let profile = Profile::default();
    for _ in 0..2 {
        let (_plan, mut running) = launch(&paths, &registry, &profile, 100).unwrap();
        let outcome = loop {
            if let Some(outcome) = running.poll().unwrap() {
                break outcome;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        assert!(outcome.crashed);
        assert_eq!(outcome.code, Some(101));
    }
    let logs = fs::read_dir(profile.game_dir(&paths).join("logs")).unwrap().count();
    assert_eq!(logs, 2, "latest.log and one older log");
}

#[cfg(unix)]
#[test]
fn stopping_a_running_game_ends_it() {
    let dir = tempfile::tempdir().unwrap();
    let build = fake_game(dir.path(), "echo started; exec sleep 30");
    let mut registry = Registry::default();
    registry.put(build);
    let paths = crate::paths::Paths::portable(dir.path());
    let (_plan, mut running) = launch(&paths, &registry, &Profile::default(), 100).unwrap();
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
