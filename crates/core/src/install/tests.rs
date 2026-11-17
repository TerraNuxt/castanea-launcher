use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use super::detect;
use super::github::{GithubReleases, parse_sums};
use super::local::{self, BINARY};
use super::registry::{Origin, Registry};
use super::source::{GameSource, asset_fits};
use crate::archive::tests::write_zip;
use crate::download::sha256_hex;
use crate::paths::Paths;
use crate::test_server::TestServer;

const LINUX: &str = "x86_64-unknown-linux-gnu";

fn game_zip(dir: &Path) -> Vec<u8> {
    let zip = dir.join("game.zip");
    write_zip(&zip, &[("castanea-0.2.0/castanea", b"#!/bin/sh\n"), ("castanea-0.2.0/assets/engine/a.json", b"{}")]);
    fs::read(zip).unwrap()
}

fn releases_json(base: &str, digest: Option<&str>) -> String {
    let digest = digest.map_or("null".to_owned(), |d| format!("\"sha256:{d}\""));
    format!(
        r#"[
        {{"tag_name":"v0.2.0","name":"","draft":false,"prerelease":false,"published_at":"2026-11-01T00:00:00Z","body":"notes",
          "assets":[{{"name":"castanea-0.2.0-{LINUX}.zip","url":"{base}/api/asset/1","browser_download_url":"{base}/dl/game.zip","size":SIZE,"digest":{digest}}},
                    {{"name":"SHA256SUMS","url":"{base}/api/asset/2","browser_download_url":"{base}/dl/SHA256SUMS","size":10}}]}},
        {{"tag_name":"v0.3.0-draft","draft":true,"assets":[]}},
        {{"tag_name":"v0.1.0","prerelease":true,"assets":[{{"name":"castanea-0.1.0-x86_64-pc-windows-msvc.zip","url":"u","browser_download_url":"b","size":1}}]}}
    ]"#
    )
}

fn source(server: &TestServer, token: Option<&str>) -> GithubReleases {
    let mut source = GithubReleases::new("TerraNuxt/castanea", token.map(str::to_owned));
    source.api = server.base.clone();
    source.target = LINUX.to_owned();
    source
}

#[test]
fn asset_names_fit_their_platform() {
    assert!(asset_fits("castanea-0.2.0-x86_64-unknown-linux-gnu.zip", LINUX));
    assert!(asset_fits("castanea-0.2.0-linux.zip", LINUX));
    assert!(!asset_fits("castanea-0.2.0-windows.zip", LINUX));
    assert!(asset_fits("Castanea-0.2.0-x86_64-pc-windows-msvc.zip", "x86_64-pc-windows-msvc"));
    assert!(!asset_fits("castanea-0.2.0-linux.tar.gz", LINUX));
    assert!(!asset_fits("other-linux.zip", LINUX));
}

#[test]
fn sums_files_are_read() {
    let hash = "a".repeat(64);
    let text = format!("{hash}  castanea-0.2.0-linux.zip\n{}  *other.zip\n", "b".repeat(64));
    assert_eq!(parse_sums(&text, "castanea-0.2.0-linux.zip"), Some(hash));
    assert_eq!(parse_sums(&text, "other.zip"), Some("b".repeat(64)));
    assert_eq!(parse_sums(&text, "missing.zip"), None);
}

#[test]
fn releases_list_skips_drafts_and_picks_the_platform_asset() {
    let server = TestServer::start();
    let json = releases_json(&server.base, None).replace("SIZE", "5");
    server.put("/repos/TerraNuxt/castanea/releases?per_page=30", json);
    let builds = source(&server, None).list(&TestServer::client()).unwrap();
    assert_eq!(builds.len(), 2);
    assert_eq!(builds[0].id, "v0.2.0");
    assert_eq!(builds[0].version, "0.2.0");
    assert_eq!(builds[0].title, "v0.2.0");
    assert!(builds[0].asset.is_some());
    assert!(builds[1].prerelease);
    assert!(builds[1].asset.is_none(), "only a Windows asset");
}

#[test]
fn a_release_installs_with_the_digest_and_the_token_goes_to_the_api_url() {
    let server = TestServer::start();
    let dir = tempfile::tempdir().unwrap();
    let bytes = game_zip(dir.path());
    let json = releases_json(&server.base, Some(&sha256_hex(&bytes))).replace("SIZE", &bytes.len().to_string());
    server.put("/repos/TerraNuxt/castanea/releases?per_page=30", json);
    server.put("/api/asset/1", bytes);
    let source = source(&server, Some("secret-token"));
    let client = TestServer::client();
    let build = source.list(&client).unwrap().remove(0);
    let paths = Paths::portable(dir.path());
    let installed = source
        .install(&client, &build, &paths.versions(), &paths.downloads(), &mut |_, _| {}, &AtomicBool::new(false))
        .unwrap();
    assert!(installed.managed);
    assert!(installed.is_present());
    assert_eq!(installed.binary, paths.versions().join("v0.2.0").join(BINARY));
    assert!(paths.versions().join("v0.2.0/install.json").is_file());
    let seen = server.seen.lock().unwrap();
    let (path, headers) = seen.iter().find(|(p, _)| p == "/api/asset/1").unwrap();
    assert_eq!(path, "/api/asset/1");
    assert!(headers.iter().any(|(k, v)| k == "authorization" && v == "Bearer secret-token"));
    assert!(headers.iter().any(|(k, v)| k == "accept" && v == "application/octet-stream"));
    assert_eq!(fs::read_dir(paths.downloads()).unwrap().count(), 0, "the archive is deleted after unpacking");
}

