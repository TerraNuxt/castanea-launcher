use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use super::release_api::base_url;
use super::update::{self, Check};
use super::{Build, GameSource, Platform, ReleaseApi, local};
use crate::archive::tests::write_zip;
use crate::download::{Client, Request, Retry, sha256_hex};
use crate::paths::Paths;
use crate::release_server::ReleaseServer;
use crate::settings::{DEFAULT_RELEASE_URL, Settings};

fn platform() -> Platform {
    Platform::detect().unwrap()
}

/// A game archive: the executable and `assets/` at the top, and a filler file so the zip is a few hundred kB.
fn game_zip(dir: &Path, version: &str) -> Vec<u8> {
    let zip = dir.join(format!("game-{version}.zip"));
    let marker = format!("assets/version-{version}.json");
    let mut state = 12345u32;
    let filler: Vec<u8> = (0..400_000)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 24) as u8
        })
        .collect();
    write_zip(&zip, &[(platform().binary(), b"#!/bin/sh\n"), (marker.as_str(), b"{}"), ("assets/filler.bin", &filler)]);
    fs::read(zip).unwrap()
}

fn publish(server: &ReleaseServer, dir: &Path, version: &str, channel: &str, notes: &str) -> Vec<u8> {
    let bytes = game_zip(dir, version);
    server.publish(version, channel, platform().as_str(), notes, bytes.clone());
    bytes
}

fn api(server: &ReleaseServer, beta: bool) -> ReleaseApi {
    ReleaseApi::new(&server.base, platform(), beta).unwrap().with_trust(crate::signature::tests::test_store())
}

fn fast(api: &ReleaseApi) -> Client {
    let mut client = api.client();
    client.retry = Retry { tries: 5, base: Duration::from_millis(5), max_wait: Duration::from_secs(5) };
    client
}

fn fetch_and_swap(api: &ReleaseApi, paths: &Paths) -> crate::Result<Build> {
    let client = fast(api);
    let installed = update::installed(paths);
    let Check::Update(release) = update::check(api, &client, installed.as_ref())? else { panic!("no update") };
    update::download(api, &client, &release, paths, &mut |_, _| {}, &AtomicBool::new(false))?;
    update::activate(paths)
}

#[test]
fn the_server_address_comes_from_the_environment_the_setting_or_the_default() {
    let mut settings = Settings::default();
    assert_eq!(base_url(&settings, None), DEFAULT_RELEASE_URL);
    assert_eq!(base_url(&settings, Some("  ")), DEFAULT_RELEASE_URL);
    settings.release_url = "https://staging.example.org/".into();
    assert_eq!(base_url(&settings, None), "https://staging.example.org/");
    assert_eq!(base_url(&settings, Some("http://127.0.0.1:9")), "http://127.0.0.1:9");
    assert_eq!(
        ReleaseApi::new("https://staging.example.org/", platform(), false).unwrap().base,
        "https://staging.example.org"
    );
}

#[test]
fn only_https_and_the_loopback_are_accepted_as_the_server() {
    assert!(ReleaseApi::new("http://example.org", platform(), false).is_err());
    assert!(ReleaseApi::new("http://127.0.0.1.evil.org", platform(), false).is_err());
    assert!(ReleaseApi::new("ftp://example.org", platform(), false).is_err());
    let local = ReleaseApi::new("http://127.0.0.1:8080", platform(), false).unwrap();
    assert!(local.client().policy.allow_loopback_http);
    let remote = ReleaseApi::new("https://example.org", platform(), false).unwrap();
    assert!(!remote.client().policy.allow_loopback_http, "plain http stays off for a real server");
    assert!(ReleaseApi::new("", platform(), false).unwrap().base == DEFAULT_RELEASE_URL);
}

#[test]
fn an_unsupported_platform_says_so() {
    let error = Platform::from_parts("freebsd", "x86_64").unwrap_err().to_string();
    assert!(error.contains("freebsd") && error.contains("linux-x86_64") && error.contains("windows-x86_64"), "{error}");
}

