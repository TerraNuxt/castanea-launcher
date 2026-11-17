//! The launcher self-update against the local release server, and its file-based state machine without any process.

use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use super::*;
use crate::archive::tests::write_zip;
use crate::download::{Client, Retry};
use crate::install::{Platform, ReleaseApi};
use crate::release_server::{ReleaseServer, Sig};
use crate::signature::tests::test_store;

fn platform() -> Platform {
    Platform::detect().unwrap()
}

/// A launcher archive: the executable at its root, with filler so a drop can cut it.
fn launcher_zip(dir: &Path, version: &str) -> Vec<u8> {
    let zip = dir.join(format!("launcher-{version}.zip"));
    let body = format!("launcher {version}\n").into_bytes();
    let mut state = 777u32;
    let filler: Vec<u8> = (0..300_000)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 24) as u8
        })
        .collect();
    write_zip(&zip, &[(platform().launcher_binary(), &body), ("filler.bin", &filler)]);
    fs::read(zip).unwrap()
}

fn publish(server: &ReleaseServer, dir: &Path, version: &str, channel: &str, sig: Sig) {
    let zip = launcher_zip(dir, version);
    server.publish_as("launcher", sig, version, channel, platform().as_str(), "launcher notes", zip);
}

fn api(server: &ReleaseServer, beta: bool) -> ReleaseApi {
    ReleaseApi::new(&server.base, platform(), beta).unwrap().with_trust(test_store()).for_launcher()
}

fn fast(api: &ReleaseApi) -> Client {
    let mut client = api.client();
    client.retry = Retry { tries: 5, base: Duration::from_millis(5), max_wait: Duration::from_secs(5) };
    client
}

fn found(api: &ReleaseApi, running: &str, files: &Files) -> Release {
    let Check::Update(release) = check(api, &fast(api), running, files).unwrap() else { panic!("no update") };
    *release
}

fn do_stage(api: &ReleaseApi, release: &Release, paths: &Paths) -> Result<Staged> {
    stage(api, &fast(api), release, paths, &mut |_, _| {}, &AtomicBool::new(false))
}

#[test]
fn a_newer_launcher_is_found_staged_and_checked_by_hash_and_signature() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    let files = Files::new(&paths);
    let api = api(&server, false);
    assert_eq!(check(&api, &fast(&api), "0.1.0", &files).unwrap(), Check::NoRelease);
    publish(&server, dir.path(), "0.2.0", "stable", Sig::Valid);
    let release = found(&api, "0.1.0", &files);
    assert_eq!((release.product.as_str(), release.version.as_str()), ("launcher", "0.2.0"));
    let staged = do_stage(&api, &release, &paths).unwrap();
    assert_eq!(staged.version, "0.2.0");
    assert_eq!(fs::read_to_string(&staged.binary).unwrap(), "launcher 0.2.0\n");
    assert_eq!(staged.notes, "launcher notes");
    assert_eq!(files.staged().unwrap(), staged);
    assert_eq!(fs::read_dir(paths.downloads()).unwrap().count(), 0, "the archive is deleted after unpacking");
    let seen = server.seen();
    assert!(seen.iter().all(|s| !s.path.contains("/latest") || s.path.contains("product=launcher")), "{seen:?}");
    assert!(seen.iter().any(|s| s.path.ends_with("/download?product=launcher")), "{seen:?}");
    let before = server.downloads().len();
    assert_eq!(do_stage(&api, &release, &paths).unwrap(), staged, "a staged copy of this release is reused");
    assert_eq!(server.downloads().len(), before);
}

#[test]
fn the_same_an_older_or_a_skipped_version_is_a_no_op() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    let files = Files::new(&Paths::portable(dir.path()));
    publish(&server, dir.path(), "0.2.0", "stable", Sig::Valid);
    let api = api(&server, false);
    let client = fast(&api);
    assert_eq!(check(&api, &client, "0.2.0", &files).unwrap(), Check::UpToDate);
    assert_eq!(check(&api, &client, "0.3.0", &files).unwrap(), Check::UpToDate, "never a downgrade");
    assert_eq!(check(&api, &client, "0.10.0", &files).unwrap(), Check::UpToDate, "numbers, not text");
    assert!(matches!(check(&api, &client, "0.1.0", &files).unwrap(), Check::Update(_)));
    files.skip("0.2.0").unwrap();
    assert_eq!(check(&api, &client, "0.1.0", &files).unwrap(), Check::UpToDate, "a version that failed is skipped");
    assert_eq!(server.downloads().len(), 0, "a check never downloads");
}

