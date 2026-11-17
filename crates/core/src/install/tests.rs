use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use super::github::{GithubReleases, parse_sums};
use super::local::{self, BINARY};
use super::source::{GameSource, RemoteBuild, asset_fits};
use super::update::{self, Check};
use crate::archive::tests::write_zip;
use crate::download::sha256_hex;
use crate::paths::Paths;
use crate::test_server::TestServer;

const LINUX: &str = "x86_64-unknown-linux-gnu";
const RELEASES: &str = "/repos/TerraNuxt/castanea/releases?per_page=30";

fn game_zip(dir: &Path, version: &str) -> Vec<u8> {
    let zip = dir.join(format!("game-{version}.zip"));
    let binary = format!("castanea-{version}/{BINARY}");
    let marker = format!("castanea-{version}/assets/version-{version}.json");
    write_zip(&zip, &[(binary.as_str(), b"#!/bin/sh\n"), (marker.as_str(), b"{}")]);
    fs::read(zip).unwrap()
}

/// One release `version` whose zip is at `/dl/<version>.zip`, plus a draft and a Windows-only pre-release.
fn releases_json(base: &str, version: &str, size: usize, digest: Option<&str>) -> String {
    let digest = digest.map_or("null".to_owned(), |d| format!("\"sha256:{d}\""));
    format!(
        r#"[
        {{"tag_name":"v{version}","name":"","draft":false,"prerelease":false,"published_at":"2026-11-01T00:00:00Z",
          "assets":[{{"name":"castanea-{version}-{LINUX}.zip","url":"{base}/api/asset/{version}","browser_download_url":"{base}/dl/{version}.zip","size":{size},"digest":{digest}}},
                    {{"name":"SHA256SUMS","url":"{base}/api/sums","browser_download_url":"{base}/dl/SHA256SUMS","size":10}}]}},
        {{"tag_name":"v9.9.9-draft","draft":true,"assets":[]}},
        {{"tag_name":"v0.0.1","prerelease":true,"assets":[{{"name":"castanea-0.0.1-x86_64-pc-windows-msvc.zip","url":"u","browser_download_url":"b","size":1}}]}}
    ]"#
    )
}

fn source(server: &TestServer, token: Option<&str>) -> GithubReleases {
    let mut source = GithubReleases::new("TerraNuxt/castanea", token.map(str::to_owned));
    source.api = server.base.clone();
    source.target = LINUX.to_owned();
    source
}

/// Publishes `version` on the server with its digest; returns the zip bytes.
fn publish(server: &TestServer, dir: &Path, version: &str) -> Vec<u8> {
    let bytes = game_zip(dir, version);
    let json = releases_json(&server.base, version, bytes.len(), Some(&sha256_hex(&bytes)));
    server.put(RELEASES, json);
    server.put(&format!("/dl/{version}.zip"), bytes.clone());
    bytes
}

fn remote(tag: &str, version: &str, prerelease: bool, asset: bool) -> RemoteBuild {
    let asset = asset.then(|| super::source::Asset {
        name: "castanea.zip".into(),
        download_url: String::new(),
        api_url: String::new(),
        size: 1,
        sha256: None,
        sums_url: None,
    });
    RemoteBuild {
        tag: tag.into(),
        version: version.into(),
        title: tag.into(),
        published: String::new(),
        prerelease,
        asset,
    }
}

