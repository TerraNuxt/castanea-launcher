//! What an update leaves on disk: only `current` after a swap, nothing staged after a failure, and start-up clean-up.

use std::fs;

use super::*;
use crate::archive::tests::write_zip;

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .map(|e| e.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    names.sort();
    names
}

/// A folder that looks like a whole build of `version`, for the start-up clean-up to find.
fn fake_build(dir: &Path, version: &str) {
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::write(dir.join(local::BINARY), "bin").unwrap();
    fs::write(dir.join("assets/marker.json"), version).unwrap();
    Build { version: version.into(), root: dir.to_path_buf(), binary: dir.join(local::BINARY), ..Build::default() }
        .write()
        .unwrap();
}

#[test]
fn after_an_update_only_current_remains() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(&dir.path().join("home"));
    let api = api(&server, false);
    publish(&server, dir.path(), "0.2.0", "stable", "");
    fetch_and_swap(&api, &paths).unwrap();
    assert_eq!(names(&paths.build()), ["current"]);
    publish(&server, dir.path(), "0.3.0", "stable", "");
    assert_eq!(fetch_and_swap(&api, &paths).unwrap().version, "0.3.0");
    assert_eq!(names(&paths.build()), ["current"], "no previous, no next");
    assert!(names(&paths.downloads()).is_empty(), "no archive or partial file of the installed build");
}

#[test]
fn a_failed_check_leaves_the_installed_build_and_no_staging() {
    let server = ReleaseServer::start();
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(&dir.path().join("home"));
    let api = api(&server, false);
    publish(&server, dir.path(), "0.2.0", "stable", "");
    let old = fetch_and_swap(&api, &paths).unwrap();
    let broken = dir.path().join("broken.zip");
    write_zip(&broken, &[("assets/only.json", b"{}")]);
    server.publish("0.3.0", "stable", platform().as_str(), "", fs::read(&broken).unwrap());
    let error = fetch_and_swap(&api, &paths).unwrap_err().to_string();
    assert!(error.contains("must hold"), "{error}");
    assert_eq!(update::installed(&paths).unwrap(), old, "the installed build is untouched");
    assert_eq!(names(&paths.build()), ["current"], "the staging folder is gone");
    assert!(names(&paths.downloads()).is_empty(), "the archive is gone");
}

#[test]
fn start_up_removes_a_next_folder_left_by_a_stopped_update() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    fake_build(&update::current_dir(&paths), "0.2.0");
    fake_build(&update::next_dir(&paths), "0.3.0");
    update::clean_leftovers(&paths);
    assert_eq!(names(&paths.build()), ["current"]);
    assert_eq!(update::installed(&paths).unwrap().version, "0.2.0");
}

#[test]
fn start_up_removes_a_previous_folder_when_current_is_whole() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    fake_build(&update::current_dir(&paths), "0.3.0");
    fake_build(&update::previous_dir(&paths), "0.2.0");
    update::clean_leftovers(&paths);
    assert_eq!(names(&paths.build()), ["current"]);
    assert_eq!(update::installed(&paths).unwrap().version, "0.3.0");
}

#[test]
fn start_up_puts_back_a_previous_build_when_the_swap_stopped_half_way() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    fake_build(&update::previous_dir(&paths), "0.2.0");
    fake_build(&update::next_dir(&paths), "0.3.0");
    update::clean_leftovers(&paths);
    assert_eq!(names(&paths.build()), ["current"], "the installed build is current again");
    assert_eq!(update::installed(&paths).unwrap().version, "0.2.0");
}

#[test]
fn start_up_removes_a_previous_folder_that_is_no_build_at_all() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    fs::create_dir_all(update::previous_dir(&paths)).unwrap();
    fs::write(update::previous_dir(&paths).join("junk"), "x").unwrap();
    update::clean_leftovers(&paths);
    assert!(names(&paths.build()).is_empty());
    assert!(update::installed(&paths).is_none());
}

#[test]
fn start_up_with_nothing_to_clean_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    fake_build(&update::current_dir(&paths), "0.2.0");
    update::clean_leftovers(&paths);
    update::clean_leftovers(&paths);
    assert_eq!(names(&paths.build()), ["current"]);
}

#[test]
fn a_locked_file_is_retried_quietly_and_then_given_up() {
    let io = std::io::ErrorKind::PermissionDenied;
    let mut tries = 0;
    assert!(
        update::retry_quietly(|| {
            tries += 1;
            Err(io.into())
        })
        .is_err()
    );
    assert_eq!(tries, 6, "one try and five retries");
    let mut tries = 0;
    let result = update::retry_quietly(|| {
        tries += 1;
        if tries < 3 { Err(io.into()) } else { Ok(()) }
    });
    assert!(result.is_ok() && tries == 3, "a lock that clears is not an error");
    let mut tries = 0;
    assert!(
        update::retry_quietly(|| {
            tries += 1;
            Err(std::io::ErrorKind::NotFound.into())
        })
        .is_err()
    );
    assert_eq!(tries, 1, "a missing file is not retried");
}