#[test]
fn the_launcher_and_the_game_do_not_see_each_others_releases() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    let files = Files::new(&Paths::portable(dir.path()));
    server.publish("9.0.0", "stable", platform().as_str(), "", vec![1, 2, 3]);
    let api = api(&server, false);
    assert_eq!(check(&api, &fast(&api), "0.1.0", &files).unwrap(), Check::NoRelease);
}

#[test]
fn a_launcher_without_a_valid_signature_is_refused_and_nothing_is_staged() {
    for sig in [Sig::Missing, Sig::Garbage, Sig::WrongKey, Sig::UnknownId] {
        let server = ReleaseServer::start();
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::portable(dir.path());
        let files = Files::new(&paths);
        publish(&server, dir.path(), "0.2.0", "stable", sig);
        let api = api(&server, false);
        let release = found(&api, "0.1.0", &files);
        let error = do_stage(&api, &release, &paths).unwrap_err().to_string();
        assert!(error.contains("this download is not signed by castanea"), "{sig:?}: {error}");
        assert!(files.staged().is_none() && !files.staged_dir().exists(), "{sig:?}");
        assert_eq!(fs::read_dir(paths.downloads()).unwrap().count(), 0, "{sig:?}");
        let mut lenient = api_allowing_unsigned(&server);
        lenient.allow_unsigned = true;
        let again = found(&lenient, "0.1.0", &files);
        assert!(do_stage(&lenient, &again, &paths).is_err(), "{sig:?}: the launcher never accepts an unsigned update");
    }
}

fn api_allowing_unsigned(server: &ReleaseServer) -> ReleaseApi {
    let mut api = api(server, false);
    api.allow_unsigned = true;
    api
}

#[test]
fn a_hash_mismatch_is_refused() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    let files = Files::new(&paths);
    publish(&server, dir.path(), "0.2.0", "stable", Sig::Valid);
    server.state.lock().unwrap().entries[0].claimed_sha = Some("ab".repeat(32));
    let api = api(&server, false);
    let release = found(&api, "0.1.0", &files);
    let error = do_stage(&api, &release, &paths).unwrap_err().to_string();
    assert!(error.contains("SHA-256"), "{error}");
    assert!(files.staged().is_none());
    assert_eq!(fs::read_dir(paths.downloads()).unwrap().count(), 0);
}

#[test]
fn an_interrupted_download_is_resumed() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    let files = Files::new(&paths);
    publish(&server, dir.path(), "0.2.0", "stable", Sig::Valid);
    server.state.lock().unwrap().truncate.extend([100_000, 50_000]);
    let api = api(&server, false);
    let release = found(&api, "0.1.0", &files);
    let staged = do_stage(&api, &release, &paths).unwrap();
    assert_eq!(fs::read_to_string(staged.binary).unwrap(), "launcher 0.2.0\n");
    let downloads = server.downloads();
    assert_eq!(downloads.len(), 3, "{downloads:?}");
    assert_eq!(downloads[1].range.as_deref(), Some("bytes=100000-"));
    assert_eq!(downloads[2].range.as_deref(), Some("bytes=150000-"));
}

#[test]
fn offline_the_check_fails_quietly_and_nothing_changes() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    let files = Files::new(&paths);
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let gone = ReleaseApi::new(&format!("http://127.0.0.1:{port}"), platform(), false).unwrap().for_launcher();
    let error = check(&gone, &fast(&gone), "0.1.0", &files).unwrap_err();
    assert!(matches!(error, crate::Error::Http { .. }), "{error}");
    assert!(files.staged().is_none() && files.pending().is_none());
}

#[test]
fn beta_launchers_follow_the_beta_toggle() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    let files = Files::new(&Paths::portable(dir.path()));
    publish(&server, dir.path(), "0.2.0", "stable", Sig::Valid);
    publish(&server, dir.path(), "0.3.0-beta.1", "beta", Sig::Valid);
    let stable = api(&server, false);
    let beta = api(&server, true);
    assert_eq!(found(&stable, "0.1.0", &files).version, "0.2.0");
    assert_eq!(found(&beta, "0.1.0", &files).version, "0.3.0-beta.1");
    assert!(server.seen().iter().any(|s| s.path.contains("channel=beta") && s.path.contains("product=launcher")));
    let release = found(&beta, "0.2.0", &files);
    assert_eq!((release.channel.as_str(), release.version.as_str()), ("beta", "0.3.0-beta.1"));
    assert_eq!(check(&stable, &fast(&stable), "0.2.0", &files).unwrap(), Check::UpToDate);
}