fn update_to_newest(source: &GithubReleases, paths: &Paths) -> crate::Result<super::Build> {
    let client = TestServer::client();
    let installed = update::installed(paths);
    let Check::Update(remote) = update::check(source, &client, installed.as_ref())? else { panic!("no update") };
    update::download(source, &client, &remote, paths, &mut |_, _| {}, &AtomicBool::new(false))?;
    update::activate(paths)
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
fn releases_list_skips_drafts_and_the_newest_stable_release_wins() {
    let server = TestServer::start();
    server.put(RELEASES, releases_json(&server.base, "0.2.0", 5, None));
    let builds = source(&server, None).list(&TestServer::client()).unwrap();
    assert_eq!(builds.len(), 2);
    assert_eq!((builds[0].tag.as_str(), builds[0].version.as_str()), ("v0.2.0", "0.2.0"));
    assert!(builds[1].asset.is_none(), "only a Windows asset");

    let list = vec![
        remote("v0.10.0", "0.10.0", false, true),
        remote("v0.9.0", "0.9.0", false, true),
        remote("v0.11.0-rc.1", "0.11.0-rc.1", true, true),
        remote("v1.0.0", "1.0.0", false, false),
    ];
    assert_eq!(update::latest(list).unwrap().tag, "v0.10.0", "stable, with an asset, by version not by text");
    let only_pre = vec![remote("v0.1.0-rc.1", "0.1.0-rc.1", true, true)];
    assert_eq!(update::latest(only_pre).unwrap().tag, "v0.1.0-rc.1", "a pre-release when there is nothing else");
    assert!(update::latest(Vec::new()).is_none());
}

#[test]
fn an_update_is_wanted_for_another_newer_release_only() {
    let installed = super::Build { version: "0.2.0".into(), tag: "v0.2.0".into(), ..Default::default() };
    assert!(update::wanted(&remote("v0.3.0", "0.3.0", false, true), Some(&installed)));
    assert!(!update::wanted(&remote("v0.2.0", "0.2.0", false, true), Some(&installed)));
    assert!(!update::wanted(&remote("v0.1.0", "0.1.0", false, true), Some(&installed)), "never a downgrade");
    assert!(update::wanted(&remote("v0.1.0", "0.1.0", false, true), None));
}

#[test]
fn a_newer_release_is_found_downloaded_and_replaces_the_old_build() {
    let server = TestServer::start();
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(&dir.path().join("home"));
    let source = source(&server, None);
    publish(&server, dir.path(), "0.2.0");
    assert!(update::installed(&paths).is_none());
    let first = update_to_newest(&source, &paths).unwrap();
    assert_eq!((first.tag.as_str(), first.version.as_str()), ("v0.2.0", "0.2.0"));
    assert_eq!(first.binary, update::current_dir(&paths).join(BINARY));
    assert!(first.assets().join("version-0.2.0.json").is_file());
    let client = TestServer::client();
    assert_eq!(update::check(&source, &client, Some(&first)).unwrap(), Check::UpToDate);

    publish(&server, dir.path(), "0.3.0");
    let second = update_to_newest(&source, &paths).unwrap();
    assert_eq!(second.version, "0.3.0");
    assert_eq!(update::installed(&paths).unwrap(), second);
    assert!(second.assets().join("version-0.3.0.json").is_file());
    assert!(!second.assets().join("version-0.2.0.json").exists(), "the old build is replaced");
    assert!(!update::previous_dir(&paths).exists() && !update::next_dir(&paths).exists());
    assert_eq!(fs::read_dir(paths.downloads()).unwrap().count(), 0, "the archive is deleted after unpacking");
}

#[test]
fn a_hash_mismatch_is_rejected_and_the_old_build_kept() {
    let server = TestServer::start();
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(&dir.path().join("home"));
    let source = source(&server, None);
    publish(&server, dir.path(), "0.2.0");
    let old = update_to_newest(&source, &paths).unwrap();

    let mut tampered = publish(&server, dir.path(), "0.3.0");
    tampered[10] ^= 1;
    server.put("/dl/0.3.0.zip", tampered);
    let client = TestServer::client();
    let Check::Update(remote) = update::check(&source, &client, Some(&old)).unwrap() else { panic!() };
    let error =
        update::download(&source, &client, &remote, &paths, &mut |_, _| {}, &AtomicBool::new(false)).unwrap_err();
    assert!(error.to_string().contains("SHA-256"), "{error}");
    assert!(update::activate(&paths).is_err(), "nothing to activate");
    assert_eq!(update::installed(&paths).unwrap(), old);
    assert!(old.assets().join("version-0.2.0.json").is_file(), "the old build still runs");
    assert!(!update::next_dir(&paths).exists());
}

#[test]
fn offline_the_check_fails_and_the_installed_build_stays() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    let server = TestServer::start();
    publish(&server, dir.path(), "0.2.0");
    let mut source = source(&server, None);
    let installed = update_to_newest(&source, &paths).unwrap();
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    source.api = format!("http://127.0.0.1:{port}");
    let error = update::check(&source, &TestServer::client(), Some(&installed)).unwrap_err();
    assert!(matches!(error, crate::Error::Http { .. }), "{error}");
    assert_eq!(update::installed(&paths).unwrap(), installed);
}

#[test]
fn a_stopped_swap_falls_back_to_the_previous_build() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    let server = TestServer::start();
    publish(&server, dir.path(), "0.2.0");
    let source = source(&server, None);
    update_to_newest(&source, &paths).unwrap();
    fs::rename(update::current_dir(&paths), update::previous_dir(&paths)).unwrap();
    assert_eq!(update::installed(&paths).unwrap().version, "0.2.0");
}

#[test]
fn the_token_goes_to_the_api_url() {
    let server = TestServer::start();
    let dir = tempfile::tempdir().unwrap();
    let bytes = game_zip(dir.path(), "0.2.0");
    server.put(RELEASES, releases_json(&server.base, "0.2.0", bytes.len(), Some(&sha256_hex(&bytes))));
    server.put("/api/asset/0.2.0", bytes);
    let source = source(&server, Some("secret-token"));
    let paths = Paths::portable(dir.path());
    update_to_newest(&source, &paths).unwrap();
    let seen = server.seen.lock().unwrap();
    let (_, headers) = seen.iter().find(|(p, _)| p == "/api/asset/0.2.0").unwrap();
    assert!(headers.iter().any(|(k, v)| k == "authorization" && v == "Bearer secret-token"));
    assert!(headers.iter().any(|(k, v)| k == "accept" && v == "application/octet-stream"));
}

#[test]
fn a_release_falls_back_to_sha256sums_or_is_refused_without_a_checksum() {
    let server = TestServer::start();
    let dir = tempfile::tempdir().unwrap();
    let bytes = game_zip(dir.path(), "0.2.0");
    server.put(RELEASES, releases_json(&server.base, "0.2.0", bytes.len(), None));
    server.put("/dl/0.2.0.zip", bytes.clone());
    server.put("/dl/SHA256SUMS", format!("{}  castanea-0.2.0-{LINUX}.zip\n", sha256_hex(&bytes)));
    let source = source(&server, None);
    let paths = Paths::portable(&dir.path().join("a"));
    assert_eq!(update_to_newest(&source, &paths).unwrap().version, "0.2.0");

    let json = releases_json(&server.base, "0.2.0", bytes.len(), None).replace("\"SHA256SUMS\"", "\"notes.txt\"");
    server.put(RELEASES, json);
    let error = update_to_newest(&source, &Paths::portable(&dir.path().join("b"))).unwrap_err();
    assert!(error.to_string().contains("no checksum"), "{error}");
}

#[test]
fn local_folders_and_checkouts_are_read_in_place() {
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

    assert_eq!(local::inspect(&release).unwrap().version, "0.4.0");
    let build = local::inspect(&checkout).unwrap();
    assert_eq!(build.version, "0.1.0");
    assert!(build.binary.ends_with(Path::new("target/release").join(BINARY)));
    assert!(build.tag.is_empty());
    assert!(local::inspect(&empty).is_err());
}
