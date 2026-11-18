//! The backend choice against a fake game: a shell script that answers `--probe-gpu` and records what it is asked.
#![cfg_attr(not(unix), allow(dead_code, unused_imports))]

use std::time::Duration;

use super::probe::{self, Probe, Unsupported};
use super::*;
use crate::settings::Graphics;

const OK: &str = r#"{"vulkan":{"ok":true,"adapter":"Fake GPU","driver":"1.2"},"wgpu":{"ok":true,"adapter":"Fake GPU","driver":"dx12"}}"#;
const VK_FAILS: &str = r#"{"vulkan":{"ok":false,"adapter":"","driver":"","error":"no device"},"wgpu":{"ok":true,"adapter":"Fake GPU","driver":"dx12"}}"#;
const BOTH_FAIL: &str = r#"{"vulkan":{"ok":false,"error":"no device"},"wgpu":{"ok":false,"error":"no adapter"}}"#;
const LONG: Duration = Duration::from_secs(20);

/// A shell script has the flag name in a comment, so the binary scan finds it; `probe` is what the probe prints.
#[cfg(unix)]
fn fake(dir: &std::path::Path, probe_body: &str, game_body: &str) -> Build {
    use std::os::unix::fs::PermissionsExt;
    let root = dir.join("build");
    std::fs::create_dir_all(root.join("assets")).unwrap();
    let binary = root.join("castanea");
    let script = format!(
        "#!/bin/sh\n# supports --probe-gpu\nif [ \"$1\" = --probe-gpu ]; then\necho probe >> \"$PWD/probes\"\n{probe_body}\nfi\n{game_body}\n"
    );
    std::fs::write(&binary, script).unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    Build { version: "1.0.0".into(), root, binary, ..Build::default() }
}

#[cfg(unix)]
fn says(json: &str) -> String {
    format!("echo '{json}'; exit 0")
}

fn probes(paths: &Paths) -> usize {
    std::fs::read_to_string(paths.game().join("probes")).map_or(0, |t| t.lines().count())
}

fn auto() -> GameOptions {
    GameOptions::default()
}

#[cfg(unix)]
fn setup(probe_body: &str, game_body: &str) -> (tempfile::TempDir, Paths, Build) {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    let build = fake(dir.path(), probe_body, game_body);
    (dir, paths, build)
}

