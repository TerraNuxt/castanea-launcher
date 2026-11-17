use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use super::client::{self, Freshness, ModIndex, StaticIndex};
use super::format::Index;
use super::install;
use crate::archive::tests::write_zip;
use crate::download::{UrlPolicy, sha256_hex};
use crate::mods::installed;
use crate::test_server::TestServer;
use crate::version::Compat;

fn example() -> Vec<u8> {
    fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/index.json")).unwrap()
}

fn strict() -> UrlPolicy {
    UrlPolicy::default()
}

#[test]
fn the_documented_example_parses() {
    let index = Index::parse(&example(), "example", strict()).unwrap();
    assert_eq!(index.mods.len(), 3);
    assert_eq!(index.news.len(), 1);
    let amber = index.get("amber").unwrap();
    assert_eq!(amber.versions[0].version, "0.2.0", "newest first");
    assert_eq!(amber.best_version(Some("0.1.5")).unwrap().version, "0.1.0");
    assert_eq!(amber.best_version(Some("0.3.0")).unwrap().version, "0.2.0");
    assert_eq!(amber.compat(Some("0.5.0")), Compat::No);
    assert_eq!(amber.compat(Some("local")), Compat::Unknown);
    assert_eq!(index.get("redmoon").unwrap().compat(Some("0.1.0")), Compat::Unknown, "no range given");
    assert!(index.get("wasm_hello").unwrap().has_code);
    assert_eq!(index.tags()[0], ("example".to_owned(), 3));
    let found: Vec<&str> = index.mods.iter().filter(|m| m.matches("MOON")).map(|m| m.id.as_str()).collect();
    assert_eq!(found, ["redmoon"]);
    assert!(index.mods.iter().all(|m| m.matches("")));
}

#[test]
fn bad_indexes_are_refused_with_a_reason() {
    let text = String::from_utf8(example()).unwrap();
    let cases = [
        (text.replace("\"format\": 1", "\"format\": 2"), "format 2"),
        (text.replace("\"id\": \"amber\"", "\"id\": \"Amber!\""), "not valid"),
        (text.replace("https://example.org/mods/amber-0.1.0.zip", "http://example.org/a.zip"), "https"),
        (text.replace("0000000000000000000000000000000000000000000000000000000000000001", "xyz"), "sha256"),
        (text.replace("\"id\": \"redmoon\"", "\"id\": \"amber\""), "twice"),
        ("{".to_owned(), "example"),
    ];
    for (bad, needle) in cases {
        let error = Index::parse(bad.as_bytes(), "example", strict()).unwrap_err().to_string();
        assert!(error.contains(needle), "{needle}: {error}");
    }
}

#[test]
fn a_remote_index_is_cached_revalidated_and_used_offline() {
    let server = TestServer::start();
    server.put("/index.json", example());
    let dir = tempfile::tempdir().unwrap();
    let source = StaticIndex::new(&server.url("/index.json"), dir.path());
    let client = TestServer::client();
    assert!(source.load(&client, true).is_err(), "nothing cached yet");
    let first = source.load(&client, false).unwrap();
    assert_eq!(first.freshness, Freshness::Fresh);
    let second = source.load(&client, false).unwrap();
    assert_eq!(second.freshness, Freshness::NotModified);
    let sent_etag =
        server.seen.lock().unwrap().iter().filter(|(_, h)| h.iter().any(|(k, _)| k == "if-none-match")).count();
    assert_eq!(sent_etag, 1);
    let offline = source.load(&client, true).unwrap();
    assert_eq!(offline.freshness, Freshness::Cached);
    assert_eq!(offline.index, first.index);
    drop(server);
    let down = source.load(&TestServer::client(), false).unwrap();
    assert_eq!(down.freshness, Freshness::Cached);
    assert!(down.warning.is_some());
}

#[test]
fn local_files_load_and_indexes_merge_with_warnings() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("index.json");
    fs::write(&file, example()).unwrap();
    let extra = dir.path().join("extra.json");
    fs::write(&extra, r#"{"format":1,"mods":[{"id":"amber","name":"Other amber","versions":[]},{"id":"frost","name":"Frost","versions":[]}]}"#).unwrap();
    let sources: Vec<Box<dyn ModIndex>> = vec![
        Box::new(StaticIndex::new(file.to_str().unwrap(), dir.path())),
        Box::new(StaticIndex::new(&format!("file://{}", extra.display()), dir.path())),
        Box::new(StaticIndex::new("/no/such/index.json", dir.path())),
        Box::new(StaticIndex::new("http://example.org/index.json", dir.path())),
    ];
    let (index, warnings) = client::load_all(&sources, &crate::download::Client::new(strict()), false);
    assert_eq!(index.mods.len(), 4);
    assert_eq!(index.get("amber").unwrap().name, "Amber", "the first index wins");
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(warnings[1].contains("https"), "{warnings:?}");
}