#[test]
fn a_release_falls_back_to_sha256sums_and_refuses_a_wrong_archive() {
    let server = TestServer::start();
    let dir = tempfile::tempdir().unwrap();
    let bytes = game_zip(dir.path());
    let json = releases_json(&server.base, None).replace("SIZE", &bytes.len().to_string());
    server.put("/repos/TerraNuxt/castanea/releases?per_page=30", json);
    server.put("/dl/game.zip", bytes.clone());
    server.put("/dl/SHA256SUMS", format!("{}  castanea-0.2.0-{LINUX}.zip\n", sha256_hex(&bytes)));
    let source = source(&server, None);
    let client = TestServer::client();
    let build = source.list(&client).unwrap().remove(0);
    let paths = Paths::portable(dir.path());
    source
        .install(&client, &build, &paths.versions(), &paths.downloads(), &mut |_, _| {}, &AtomicBool::new(false))
        .unwrap();

    let mut tampered = bytes.clone();
    tampered[10] ^= 1;
    server.put("/dl/game.zip", tampered);
    let mut again = build.clone();
    again.id = "v0.2.0-again".to_owned();
    let error = source
        .install(&client, &again, &paths.versions(), &paths.downloads(), &mut |_, _| {}, &AtomicBool::new(false))
        .unwrap_err();
    assert!(error.to_string().contains("SHA-256"), "{error}");
    assert!(!paths.versions().join("v0.2.0-again").exists());
}

#[test]
fn a_release_without_any_checksum_is_refused() {
    let server = TestServer::start();
    let json = releases_json(&server.base, None).replace("SIZE", "5").replace("\"SHA256SUMS\"", "\"notes.txt\"");
    server.put("/repos/TerraNuxt/castanea/releases?per_page=30", json);
    let dir = tempfile::tempdir().unwrap();
    let source = source(&server, None);
    let client = TestServer::client();
    let build = source.list(&client).unwrap().remove(0);
    let paths = Paths::portable(dir.path());
    let error = source
        .install(&client, &build, &paths.versions(), &paths.downloads(), &mut |_, _| {}, &AtomicBool::new(false))
        .unwrap_err();
    assert!(error.to_string().contains("no checksum"), "{error}");
}

#[test]
fn local_folders_and_checkouts_are_found_and_registered_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let release = dir.path().join("castanea-0.4.0");
    fs::create_dir_all(release.join("assets")).unwrap();
    fs::write(release.join(BINARY), "bin").unwrap();
    fs::write(release.join("version.txt"), "0.4.0\n").unwrap();
    let checkout = dir.path().join("checkout");
    fs::create_dir_all(checkout.join("assets")).unwrap();
    fs::create_dir_all(checkout.join("target/release")).unwrap();
    fs::write(checkout.join("target/release").join(BINARY), "bin").unwrap();
    fs::write(checkout.join("Cargo.toml"), "[package]\nname = \"castanea\"\nversion = \"0.1.0\"\n").unwrap();
    let empty = dir.path().join("empty");
    fs::create_dir_all(&empty).unwrap();

    let mut registry = Registry::default();
    let found = detect::detect(&registry, &[release.clone(), checkout.clone(), empty.clone(), release.clone()]);
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(found[0].version, "0.4.0");
    assert_eq!(found[1].version, "0.1.0");
    assert_eq!(found[1].id, "local-checkout");
    assert!(local::inspect(&registry, &empty, Origin::Local).is_err());
    for build in found {
        registry.put(build);
    }
    assert!(detect::detect(&registry, std::slice::from_ref(&release)).is_empty(), "already known");

    let paths = Paths::portable(dir.path());
    registry.remove(&paths, "local-castanea-0.4.0").unwrap().unwrap();
    assert!(release.join(BINARY).is_file(), "a registered folder is never deleted");
}

#[test]
fn managed_builds_are_deleted_and_adopted_back() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    let root = paths.versions().join("v1");
    fs::create_dir_all(root.join("assets")).unwrap();
    fs::write(root.join(BINARY), "bin").unwrap();
    let mut registry = Registry::default();
    let build = super::Build {
        id: "v1".into(),
        version: "1.0.0".into(),
        root: root.clone(),
        binary: root.join(BINARY),
        origin: Origin::Github { repo: "a/b".into(), tag: "v1".into(), asset: "x.zip".into() },
        sha256: None,
        managed: true,
    };
    build.write_install_file().unwrap();
    let mut adopted = Registry::default();
    adopted.adopt_managed(&paths);
    assert_eq!(adopted.builds, std::slice::from_ref(&build));
    registry.put(build);
    assert_eq!(registry.free_id("v1"), "v1-2");
    registry.remove(&paths, "v1").unwrap();
    assert!(!root.exists());
}