#[cfg(unix)]
fn run_to_end(paths: &Paths, build: &Build, choice: &Choice) -> crate::launch::Outcome {
    let options = GameOptions { graphics: choice.graphics, ..GameOptions::default() };
    let (_plan, mut running) = crate::launch::launch(paths, build, &options, 100).unwrap();
    loop {
        if let Some(outcome) = running.poll().unwrap() {
            return outcome;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(unix)]
#[test]
fn a_working_vulkan_is_chosen_and_reported() {
    let (_dir, paths, build) = setup(&says(OK), "");
    let choice = choose(&paths, &build, &auto(), "gpu1", LONG).unwrap();
    assert_eq!((choice.graphics, choice.reason), (Graphics::Vulkan, Reason::VulkanWorks));
    assert_eq!((choice.adapter.as_str(), choice.driver.as_str()), ("Fake GPU", "1.2"));
    let last = BackendState::load(&paths).last.unwrap();
    assert_eq!(
        (last.backend.as_str(), last.reason, last.adapter.as_str()),
        ("vulkan", Reason::VulkanWorks, "Fake GPU")
    );
}

#[cfg(unix)]
#[test]
fn a_failing_vulkan_gives_wgpu_and_two_failures_give_an_error_saying_why() {
    let (_dir, paths, build) = setup(&says(VK_FAILS), "");
    let choice = choose(&paths, &build, &auto(), "gpu1", LONG).unwrap();
    assert_eq!((choice.graphics, choice.reason), (Graphics::Wgpu, Reason::VulkanFailed));

    let (_dir, paths, build) = setup(&says(BOTH_FAIL), "");
    let error = choose(&paths, &build, &auto(), "gpu1", LONG).unwrap_err().to_string();
    assert!(error.contains("no device") && error.contains("no adapter"), "{error}");
}

#[cfg(unix)]
#[test]
fn a_game_without_the_probe_starts_with_auto() {
    // No flag in the binary: the game is not even started for a probe.
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    let root = dir.path().join("build");
    std::fs::create_dir_all(root.join("assets")).unwrap();
    let binary = root.join("castanea");
    std::fs::write(&binary, "#!/bin/sh\necho probed >> \"$PWD/probes\"\n").unwrap();
    let build = Build { version: "1.0.0".into(), root, binary, ..Build::default() };
    let choice = choose(&paths, &build, &auto(), "gpu1", LONG).unwrap();
    assert_eq!((choice.graphics, choice.reason), (Graphics::Auto, Reason::NoProbe));
    assert_eq!(probes(&paths), 0, "never run");
    assert_eq!(choice.graphics.env_value(), None, "no CASTANEA_BACKEND is passed");

    // The flag is known to the binary but refused with exit code 2.
    let (_dir, paths, build) = setup("exit 2", "");
    let choice = choose(&paths, &build, &auto(), "gpu1", LONG).unwrap();
    assert_eq!((choice.graphics, choice.reason), (Graphics::Auto, Reason::NoProbe));
}

#[cfg(unix)]
#[test]
fn a_probe_that_hangs_is_killed_and_counts_as_no_probe() {
    let (_dir, paths, build) = setup("sleep 30", "");
    let started = std::time::Instant::now();
    let choice = choose(&paths, &build, &auto(), "gpu1", Duration::from_millis(300)).unwrap();
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!((choice.graphics, choice.reason), (Graphics::Auto, Reason::NoProbe));
    let state = BackendState::load(&paths);
    assert!(matches!(state.probes[0].result, Stored::Unsupported { why: Unsupported::Timeout }));
    // A timeout is trusted for a day only.
    let at = state.probes[0].at;
    assert!(state.fresh("1.0.0", "gpu1", at + 3600).is_some());
    assert!(state.fresh("1.0.0", "gpu1", at + 2 * 24 * 3600).is_none());
}

#[cfg(unix)]
#[test]
fn a_probe_killed_by_the_driver_counts_as_vulkan_failed() {
    let (_dir, paths, build) = setup("kill -SEGV $$", "");
    let choice = choose(&paths, &build, &auto(), "gpu1", LONG).unwrap();
    assert_eq!((choice.graphics, choice.reason), (Graphics::Wgpu, Reason::VulkanFailed));
}

#[cfg(unix)]
#[test]
fn the_result_is_cached_per_version_and_gpu_and_expires() {
    let (_dir, paths, mut build) = setup(&says(OK), "");
    let at = |paths: &Paths, build: &Build, gpu: &str, now: u64| {
        choose_at(paths, build, &auto(), gpu, LONG, now).unwrap();
    };
    at(&paths, &build, "gpu1", 1000);
    at(&paths, &build, "gpu1", 2000);
    assert_eq!(probes(&paths), 1, "the second start uses the cache");
    at(&paths, &build, "gpu2", 2000);
    assert_eq!(probes(&paths), 2, "another GPU or driver probes again");
    build.version = "1.1.0".into();
    at(&paths, &build, "gpu1", 2000);
    assert_eq!(probes(&paths), 3, "another game version probes again");
    at(&paths, &build, "gpu1", 2000 + 31 * 24 * 3600);
    assert_eq!(probes(&paths), 4, "an old result is checked again");
    at(&paths, &build, "", 5000);
    at(&paths, &build, "", 5000 + 2 * 24 * 3600);
    assert_eq!(probes(&paths), 6, "an unknown GPU is trusted for a day only");
}

#[cfg(unix)]
#[test]
fn a_chosen_setting_or_environment_runs_no_probe() {
    let (_dir, paths, build) = setup(&says(OK), "");
    let wgpu = GameOptions { graphics: Graphics::Wgpu, ..auto() };
    let choice = choose(&paths, &build, &wgpu, "gpu1", LONG).unwrap();
    assert_eq!((choice.graphics, choice.reason), (Graphics::Wgpu, Reason::Setting));
    let env = GameOptions { env: vec![("CASTANEA_BACKEND".into(), "vulkan".into())], ..auto() };
    let choice = choose(&paths, &build, &env, "gpu1", LONG).unwrap();
    assert_eq!(choice.reason, Reason::Environment);
    assert_eq!(probes(&paths), 0);
}

#[cfg(unix)]
#[test]
fn a_vulkan_start_that_dies_at_once_is_remembered_and_the_next_start_uses_wgpu() {
    // The game prints its backend and dies; as a probe it says Vulkan works.
    let (_dir, paths, build) = setup(&says(OK), "echo \"backend=$CASTANEA_BACKEND\"; exit 139");
    let choice = choose(&paths, &build, &auto(), "gpu1", LONG).unwrap();
    assert_eq!(choice.graphics, Graphics::Vulkan);
    let outcome = run_to_end(&paths, &build, &choice);
    assert!(outcome.crashed && outcome.seconds < EARLY_SECONDS);
    assert_eq!(record_outcome(&paths, &choice, &outcome), Verdict::VulkanDied);

    let next = choose(&paths, &build, &auto(), "gpu1", LONG).unwrap();
    assert_eq!((next.graphics, next.reason), (Graphics::Wgpu, Reason::VulkanCrashed));
    assert_eq!(probes(&paths), 1, "no new probe for the decision");
    let outcome = run_to_end(&paths, &build, &next);
    assert_eq!(record_outcome(&paths, &next, &outcome), Verdict::Nothing, "wgpu dying is not blamed on Vulkan");

    let other_gpu = choose(&paths, &build, &auto(), "gpu2", LONG).unwrap();
    assert_eq!(other_gpu.graphics, Graphics::Vulkan, "another GPU gets its own chance");
    let mut newer = build.clone();
    newer.version = "1.0.1".into();
    assert_eq!(choose(&paths, &newer, &auto(), "gpu1", LONG).unwrap().graphics, Graphics::Vulkan);

    forget(&paths);
    assert_eq!(choose(&paths, &build, &auto(), "gpu1", LONG).unwrap().graphics, Graphics::Vulkan, "reset by Settings");
}

#[test]
fn only_an_early_crash_of_a_chosen_vulkan_start_is_remembered() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    let choice = |graphics, reason| Choice {
        graphics,
        reason,
        adapter: String::new(),
        driver: String::new(),
        version: "1.0.0".into(),
        gpu: "g".into(),
    };
    let outcome = |code: i32, seconds: u64| crate::launch::Outcome {
        code: Some(code),
        crashed: code != 0,
        crash_reports: Vec::new(),
        seconds,
    };
    let vulkan = choice(Graphics::Vulkan, Reason::VulkanWorks);
    assert_eq!(record_outcome(&paths, &vulkan, &outcome(1, EARLY_SECONDS)), Verdict::Nothing, "a late crash");
    assert_eq!(record_outcome(&paths, &vulkan, &outcome(0, 1)), Verdict::Nothing, "a clean exit");
    let forced = choice(Graphics::Vulkan, Reason::Setting);
    assert_eq!(record_outcome(&paths, &forced, &outcome(1, 1)), Verdict::Nothing, "the user's own choice");
    let auto_game = choice(Graphics::Auto, Reason::NoProbe);
    assert_eq!(record_outcome(&paths, &auto_game, &outcome(1, 1)), Verdict::Nothing, "backend unknown");
    assert!(!BackendState::load(&paths).vulkan_died("1.0.0", "g"));
    assert_eq!(record_outcome(&paths, &vulkan, &outcome(1, 2)), Verdict::VulkanDied);
    assert!(BackendState::load(&paths).vulkan_died("1.0.0", "g"));
}

#[test]
fn the_probe_line_is_read_after_noise_and_bad_output_is_told_from_a_crash() {
    let noisy = format!("warning: layers\n{OK}\n");
    assert!(matches!(probe::interpret(Some(0), &noisy), Probe::Report(r) if r.vulkan.ok));
    let no_wgpu = r#"{"vulkan":{"ok":true}}"#;
    assert!(matches!(probe::interpret(Some(0), no_wgpu), Probe::Report(r) if r.wgpu.is_none()));
    assert_eq!(probe::interpret(Some(2), "unknown option"), Probe::Unsupported(Unsupported::Refused(2)));
    assert_eq!(probe::interpret(Some(0), "{not json"), Probe::Unsupported(Unsupported::Refused(0)));
    assert_eq!(probe::interpret(None, ""), Probe::Crashed(None));
    assert_eq!(probe::interpret(Some(-1073741819), ""), Probe::Crashed(Some(-1073741819)));
}