fn mod_zip(dir: &Path, id: &str, version: &str) -> Vec<u8> {
    let path = dir.join(format!("{id}-{version}.zip"));
    let manifest = format!(r#"{{"id":"{id}","name":"{id}","version":"{version}"}}"#);
    write_zip(&path, &[("mod.json", manifest.as_bytes()), ("assets/content/blocks/x.json", b"{}")]);
    fs::read(path).unwrap()
}

fn index_for(server: &TestServer, entries: &[(&str, &str, &[u8])]) -> Index {
    let mods: Vec<String> = entries
        .iter()
        .map(|(id, version, bytes)| {
            format!(
                r#"{{"id":"{id}","name":"{id}","versions":[{{"version":"{version}","url":"{}","size":{},"sha256":"{}"}}]}}"#,
                server.url(&format!("/{id}.zip")),
                bytes.len(),
                sha256_hex(bytes)
            )
        })
        .collect();
    let json = format!(r#"{{"format":1,"mods":[{}]}}"#, mods.join(","));
    Index::parse(json.as_bytes(), "test", UrlPolicy { allow_loopback_http: true }).unwrap()
}

#[test]
fn a_mod_installs_disabled_and_an_update_keeps_it_enabled() {
    let server = TestServer::start();
    let dir = tempfile::tempdir().unwrap();
    let mods = dir.path().join("mods");
    let downloads = dir.path().join("dl");
    let v1 = mod_zip(dir.path(), "frost", "1.0.0");
    server.put("/frost.zip", v1.clone());
    let index = index_for(&server, &[("frost", "1.0.0", &v1)]);
    let item = index.get("frost").unwrap();
    let client = TestServer::client();
    let package =
        install::install(&client, item, &item.versions[0], &mods, &downloads, &mut |_, _| {}, &AtomicBool::new(false))
            .unwrap();
    assert!(!package.enabled);
    installed::set_enabled(&mods, &package, true).unwrap();

    let v2 = mod_zip(dir.path(), "frost", "1.1.0");
    server.put("/frost.zip", v2.clone());
    let index = index_for(&server, &[("frost", "1.1.0", &v2)]);
    let listed = installed::list(&mods).unwrap();
    let updates = install::updates(&listed, &index, Some("0.1.0"));
    assert_eq!(updates.len(), 1);
    assert_eq!((updates[0].installed.as_str(), updates[0].available.as_str()), ("1.0.0", "1.1.0"));
    let item = index.get("frost").unwrap();
    let package =
        install::install(&client, item, &item.versions[0], &mods, &downloads, &mut |_, _| {}, &AtomicBool::new(false))
            .unwrap();
    assert!(package.enabled);
    let listed = installed::list(&mods).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].manifest.as_ref().unwrap().version, "1.1.0");
    assert!(install::updates(&listed, &index, None).is_empty());
    assert_eq!(fs::read_dir(&downloads).unwrap().count(), 0);
}

#[test]
fn a_tampered_or_mislabelled_mod_is_refused() {
    let server = TestServer::start();
    let dir = tempfile::tempdir().unwrap();
    let mods = dir.path().join("mods");
    let good = mod_zip(dir.path(), "frost", "1.0.0");
    let index = index_for(&server, &[("frost", "1.0.0", &good)]);
    let mut evil = good.clone();
    let last = evil.len() - 30;
    evil[last] ^= 0xff;
    server.put("/frost.zip", evil);
    let item = index.get("frost").unwrap();
    let client = TestServer::client();
    let error =
        install::install(&client, item, &item.versions[0], &mods, dir.path(), &mut |_, _| {}, &AtomicBool::new(false))
            .unwrap_err();
    assert!(error.to_string().contains("SHA-256"), "{error}");

    let other = mod_zip(dir.path(), "notfrost", "1.0.0");
    server.put("/frost.zip", other.clone());
    let index = index_for(&server, &[("frost", "1.0.0", &other)]);
    let item = index.get("frost").unwrap();
    let error =
        install::install(&client, item, &item.versions[0], &mods, dir.path(), &mut |_, _| {}, &AtomicBool::new(false))
            .unwrap_err();
    assert!(error.to_string().contains("index says"), "{error}");
    assert!(installed::list(&mods).unwrap().is_empty());
}

#[test]
fn missing_dependencies_come_from_the_index() {
    let index = Index::parse(&example(), "example", strict()).unwrap();
    let redmoon = index.get("redmoon").unwrap();
    let missing = install::missing_dependencies(&index, redmoon, &redmoon.versions[0], &[], Some("0.3.0"));
    let ids: Vec<(&str, &str)> = missing.iter().map(|(m, v)| (m.id.as_str(), v.version.as_str())).collect();
    assert_eq!(ids, [("amber", "0.2.0")]);
}