#[test]
fn a_newer_release_is_found_downloaded_and_replaces_the_old_build() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(&dir.path().join("home"));
    let api = api(&server, false);
    publish(&server, dir.path(), "0.2.0", "stable", "## First\n\n- **fast** start");
    assert!(update::installed(&paths).is_none());
    let first = fetch_and_swap(&api, &paths).unwrap();
    assert_eq!((first.version.as_str(), first.channel.as_str()), ("0.2.0", "stable"));
    assert_eq!(first.binary, update::current_dir(&paths).join(local::BINARY));
    assert!(first.assets().join("version-0.2.0.json").is_file());
    assert_eq!(first.notes, "First\n\n• fast start", "the notes are kept as plain text");
    assert_eq!(update::installed(&paths).unwrap().notes, first.notes, "and survive a restart");
    let seen = server.seen();
    let wanted = format!("/v1/releases/latest?platform={}&channel=stable", platform().as_str());
    assert!(seen.iter().any(|s| s.path == wanted), "{seen:?}");
    assert!(seen.iter().all(|s| !s.path.contains("channel=beta")), "no beta request while it is off");

    publish(&server, dir.path(), "0.3.0", "stable", "second");
    let second = fetch_and_swap(&api, &paths).unwrap();
    assert_eq!(second.version, "0.3.0");
    assert_eq!(update::installed(&paths).unwrap(), second);
    assert!(second.assets().join("version-0.3.0.json").is_file());
    assert!(!second.assets().join("version-0.2.0.json").exists(), "the old build is replaced");
    assert!(!update::previous_dir(&paths).exists() && !update::next_dir(&paths).exists());
    assert_eq!(fs::read_dir(paths.downloads()).unwrap().count(), 0, "the archive is deleted after unpacking");
}

#[test]
fn the_same_or_an_older_version_is_a_no_op() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    let api = api(&server, false);
    publish(&server, dir.path(), "0.2.0", "stable", "");
    let installed = fetch_and_swap(&api, &paths).unwrap();
    let client = fast(&api);
    assert_eq!(update::check(&api, &client, Some(&installed)).unwrap(), Check::UpToDate);
    let newer = Build { version: "0.9.0".into(), ..installed.clone() };
    assert_eq!(update::check(&api, &client, Some(&newer)).unwrap(), Check::UpToDate, "never a downgrade");
    let before = server.downloads().len();
    assert_eq!(update::check(&api, &client, Some(&installed)).unwrap(), Check::UpToDate);
    assert_eq!(server.downloads().len(), before, "a check never downloads");
}

#[test]
fn no_release_yet_is_a_check_result_not_an_error() {
    let server = ReleaseServer::start();
    let api = api(&server, false);
    assert_eq!(update::check(&api, &fast(&api), None).unwrap(), Check::NoRelease);
    let dir = tempfile::tempdir().unwrap();
    publish(&server, dir.path(), "0.2.0", "stable", "");
    // A release for the other platform does not count.
    let other =
        ReleaseApi::new(&server.base, Platform::ALL.into_iter().find(|p| *p != platform()).unwrap(), false).unwrap();
    assert_eq!(update::check(&other, &fast(&other), None).unwrap(), Check::NoRelease);
}

#[test]
fn a_404_that_is_not_the_no_release_answer_is_an_error() {
    let server = ReleaseServer::start();
    let wrong = ReleaseApi::new(&format!("{}/wrong-prefix", server.base), platform(), false).unwrap();
    let error = update::check(&wrong, &fast(&wrong), None).unwrap_err().to_string();
    assert!(error.contains("404") && error.contains("right server"), "{error}");
}

#[test]
fn beta_builds_are_offered_only_when_enabled() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    publish(&server, dir.path(), "0.2.0", "stable", "");
    publish(&server, dir.path(), "0.3.0-beta.1", "beta", "beta notes");
    let client = fast(&api(&server, false));
    let latest = |beta: bool| api(&server, beta).latest(&client).unwrap().unwrap().version;
    assert_eq!(latest(false), "0.2.0");
    assert_eq!(latest(true), "0.3.0-beta.1");
    publish(&server, dir.path(), "0.3.0", "stable", "");
    assert_eq!(latest(true), "0.3.0", "a newer stable release beats an older beta");
    assert_eq!(latest(false), "0.3.0");
    let only_beta = ReleaseServer::start();
    publish(&only_beta, dir.path(), "0.1.0-beta.1", "beta", "");
    let on = api(&only_beta, true);
    assert_eq!(on.latest(&client).unwrap().unwrap().version, "0.1.0-beta.1");
    assert!(api(&only_beta, false).latest(&client).unwrap().is_none());
    // The beta build installs over a stable one and the installed channel is kept.
    let paths = Paths::portable(&dir.path().join("home"));
    let beta_server = ReleaseServer::start();
    publish(&beta_server, dir.path(), "1.0.0", "stable", "");
    fetch_and_swap(&api(&beta_server, true), &paths).unwrap();
    publish(&beta_server, dir.path(), "1.1.0-beta.2", "beta", "");
    let installed = fetch_and_swap(&api(&beta_server, true), &paths).unwrap();
    assert_eq!((installed.version.as_str(), installed.channel.as_str()), ("1.1.0-beta.2", "beta"));
}