#[test]
fn an_archive_without_the_launcher_or_with_a_slip_is_refused() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    let files = Files::new(&paths);
    for (version, entries) in
        [("0.2.0", vec![("readme.txt", b"x".as_slice())]), ("0.3.0", vec![("../castanea-launcher", b"x".as_slice())])]
    {
        let zip = dir.path().join("bad.zip");
        write_zip(&zip, &entries);
        let bytes = fs::read(zip).unwrap();
        server.publish_as("launcher", Sig::Valid, version, "stable", platform().as_str(), "", bytes);
    }
    let api = api(&server, false);
    let first = found(&api, "0.1.0", &files);
    assert_eq!(first.version, "0.3.0");
    assert!(do_stage(&api, &first, &paths).unwrap_err().to_string().contains("unsafe path"));
    let second = Release { version: "0.2.0".into(), ..first.clone() };
    let server_list = api.list(&fast(&api), crate::install::Channel::Stable).unwrap();
    let plain = server_list.into_iter().find(|r| r.version == "0.2.0").unwrap();
    assert!(do_stage(&api, &plain, &paths).unwrap_err().to_string().contains("must hold"), "{second:?}");
    assert!(files.staged().is_none() && !files.staged_dir().exists() && !files.unpack_dir().exists());
}

// The install location.

#[test]
fn a_managed_or_read_only_install_does_not_replace_itself() {
    let yes = |_: &Path| true;
    let no = |_: &Path| false;
    let home = Path::new("/home/u/bin/castanea-launcher");
    assert_eq!(classify(home, false, &yes), Install::SelfUpdating);
    assert_eq!(classify(home, false, &no), Install::Managed(Why::ReadOnly));
    assert_eq!(classify(home, true, &yes), Install::Managed(Why::AppImage));
    assert_eq!(
        classify(Path::new("/tmp/.mount_castanXYZ/usr/bin/castanea-launcher"), false, &yes),
        Install::Managed(Why::AppImage)
    );
    for system in
        ["/usr/bin/castanea-launcher", "/nix/store/abc-castanea/bin/castanea-launcher", "/snap/c/1/c", "/app/bin/c"]
    {
        assert_eq!(classify(Path::new(system), false, &yes), Install::Managed(Why::System), "{system}");
    }
    assert_eq!(
        classify(Path::new("C:\\Program Files\\castanea\\castanea-launcher.exe"), false, &yes),
        Install::Managed(Why::System)
    );
    assert_eq!(
        classify(Path::new("C:\\Users\\u\\AppData\\Local\\castanea-launcher.exe"), false, &yes),
        Install::SelfUpdating
    );
}

#[test]
fn a_real_read_only_folder_is_noticed() {
    let dir = tempfile::tempdir().unwrap();
    assert!(dir_writable(dir.path()));
    assert!(!dir_writable(&dir.path().join("missing")));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();
        let root = fs::File::create(dir.path().join("probe")).is_ok();
        assert_eq!(dir_writable(dir.path()), root, "a root user can write anywhere");
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
    }
}

// The state machine: the marker, the rollback and the plans, on temp files only.

fn pending(started: Option<u64>) -> Pending {
    Pending { from: "0.1.0".into(), to: "0.2.0".into(), applied_at: 10, started_at: started }
}

#[test]
fn the_marker_decides_between_first_start_rollback_and_stale() {
    assert_eq!(decide("0.1.0", None, 99), Decision::Nothing);
    assert_eq!(decide("0.2.0", Some(pending(None)), 99), Decision::FirstStart(pending(Some(99))));
    assert_eq!(decide("0.2.0", Some(pending(Some(50))), 99), Decision::RollBack(pending(Some(50))));
    assert_eq!(decide("v0.2", Some(pending(Some(50))), 99), Decision::RollBack(pending(Some(50))), "same version");
    assert_eq!(decide("0.1.0", Some(pending(Some(50))), 99), Decision::Stale, "the old launcher is running");
    assert_eq!(decide("0.3.0", Some(pending(None)), 99), Decision::Stale);
}

