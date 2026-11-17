//! Signatures on game builds: a build without a good signature is never installed.

use std::fs;
use std::sync::atomic::AtomicBool;

use super::*;
use crate::install::Release;
use crate::release_server::Sig;
use crate::signature::tests::test_store;

fn publish_sig(server: &ReleaseServer, dir: &Path, version: &str, sig: Sig) {
    server.publish_as("game", sig, version, "stable", platform().as_str(), "", game_zip(dir, version));
}

#[test]
fn a_signed_build_installs() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    publish(&server, dir.path(), "0.2.0", "stable", "");
    assert_eq!(fetch_and_swap(&api(&server, false), &paths).unwrap().version, "0.2.0");
}

#[test]
fn a_missing_garbled_foreign_or_unknown_key_signature_installs_nothing_and_keeps_the_build() {
    for sig in [Sig::Missing, Sig::Garbage, Sig::WrongKey, Sig::UnknownId] {
        let server = ReleaseServer::start();
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::portable(&dir.path().join("home"));
        let api = api(&server, false);
        publish(&server, dir.path(), "0.2.0", "stable", "");
        let old = fetch_and_swap(&api, &paths).unwrap();
        publish_sig(&server, dir.path(), "0.3.0", sig);
        let client = fast(&api);
        let Check::Update(release) = update::check(&api, &client, Some(&old)).unwrap() else { panic!("{sig:?}") };
        let error = update::download(&api, &client, &release, &paths, &mut |_, _| {}, &AtomicBool::new(false))
            .unwrap_err()
            .to_string();
        assert!(error.contains("this download is not signed by castanea"), "{sig:?}: {error}");
        assert_eq!(update::installed(&paths).unwrap(), old, "{sig:?}");
        assert!(!update::next_dir(&paths).exists(), "{sig:?}: nothing was unpacked");
        assert_eq!(fs::read_dir(paths.downloads()).unwrap().count(), 0, "{sig:?}: the file is deleted");
    }
}

#[test]
fn the_trust_list_decides_not_the_server() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    publish(&server, dir.path(), "0.2.0", "stable", "");
    // The embedded placeholder key signed nothing, so a launcher with its own list refuses the build.
    let untrusting = ReleaseApi::new(&server.base, platform(), false).unwrap();
    assert_ne!(untrusting.trust, test_store());
    let paths = Paths::portable(&dir.path().join("home"));
    let error = fetch_and_swap(&untrusting, &paths).unwrap_err().to_string();
    assert!(error.contains("not signed by castanea"), "{error}");
}

#[test]
fn unsigned_builds_install_only_with_the_developer_switch() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    publish_sig(&server, dir.path(), "0.2.0", Sig::Missing);
    let paths = Paths::portable(&dir.path().join("home"));
    assert!(fetch_and_swap(&api(&server, false), &paths).is_err(), "the default refuses");
    let mut allowing = api(&server, false);
    allowing.allow_unsigned = true;
    assert_eq!(fetch_and_swap(&allowing, &paths).unwrap().version, "0.2.0");
    let settings = Settings { allow_unsigned: true, ..Settings::default() };
    assert_eq!(settings.unsigned_allowed(), cfg!(debug_assertions), "a release build ignores the switch");
    assert!(!Settings::default().allow_unsigned, "off by default");
}

#[test]
fn a_signature_for_another_size_or_channel_fails_on_the_real_file() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    publish(&server, dir.path(), "0.2.0", "stable", "");
    let api = api(&server, false);
    let client = fast(&api);
    let release = api.latest(&client).unwrap().unwrap();
    api.verify(&release, &release.sha256, release.size).unwrap();
    assert!(api.verify(&release, &release.sha256, release.size + 1).is_err(), "the file's own size counts");
    assert!(api.verify(&release, &"0".repeat(64), release.size).is_err(), "the file's own hash counts");
    let moved = Release { channel: "beta".into(), ..release.clone() };
    assert!(api.verify(&moved, &release.sha256, release.size).is_err());
}