#[test]
fn the_release_list_is_newest_first() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    for version in ["0.2.0", "0.10.0", "0.9.0"] {
        publish(&server, dir.path(), version, "stable", "");
    }
    let api = api(&server, false);
    let list = api.list(&fast(&api), super::Channel::Stable).unwrap();
    let versions: Vec<&str> = list.iter().map(|r| r.version.as_str()).collect();
    assert_eq!(versions, ["0.10.0", "0.9.0", "0.2.0"]);
}

#[test]
fn a_hash_mismatch_is_rejected_and_the_old_build_kept() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(&dir.path().join("home"));
    let api = api(&server, false);
    publish(&server, dir.path(), "0.2.0", "stable", "");
    let old = fetch_and_swap(&api, &paths).unwrap();

    publish(&server, dir.path(), "0.3.0", "stable", "");
    server.state.lock().unwrap().entries[1].claimed_sha = Some(sha256_hex(b"some other build"));
    let client = fast(&api);
    let Check::Update(release) = update::check(&api, &client, Some(&old)).unwrap() else { panic!() };
    let error = update::download(&api, &client, &release, &paths, &mut |_, _| {}, &AtomicBool::new(false)).unwrap_err();
    assert!(error.to_string().contains("SHA-256"), "{error}");
    assert!(update::activate(&paths).is_err(), "nothing to activate");
    assert_eq!(update::installed(&paths).unwrap(), old);
    assert!(old.assets().join("version-0.2.0.json").is_file(), "the old build still runs");
    assert!(!update::next_dir(&paths).exists());
    assert_eq!(fs::read_dir(paths.downloads()).unwrap().count(), 0, "the bad file and its partial are gone");
}

#[test]
fn an_etag_of_another_build_is_refused() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    publish(&server, dir.path(), "0.2.0", "stable", "");
    server.state.lock().unwrap().etag = Some("f".repeat(64));
    let paths = Paths::portable(dir.path());
    let error = fetch_and_swap(&api(&server, false), &paths).unwrap_err().to_string();
    assert!(error.contains("ETag"), "{error}");
    assert!(update::installed(&paths).is_none());
}

#[test]
fn a_dropped_connection_is_resumed_with_a_range_and_the_bytes_are_identical() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    let bytes = publish(&server, dir.path(), "0.2.0", "stable", "");
    server.state.lock().unwrap().truncate.extend([150_000, 90_000]);
    let api = api(&server, false);
    let client = fast(&api);
    let release = api.latest(&client).unwrap().unwrap();
    let dest = dir.path().join("dl/file.zip");
    let request = Request {
        url: &release.url,
        sha256: &release.sha256,
        size: Some(release.size),
        max_bytes: 1 << 30,
        headers: Vec::new(),
    };
    let mut last = (0, None);
    client
        .download_resumable(&request, &dest, &mut |done, total| last = (done, total), &AtomicBool::new(false))
        .unwrap();
    assert_eq!(fs::read(&dest).unwrap(), bytes, "identical bytes after two drops");
    assert_eq!(last, (bytes.len() as u64, Some(bytes.len() as u64)));
    assert!(!dest.with_extension("part").exists());
    let downloads = server.downloads();
    assert_eq!(downloads.len(), 3, "{downloads:?}");
    assert_eq!(downloads[0].range, None);
    assert_eq!(downloads[1].range.as_deref(), Some("bytes=150000-"));
    assert_eq!(downloads[2].range.as_deref(), Some("bytes=240000-"));
    assert_eq!(downloads[1].if_range.as_deref(), Some(format!("\"{}\"", release.sha256).as_str()));
}

#[test]
fn a_partial_file_from_an_earlier_start_is_continued_by_the_update() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    let bytes = publish(&server, dir.path(), "0.2.0", "stable", "");
    let paths = Paths::portable(&dir.path().join("home"));
    let api = api(&server, false);
    let client = fast(&api);
    let Check::Update(release) = update::check(&api, &client, None).unwrap() else { panic!() };
    // The launcher was closed after 100 kB: only the partial file is left.
    fs::create_dir_all(paths.downloads()).unwrap();
    fs::write(paths.downloads().join(format!("{}.part", release.sha256)), &bytes[..100_000]).unwrap();
    fs::write(paths.downloads().join("0000.part"), b"another build").unwrap();
    update::download(&api, &client, &release, &paths, &mut |_, _| {}, &AtomicBool::new(false)).unwrap();
    let build = update::activate(&paths).unwrap();
    assert_eq!(build.sha256.as_deref(), Some(release.sha256.as_str()));
    let downloads = server.downloads();
    assert_eq!(downloads.len(), 1);
    assert_eq!(downloads[0].range.as_deref(), Some("bytes=100000-"), "only the rest was fetched");
    assert_eq!(fs::read_dir(paths.downloads()).unwrap().count(), 0, "the other build's partial file is forgotten");
}