fn setup(os: Os) -> (tempfile::TempDir, Paths, Layout, Staged) {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(&dir.path().join("home"));
    let name = if os == Os::Windows { "castanea-launcher.exe" } else { "castanea-launcher" };
    let bin = dir.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join(name), "OLD").unwrap();
    let staged_file = dir.path().join("new-binary");
    fs::write(&staged_file, "NEW").unwrap();
    let staged = Staged {
        version: "0.2.0".into(),
        channel: "stable".into(),
        sha256: "00".repeat(32),
        notes: String::new(),
        binary: staged_file,
    };
    (dir, paths, Layout::new(bin.join(name)), staged)
}

#[test]
fn the_plans_have_the_documented_sequence() {
    let layout = Layout::new("/b/castanea-launcher.exe");
    assert_eq!(layout.old(), Path::new("/b/castanea-launcher.old.exe"));
    assert_eq!(Layout::new("/b/castanea-launcher").old(), Path::new("/b/castanea-launcher.old"));
    let staged = Path::new("/s/new.exe");
    let windows = apply_plan(Os::Windows, &layout, staged);
    let renames: Vec<_> = windows
        .iter()
        .filter_map(|op| match op {
            Op::Rename { from, to } => {
                Some((from.file_name().unwrap().to_str().unwrap(), to.file_name().unwrap().to_str().unwrap()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        renames,
        [
            ("castanea-launcher.exe", "castanea-launcher.old.exe"),
            ("castanea-launcher.new.exe", "castanea-launcher.exe")
        ]
    );
    assert!(windows.iter().all(|op| !matches!(op, Op::ModeLike { .. })));
    let unix = apply_plan(Os::Unix, &Layout::new("/b/castanea-launcher"), Path::new("/s/new"));
    assert!(matches!(unix.last(), Some(Op::Rename { to, .. }) if to == Path::new("/b/castanea-launcher")));
    assert!(unix.iter().any(|op| matches!(op, Op::Copy { to, .. } if to == Path::new("/b/castanea-launcher.old"))));
    assert_eq!(rollback_plan(Os::Windows, &layout).len(), 3);
}

#[test]
fn both_sequences_replace_the_launcher_and_keep_the_old_one() {
    for os in [Os::Unix, Os::Windows] {
        let (_dir, paths, layout, staged) = setup(os);
        let files = Files::new(&paths);
        apply(os, &layout, &files, &staged, "0.1.0").unwrap();
        assert_eq!(fs::read_to_string(&layout.exe).unwrap(), "NEW", "{os:?}");
        assert_eq!(fs::read_to_string(layout.old()).unwrap(), "OLD", "{os:?}");
        assert!(!layout.incoming().exists(), "{os:?}");
        let marker = files.pending().unwrap();
        assert_eq!((marker.from.as_str(), marker.to.as_str(), marker.started_at), ("0.1.0", "0.2.0", None));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if os == Os::Unix {
                assert_eq!(fs::metadata(&layout.exe).unwrap().permissions().mode() & 0o111, 0o111, "executable");
            }
        }
    }
}

#[test]
fn a_failed_step_leaves_the_launcher_as_it_was() {
    for os in [Os::Unix, Os::Windows] {
        let (dir, paths, layout, mut staged) = setup(os);
        let files = Files::new(&paths);
        staged.binary = dir.path().join("does-not-exist");
        assert!(apply(os, &layout, &files, &staged, "0.1.0").is_err(), "{os:?}");
        assert_eq!(fs::read_to_string(&layout.exe).unwrap(), "OLD", "{os:?}: the rename is undone");
        assert!(files.pending().is_none(), "{os:?}: no marker for an update that did not happen");
        assert!(!layout.incoming().exists() && !layout.old().exists(), "{os:?}");
    }
}

#[test]
fn an_update_that_is_not_newer_is_not_applied() {
    let (_dir, paths, layout, staged) = setup(Os::Unix);
    let files = Files::new(&paths);
    assert!(apply(Os::Unix, &layout, &files, &staged, "0.2.0").is_err());
    assert!(apply(Os::Unix, &layout, &files, &staged, "0.9.0").is_err());
    assert_eq!(fs::read_to_string(&layout.exe).unwrap(), "OLD");
}

#[test]
fn a_launcher_that_never_draws_a_frame_is_rolled_back_and_skipped() {
    for os in [Os::Unix, Os::Windows] {
        let (_dir, paths, layout, staged) = setup(os);
        let files = Files::new(&paths);
        apply(os, &layout, &files, &staged, "0.1.0").unwrap();
        // First start of the new launcher: stamped, old kept.
        assert_eq!(startup(os, &paths, &layout, "0.2.0"), Startup::Normal);
        assert!(files.pending().unwrap().started_at.is_some());
        assert!(layout.old().is_file(), "the old one stays until the first frame");
        // It crashed before its first frame; the next start rolls back.
        assert_eq!(
            startup(os, &paths, &layout, "0.2.0"),
            Startup::RolledBack { failed: "0.2.0".into(), restored: "0.1.0".into() },
            "{os:?}"
        );
        assert_eq!(fs::read_to_string(&layout.exe).unwrap(), "OLD", "{os:?}");
        assert!(files.pending().is_none());
        assert!(files.is_skipped("0.2.0"));
        if os == Os::Windows {
            assert_eq!(fs::read_to_string(layout.bad()).unwrap(), "NEW", "the failed exe is set aside");
        }
        // The restored launcher cleans up and never offers 0.2.0 again.
        assert_eq!(startup(os, &paths, &layout, "0.1.0"), Startup::Normal);
        for _ in 0..50 {
            if layout.leftovers().iter().all(|p| !p.exists()) {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(layout.leftovers().iter().all(|p| !p.exists()), "{os:?}: the leftovers are deleted");
    }
}

#[test]
fn a_launcher_that_draws_its_first_frame_keeps_the_update_and_deletes_the_old_one() {
    for os in [Os::Unix, Os::Windows] {
        let (_dir, paths, layout, staged) = setup(os);
        let files = Files::new(&paths);
        apply(os, &layout, &files, &staged, "0.1.0").unwrap();
        assert_eq!(startup(os, &paths, &layout, "0.2.0"), Startup::Normal);
        first_frame(&paths, &layout);
        assert!(files.pending().is_none());
        assert!(!files.is_skipped("0.2.0"));
        for _ in 0..50 {
            if !layout.old().exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(!layout.old().exists(), "{os:?}");
        assert_eq!(fs::read_to_string(&layout.exe).unwrap(), "NEW");
        // Starting again later is an ordinary start.
        assert_eq!(startup(os, &paths, &layout, "0.2.0"), Startup::Normal);
    }
}

#[test]
fn a_rollback_without_a_backup_still_clears_the_marker_and_skips_the_version() {
    let (_dir, paths, layout, staged) = setup(Os::Unix);
    let files = Files::new(&paths);
    apply(Os::Unix, &layout, &files, &staged, "0.1.0").unwrap();
    startup(Os::Unix, &paths, &layout, "0.2.0");
    fs::remove_file(layout.old()).unwrap();
    assert_eq!(startup(Os::Unix, &paths, &layout, "0.2.0"), Startup::NoBackup { failed: "0.2.0".into() });
    assert!(files.pending().is_none() && files.is_skipped("0.2.0"));
}

#[test]
fn a_stale_marker_is_forgotten() {
    let (_dir, paths, layout, _staged) = setup(Os::Unix);
    let files = Files::new(&paths);
    files.set_pending(&pending(Some(5))).unwrap();
    assert_eq!(startup(Os::Unix, &paths, &layout, "0.1.0"), Startup::Normal);
    assert!(files.pending().is_none() && !files.is_skipped("0.2.0"));
    fs::create_dir_all(&files.dir).unwrap();
    fs::write(files.marker(), "{ not json").unwrap();
    assert!(files.pending().is_none(), "a broken marker is no marker");
}

#[test]
fn the_skip_list_keeps_each_version_once() {
    let dir = tempfile::tempdir().unwrap();
    let files = Files::new(&Paths::portable(dir.path()));
    assert!(files.skipped().is_empty());
    files.skip("0.2.0").unwrap();
    files.skip("0.2.0").unwrap();
    files.skip("0.3.0").unwrap();
    assert_eq!(files.skipped(), ["0.2.0", "0.3.0"]);
}

/// The Windows trick on a real Windows: the running test program renames itself and back.
#[cfg(windows)]
#[test]
fn a_running_exe_can_be_renamed_but_not_deleted() {
    let exe = std::env::current_exe().unwrap();
    let layout = Layout::new(&exe);
    let old = layout.old();
    assert!(fs::remove_file(&exe).is_err(), "a running exe cannot be deleted");
    fs::rename(&exe, &old).unwrap();
    let back = fs::rename(&old, &exe);
    assert!(back.is_ok(), "{back:?}");
}
