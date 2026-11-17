use std::fs;
use std::path::Path;

use super::*;
use crate::install::local::BINARY;
use crate::settings::Graphics;

fn old_build(versions: &Path, tag: &str, version: &str) {
    let dir = versions.join(tag);
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::write(dir.join(BINARY), "bin").unwrap();
    let install = format!(
        r#"{{"id":"{tag}","version":"{version}","root":"x","binary":"x/{BINARY}",
            "origin":{{"kind":"github","repo":"TerraNuxt/castanea","tag":"{tag}","asset":"castanea-{version}.zip"}},
            "sha256":"{}","managed":true}}"#,
        "a".repeat(64)
    );
    fs::write(dir.join("install.json"), install).unwrap();
}

#[test]
fn an_old_home_keeps_the_last_profile_and_the_newest_build() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    fs::create_dir_all(&paths.config).unwrap();
    fs::write(
        paths.config.join("profiles.json"),
        r#"{"profiles":[
            {"id":"default","name":"Default"},
            {"id":"creative","name":"Creative","extra_args":"--gamemode creative","graphics":"wgpu",
             "env":[["RUST_LOG","info"]],"start":{"kind":"join","address":"a:1"},"last_played":5}]}"#,
    )
    .unwrap();
    fs::write(paths.settings_file(), r#"{"last_profile":"creative","offline":true}"#).unwrap();
    fs::write(paths.config.join("installs.json"), r#"{"builds":[]}"#).unwrap();
    let instances = paths.data.join("instances");
    fs::create_dir_all(instances.join("default/saves/home")).unwrap();
    fs::create_dir_all(instances.join("creative/saves/lake")).unwrap();
    let versions = paths.data.join("versions");
    old_build(&versions, "v0.1.0", "0.1.0");
    old_build(&versions, "v0.10.0", "0.10.0");
    old_build(&versions, "v0.9.0", "0.9.0");
    #[cfg(unix)]
    std::os::unix::fs::symlink(versions.join("v0.9.0/assets"), instances.join("creative/assets")).unwrap();

    let mut settings = Settings::load(&paths).unwrap();
    let report = run(&paths, &mut settings).unwrap().unwrap();
    assert_eq!(report.profile.as_deref(), Some("Creative"));
    assert_eq!(report.left, [instances.join("default")]);
    assert_eq!(settings.game.extra_args, "--gamemode creative");
    assert_eq!(settings.game.graphics, Graphics::Wgpu);
    assert_eq!(settings.game.env, [("RUST_LOG".to_owned(), "info".to_owned())]);
    assert!(settings.offline, "other settings stay");
    assert_eq!(Settings::load(&paths).unwrap(), settings, "saved");
    assert!(paths.game().join("saves/lake").is_dir());
    assert!(fs::symlink_metadata(paths.game().join("assets")).is_err(), "the old assets link is gone");
    assert!(instances.join("default/saves/home").is_dir(), "other game folders are never touched");

    let build = update::installed(&paths).unwrap();
    assert_eq!((build.version.as_str(), build.tag.as_str()), ("0.10.0", "v0.10.0"));
    assert_eq!(build.sha256, Some("a".repeat(64)));
    assert!(!versions.exists() && !paths.config.join("installs.json").exists());
    assert!(!paths.config.join("profiles.json").exists() && paths.config.join("profiles.json.old").is_file());
    assert_eq!(run(&paths, &mut settings).unwrap(), None, "once only");
}

#[test]
fn a_new_home_needs_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    let mut settings = Settings::default();
    assert_eq!(run(&paths, &mut settings).unwrap(), None);
    assert_eq!(settings, Settings::default());
    assert!(!paths.config.exists() && !paths.data.exists());
}