#[test]
fn a_partial_file_that_does_not_fit_starts_over() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    let bytes = publish(&server, dir.path(), "0.2.0", "stable", "");
    let api = api(&server, false);
    let client = fast(&api);
    let release = api.latest(&client).unwrap().unwrap();
    let dest = dir.path().join("file.zip");
    let request = Request {
        url: &release.url,
        sha256: &release.sha256,
        size: Some(release.size),
        max_bytes: 1 << 30,
        headers: Vec::new(),
    };
    // Garbage partial bytes: the finished file fails the hash check, the garbage is deleted, a retry is clean.
    fs::write(dest.with_extension("part"), vec![9u8; 5000]).unwrap();
    let error = client.download_resumable(&request, &dest, &mut |_, _| {}, &AtomicBool::new(false)).unwrap_err();
    assert!(matches!(error, crate::Error::Hash { .. }), "{error}");
    assert!(!dest.with_extension("part").exists() && !dest.exists());
    client.download_resumable(&request, &dest, &mut |_, _| {}, &AtomicBool::new(false)).unwrap();
    assert_eq!(fs::read(&dest).unwrap(), bytes);
    // A server that ignores Range sends 200 and the file is simply restarted.
    fs::remove_file(&dest).unwrap();
    fs::write(dest.with_extension("part"), &bytes[..4000]).unwrap();
    server.state.lock().unwrap().ignore_range = true;
    client.download_resumable(&request, &dest, &mut |_, _| {}, &AtomicBool::new(false)).unwrap();
    assert_eq!(fs::read(&dest).unwrap(), bytes);
}

#[test]
fn a_cancelled_download_keeps_its_partial_file() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    publish(&server, dir.path(), "0.2.0", "stable", "");
    let api = api(&server, false);
    let client = fast(&api);
    let release = api.latest(&client).unwrap().unwrap();
    let dest = dir.path().join("file.zip");
    let request = Request {
        url: &release.url,
        sha256: &release.sha256,
        size: Some(release.size),
        max_bytes: 1 << 30,
        headers: Vec::new(),
    };
    let cancel = AtomicBool::new(false);
    let mut progress = |done: u64, _| {
        if done > 100_000 {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    };
    let error = client.download_resumable(&request, &dest, &mut progress, &cancel).unwrap_err();
    assert!(error.to_string().contains("cancelled"), "{error}");
    let kept = fs::metadata(dest.with_extension("part")).unwrap().len();
    assert!(kept > 100_000 && kept < release.size, "{kept}");
    assert!(!dest.exists());
}

#[test]
fn rate_limits_and_server_errors_are_retried_and_retry_after_is_honoured() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    let bytes = publish(&server, dir.path(), "0.2.0", "stable", "");
    let api = api(&server, false);
    let client = fast(&api);
    server.state.lock().unwrap().unavailable = 2;
    let release = api.latest(&client).unwrap().unwrap();
    assert_eq!(server.seen().iter().filter(|s| s.path.contains("/latest")).count(), 3, "two 503, then the answer");

    server.state.lock().unwrap().rate_limited = 1;
    let dest = dir.path().join("file.zip");
    let request = Request {
        url: &release.url,
        sha256: &release.sha256,
        size: Some(release.size),
        max_bytes: 1 << 30,
        headers: Vec::new(),
    };
    let started = Instant::now();
    client.download_resumable(&request, &dest, &mut |_, _| {}, &AtomicBool::new(false)).unwrap();
    assert!(started.elapsed() >= Duration::from_millis(900), "waited for Retry-After: 1, took {:?}", started.elapsed());
    assert_eq!(fs::read(&dest).unwrap(), bytes);

    // Without luck the retries end with a clear error.
    server.state.lock().unwrap().unavailable = 100;
    let mut tight = fast(&api);
    tight.retry.tries = 2;
    let error = api.latest(&tight).unwrap_err().to_string();
    assert!(error.contains("503"), "{error}");
}

