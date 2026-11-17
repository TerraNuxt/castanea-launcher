use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::checks::{self, Problem};
use super::import::import;
use super::installed::{self, DISABLED_DIR};
use super::manifest::Manifest;
use crate::archive::tests::write_zip;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game-mods")
}

fn write_mod(dir: &Path, folder: &str, json: &str) {
    fs::create_dir_all(dir.join(folder)).unwrap();
    fs::write(dir.join(folder).join("mod.json"), json).unwrap();
}

#[test]
fn the_game_example_manifests_parse() {
    let mut seen = Vec::new();
    for entry in fs::read_dir(fixtures()).unwrap() {
        let path = entry.unwrap().path().join("mod.json");
        if !path.exists() {
            continue;
        }
        let manifest =
            Manifest::parse(&fs::read_to_string(&path).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(manifest.unknown_keys.is_empty(), "{}", path.display());
        seen.push(manifest);
    }
    assert!(seen.len() >= 10, "{} fixtures", seen.len());
    let kiln = seen.iter().find(|m| m.id == "vanilla_kiln").unwrap();
    assert_eq!(kiln.side, "server");
    assert_eq!(kiln.wasm.as_deref(), Some("kiln.wat"));
    let amber = seen.iter().find(|m| m.id == "amber").unwrap();
    assert!(!amber.has_code());
    assert_eq!(amber.author, "Lilly");
}

#[test]
fn listing_reads_folders_zips_and_disabled_and_skips_dot_names() {
    let dir = tempfile::tempdir().unwrap();
    let mods = dir.path();
    write_mod(mods, "amber", r#"{"id":"amber","name":"Amber","version":"0.1.0"}"#);
    write_mod(&mods.join(DISABLED_DIR), "frost", r#"{"id":"frost","name":"Frost","version":"1"}"#);
    write_zip(&mods.join("zipped.zip"), &[("mod.json", br#"{"id":"zipped","name":"Z","version":"2"}"#)]);
    write_mod(mods, "broken", "{");
    fs::create_dir_all(mods.join(".cache/x")).unwrap();
    fs::write(mods.join("notes.txt"), "x").unwrap();
    let list = installed::list(mods).unwrap();
    let ids: Vec<(&str, bool)> = list.iter().map(|m| (m.id(), m.enabled)).collect();
    assert_eq!(ids, [("amber", true), ("broken", true), ("frost", false), ("zipped", true)]);
    assert!(list[1].manifest.is_err());
}

#[test]
fn enable_and_disable_move_the_package_and_remove_deletes_it() {
    let dir = tempfile::tempdir().unwrap();
    let mods = dir.path();
    write_mod(mods, "amber", r#"{"id":"amber","name":"Amber","version":"0.1.0"}"#);
    let amber = installed::list(mods).unwrap().remove(0);
    let moved = installed::set_enabled(mods, &amber, false).unwrap();
    assert_eq!(moved, mods.join(DISABLED_DIR).join("amber"));
    let amber = installed::list(mods).unwrap().remove(0);
    assert!(!amber.enabled);
    installed::set_enabled(mods, &amber, true).unwrap();
    let amber = installed::list(mods).unwrap().remove(0);
    assert!(amber.enabled);
    installed::remove(&amber).unwrap();
    assert!(installed::list(mods).unwrap().is_empty());
}

#[test]
fn dependency_problems_are_found() {
    let dir = tempfile::tempdir().unwrap();
    let mods = dir.path();
    write_mod(mods, "a", r#"{"id":"a","name":"A","version":"1","depends":["b","gone"]}"#);
    write_mod(&mods.join(DISABLED_DIR), "b", r#"{"id":"b","name":"B","version":"1"}"#);
    write_mod(mods, "c", r#"{"id":"c","name":"C","version":"1","depends":["d"]}"#);
    write_mod(mods, "d", r#"{"id":"d","name":"D","version":"1","depends":["c"]}"#);
    write_mod(mods, "d2", r#"{"id":"d","name":"D again","version":"1"}"#);
    let list = installed::list(mods).unwrap();
    let ranges = HashMap::from([("a".to_owned(), ">=0.2".to_owned())]);
    let problems = checks::check(&list, Some("0.1.0"), &ranges);
    let has = |file: &str, problem: Problem| problems.iter().any(|(f, p)| f == file && *p == problem);
    assert!(has("a", Problem::DisabledDependency("b".into())), "{problems:?}");
    assert!(has("a", Problem::MissingDependency("gone".into())));
    assert!(has("a", Problem::Incompatible { range: ">=0.2".into(), game: "0.1.0".into() }));
    assert!(has("c", Problem::Cycle));
    assert!(has("d2", Problem::DuplicateId));
    assert!(!problems.iter().any(|(f, _)| f == "b"));
}

#[test]
fn dependencies_are_listed_deepest_first() {
    let dir = tempfile::tempdir().unwrap();
    let mods = dir.path();
    write_mod(mods, "a", r#"{"id":"a","name":"A","version":"1","depends":["b"]}"#);
    write_mod(mods, "b", r#"{"id":"b","name":"B","version":"1","depends":["c"]}"#);
    write_mod(mods, "c", r#"{"id":"c","name":"C","version":"1"}"#);
    let list = installed::list(mods).unwrap();
    let deps: Vec<&str> = checks::dependencies_of(&list, "a").iter().map(|m| m.id()).collect();
    assert_eq!(deps, ["c", "b"]);
}

#[test]
fn imports_land_disabled_and_refuse_bad_packages() {
    let dir = tempfile::tempdir().unwrap();
    let mods = dir.path().join("mods");
    let source = dir.path().join("src");
    write_mod(&source, "amber", r#"{"id":"amber","name":"Amber","version":"0.1.0"}"#);
    fs::create_dir_all(source.join("amber/assets/content/blocks")).unwrap();
    fs::write(source.join("amber/assets/content/blocks/amber.json"), "{}").unwrap();
    let imported = import(&mods, &source.join("amber"), false).unwrap();
    assert!(!imported.enabled);
    assert!(mods.join(DISABLED_DIR).join("amber/assets/content/blocks/amber.json").is_file());
    assert!(import(&mods, &source.join("amber"), false).is_err(), "same id twice");
    assert!(import(&mods, &source.join("amber"), true).is_ok(), "replace");

    let zip = dir.path().join("frost.zip");
    write_zip(&zip, &[("mod.json", br#"{"id":"frost","name":"Frost","version":"1"}"#), ("assets/a.json", b"{}")]);
    let frost = import(&mods, &zip, false).unwrap();
    assert_eq!(frost.path, mods.join(DISABLED_DIR).join("frost.zip"));
    let declared = installed::declared(&frost).unwrap();
    assert_eq!(declared.files, 2);
    assert_eq!(declared.kinds, [("json".to_owned(), 2)]);

    let nested = dir.path().join("nested.zip");
    write_zip(&nested, &[("frost/mod.json", br#"{"id":"x","name":"X","version":"1"}"#)]);
    assert!(import(&mods, &nested, false).unwrap_err().to_string().contains("top of the zip"));
    let slip = dir.path().join("slip.zip");
    write_zip(&slip, &[("mod.json", br#"{"id":"y","name":"Y","version":"1"}"#), ("../y.txt", b"x")]);
    assert!(import(&mods, &slip, false).is_err());
    let names: Vec<String> = fs::read_dir(mods.join(DISABLED_DIR))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names.len(), 2, "no temp files left: {names:?}");
}