#[test]
fn offline_the_check_fails_and_the_installed_build_stays() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    let server = ReleaseServer::start();
    publish(&server, dir.path(), "0.2.0", "stable", "");
    let installed = fetch_and_swap(&api(&server, false), &paths).unwrap();
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let gone = ReleaseApi::new(&format!("http://127.0.0.1:{port}"), platform(), false).unwrap();
    let error = update::check(&gone, &fast(&gone), Some(&installed)).unwrap_err();
    assert!(matches!(error, crate::Error::Http { .. }), "{error}");
    assert_eq!(update::installed(&paths).unwrap(), installed);
}

#[test]
fn an_archive_without_the_game_is_refused_and_nothing_changes() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(&dir.path().join("home"));
    let api = api(&server, false);
    publish(&server, dir.path(), "0.2.0", "stable", "");
    let old = fetch_and_swap(&api, &paths).unwrap();
    for (name, entries) in [
        ("no assets", vec![(platform().binary(), b"x".as_slice())]),
        ("no executable", vec![("assets/a.json", b"{}".as_slice())]),
        ("nested", vec![("castanea-0.3.0/other/castanea", b"x".as_slice())]),
    ] {
        let zip = dir.path().join("bad.zip");
        write_zip(&zip, &entries);
        let mut state = server.state.lock().unwrap();
        state.entries.retain(|e| e.version != "0.3.0");
        let bytes = fs::read(&zip).unwrap();
        drop(state);
        server.publish("0.3.0", "stable", platform().as_str(), "", bytes);
        let error = fetch_and_swap(&api, &paths).unwrap_err().to_string();
        assert!(error.contains("must hold"), "{name}: {error}");
        assert_eq!(update::installed(&paths).unwrap(), old, "{name}");
        assert!(!update::next_dir(&paths).exists(), "{name}");
    }
}

#[test]
fn a_release_must_be_for_this_platform_and_download_from_the_server() {
    let server = ReleaseServer::start();
    let api = api(&server, false);
    let json = |url: &str, platform: &str| serde_json::json!({"version": "0.2.0", "platform": platform, "size": 5, "sha256": "ab".repeat(32), "url": url});
    let good = format!("{}/v1/releases/0.2.0/{}/download", server.base, platform().as_str());
    assert!(api.release_value(json(&good, platform().as_str())).is_ok());
    let other = Platform::ALL.into_iter().find(|p| *p != platform()).unwrap().as_str();
    assert!(api.release_value(json(&good, other)).unwrap_err().to_string().contains("not"));
    let elsewhere = api.release_value(json("https://evil.example.org/x.zip", platform().as_str())).unwrap_err();
    assert!(elsewhere.to_string().contains("not on"), "{elsewhere}");
    assert!(api.release_value(json(&format!("{}.evil.org/x", server.base), platform().as_str())).is_err());
}

#[test]
fn a_stopped_swap_falls_back_to_the_previous_build() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    let server = ReleaseServer::start();
    publish(&server, dir.path(), "0.2.0", "stable", "");
    fetch_and_swap(&api(&server, false), &paths).unwrap();
    fs::rename(update::current_dir(&paths), update::previous_dir(&paths)).unwrap();
    assert_eq!(update::installed(&paths).unwrap().version, "0.2.0");
}

#[test]
fn local_folders_and_checkouts_are_read_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let release = dir.path().join("castanea-0.4.0");
    fs::create_dir_all(release.join("assets")).unwrap();
    fs::write(release.join(local::BINARY), "bin").unwrap();
    fs::write(release.join("version.txt"), "0.4.0\n").unwrap();
    let checkout = dir.path().join("checkout");
    fs::create_dir_all(checkout.join("assets")).unwrap();
    fs::create_dir_all(checkout.join("target/release")).unwrap();
    fs::write(checkout.join("target/release").join(local::BINARY), "bin").unwrap();
    fs::write(checkout.join("Cargo.toml"), "[package]\nname = \"castanea\"\nversion = \"0.1.0\"\n").unwrap();
    let empty = dir.path().join("empty");
    fs::create_dir_all(&empty).unwrap();

    assert_eq!(local::inspect(&release).unwrap().version, "0.4.0");
    let build = local::inspect(&checkout).unwrap();
    assert_eq!(build.version, "0.1.0");
    assert!(build.binary.ends_with(Path::new("target/release").join(local::BINARY)));
    assert!(build.channel.is_empty());
    assert!(local::inspect(&empty).is_err());
}

mod signed;
