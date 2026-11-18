use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::*;
use crate::paths::Paths;

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// A launcher home with a file in every place the launcher uses, and a folder next to it that is not the launcher's.
struct Home {
    _dir: tempfile::TempDir,
    root: PathBuf,
    outside: PathBuf,
    paths: Paths,
}

fn home() -> Home {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("home");
    let outside = dir.path().join("outside");
    let paths = Paths::portable(&root);
    write(&outside.join("precious.txt"), "keep");
    write(&paths.cache.join("news/en.json"), "[]");
    write(&paths.cache.join("index/a.json"), "{}");
    write(&paths.cache.join("downloads/x.part"), "part");
    write(&paths.backend_file(), "{}");
    write(&paths.data.join("launcher-update/skip.json"), "[]");
    write(&paths.build().join("current/castanea"), "bin");
    write(&paths.build().join("previous/castanea"), "old");
    write(&paths.mods().join("amber/mod.json"), "{}");
    write(&paths.game().join("logs/latest.log"), "log");
    write(&paths.game().join("saves/world1/level.dat"), "world");
    write(&paths.game().join("settings.json"), "{}");
    write(&paths.settings_file(), "{}");
    write(&paths.data.join("window/state.ron"), "()");
    Home { _dir: dir, root, outside, paths }
}

fn scope(h: &Home) -> Scope {
    Scope::new(&h.paths, &[])
}

fn gone(paths: &[PathBuf]) -> bool {
    paths.iter().all(|p| fs::symlink_metadata(p).is_err())
}

#[test]
fn each_row_deletes_only_its_own_paths() {
    let own = |p: &Paths, row: Row| -> Vec<PathBuf> {
        match row {
            Row::Cache => vec![
                p.cache.join("news"),
                p.cache.join("index"),
                p.cache.join("downloads"),
                p.backend_file(),
                p.data.join("launcher-update"),
            ],
            Row::Game => vec![p.build()],
            Row::Mods => vec![p.mods()],
            Row::Logs => vec![p.game().join("logs")],
            Row::Settings => vec![p.settings_file(), p.data.join("window")],
            Row::GameData => vec![p.game().join("saves"), p.game().join("settings.json")],
        }
    };
    for row in Row::ALL {
        let h = home();
        let report = if row == Row::GameData {
            delete_confirmed(&scope(&h), &h.paths, false, "DELETE").unwrap()
        } else {
            delete_row(&scope(&h), &h.paths, row, false).unwrap()
        };
        assert!(report.is_clean(), "{row:?}: {report:?}");
        assert!(gone(&own(&h.paths, row)), "{row:?} left something");
        for other in Row::ALL.into_iter().filter(|r| *r != row) {
            let left = own(&h.paths, other);
            assert!(left.iter().all(|p| p.exists()), "{row:?} removed a path of {other:?}");
        }
        assert!(h.outside.join("precious.txt").exists());
    }
}

#[test]
fn the_game_row_keeps_worlds_and_the_game_data_row_keeps_builds() {
    let h = home();
    delete_row(&scope(&h), &h.paths, Row::Game, false).unwrap();
    assert!(h.paths.game().join("saves/world1/level.dat").exists());
    assert!(!h.paths.build().exists());
    let h = home();
    delete_confirmed(&scope(&h), &h.paths, false, "DELETE").unwrap();
    assert!(h.paths.build().join("current/castanea").exists());
    assert!(h.paths.mods().join("amber/mod.json").exists(), "mods and logs are their own rows");
    assert!(h.paths.game().join("logs/latest.log").exists());
}

#[test]
fn the_game_data_row_lists_only_what_the_game_wrote() {
    let h = home();
    let names: Vec<String> = targets(&h.paths, Row::GameData)
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["saves", "settings.json"]);
    assert!(!Row::OWN.contains(&Row::GameData));
    assert!(knows_game_data(&h.paths));
    let empty = tempfile::tempdir().unwrap();
    assert!(!knows_game_data(&Paths::portable(empty.path())));
}

#[test]
fn the_confirmation_word_must_be_typed_exactly() {
    assert!(is_confirmation("DELETE"));
    assert!(is_confirmation("  DELETE "));
    for wrong in ["", "delete", "Delete", "DELET", "DELETE!", "DELETE DELETE", "yes"] {
        assert!(!is_confirmation(wrong), "{wrong:?}");
    }
    let h = home();
    let refused = delete_confirmed(&scope(&h), &h.paths, false, "delete").unwrap_err();
    assert!(refused.to_string().contains("DELETE"), "{refused}");
    assert!(h.paths.game().join("saves/world1/level.dat").exists(), "nothing was deleted");
}

#[test]
fn delete_everything_removes_the_own_rows_and_keeps_the_game_data() {
    let h = home();
    let report = delete_all(&scope(&h), &h.paths, false).unwrap();
    assert!(report.is_clean(), "{report:?}");
    let p = &h.paths;
    assert!(gone(&[
        p.cache.join("news"),
        p.build(),
        p.mods(),
        p.game().join("logs"),
        p.settings_file(),
        p.backend_file(),
        p.data.join("window")
    ]));
    assert!(p.game().join("saves/world1/level.dat").exists());
    assert!(p.game().join("settings.json").exists());
    assert!(!p.cache.exists() && !p.config.exists(), "empty folders go too");
    assert!(h.outside.join("precious.txt").exists());
}

#[test]
fn nothing_is_deleted_while_the_game_runs() {
    let h = home();
    for row in Row::ALL {
        let error = delete_row(&scope(&h), &h.paths, row, true).unwrap_err();
        assert!(error.to_string().contains("running"), "{error}");
    }
    assert!(delete_all(&scope(&h), &h.paths, true).is_err());
    assert!(delete_confirmed(&scope(&h), &h.paths, true, "DELETE").is_err());
    assert!(h.paths.build().exists() && h.paths.settings_file().exists() && h.paths.game().join("saves").exists());
}

#[test]
fn a_path_outside_the_launcher_folders_is_refused() {
    let h = home();
    let scope = scope(&h);
    let file = h.outside.join("precious.txt");
    assert!(scope.check(&file).is_err());
    assert!(scope.check(&h.outside).is_err());
    assert!(scope.check(&h.root).is_err(), "the folder holding the launcher's folders");
    assert!(scope.check(&h.paths.data).is_err(), "a root itself");
    assert!(scope.check(&h.paths.data.join("../../outside/precious.txt")).is_err(), "dot dot");
    assert!(scope.check(Path::new("/")).is_err());
    assert!(scope.check(&h.paths.build()).is_ok());
    assert!(file.exists());
}

#[cfg(unix)]
#[test]
fn links_inside_the_launcher_folders_are_removed_and_never_followed() {
    use std::os::unix::fs::symlink;
    let h = home();
    let p = &h.paths;
    // The mods folder and one cache entry are links out of the launcher's folders; so is a file in the build.
    fs::remove_dir_all(p.mods()).unwrap();
    symlink(&h.outside, p.mods()).unwrap();
    symlink(&h.outside, p.cache.join("news-link")).unwrap();
    symlink(h.outside.join("precious.txt"), p.build().join("current/link-to-file")).unwrap();
    for row in [Row::Mods, Row::Cache, Row::Game] {
        let report = delete_row(&scope(&h), p, row, false).unwrap();
        assert!(report.is_clean(), "{row:?}: {report:?}");
    }
    assert!(h.outside.join("precious.txt").exists(), "the target of a link survived");
    assert!(fs::symlink_metadata(p.mods()).is_err(), "the link itself is gone");
}

#[cfg(unix)]
#[test]
fn a_link_in_the_way_that_leads_out_is_refused() {
    use std::os::unix::fs::symlink;
    let h = home();
    let p = &h.paths;
    // `game` itself points outside: `game/mods` and `game/logs` would resolve into the outside folder.
    write(&h.outside.join("mods/theirs/mod.json"), "{}");
    write(&h.outside.join("logs/theirs.log"), "x");
    fs::remove_dir_all(p.game()).unwrap();
    symlink(&h.outside, p.game()).unwrap();
    let report = delete_row(&scope(&h), p, Row::Mods, false).unwrap();
    assert_eq!(report.failures.len(), 1, "{report:?}");
    assert!(report.failures[0].error.contains("outside"), "{report:?}");
    let report = delete_row(&scope(&h), p, Row::Logs, false).unwrap();
    assert_eq!(report.failures.len(), 1);
    assert!(h.outside.join("mods/theirs/mod.json").exists());
    assert!(h.outside.join("logs/theirs.log").exists());
}

#[test]
fn a_local_game_folder_added_by_path_is_never_deleted() {
    let h = home();
    let local = h.paths.data.join("build/current/my-checkout");
    write(&local.join("castanea"), "mine");
    let keep = vec![local.clone()];
    let report = delete_row(&Scope::new(&h.paths, &keep), &h.paths, Row::Game, false).unwrap();
    assert_eq!(report.failures.len(), 1, "{report:?}");
    assert!(local.join("castanea").exists());
    // A folder elsewhere is never a target at all; only the reference in launcher.json goes with the Settings row.
    let reference = h.outside.join("checkout");
    write(&reference.join("castanea"), "mine");
    let settings = crate::settings::Settings { dev_build: Some(reference.clone()), ..Default::default() };
    settings.save(&h.paths).unwrap();
    delete_all(&Scope::new(&h.paths, std::slice::from_ref(&reference)), &h.paths, false).unwrap();
    assert!(reference.join("castanea").exists());
    assert!(!h.paths.settings_file().exists());
}

#[test]
fn a_locked_item_is_reported_and_the_rest_still_goes() {
    let h = home();
    let log = h.paths.game().join("logs/latest.log");
    write(&h.paths.game().join("logs/older.log"), "x");
    super::remove::LOCKED.with(|l| l.borrow_mut().push(log.clone()));
    let report = delete_all(&scope(&h), &h.paths, false).unwrap();
    assert!(!report.is_clean());
    let failed: Vec<&Path> = report.failures.iter().map(|f| f.path.as_path()).collect();
    assert_eq!(
        failed,
        [log.as_path(), h.paths.game().join("logs").as_path()],
        "the file and the folder it keeps: {report:?}"
    );
    assert!(report.failures.iter().all(|f| !f.error.is_empty()));
    assert!(!h.paths.game().join("logs/older.log").exists(), "its neighbour went");
    assert!(
        !h.paths.build().exists() && !h.paths.mods().exists() && !h.paths.settings_file().exists(),
        "the other rows went"
    );
    // Once the lock is gone, a retry of that row finishes it.
    super::remove::LOCKED.with(|l| l.borrow_mut().clear());
    let retry = delete_row(&scope(&h), &h.paths, Row::Logs, false).unwrap();
    assert!(retry.is_clean(), "{retry:?}");
    assert!(!log.exists());
}

#[test]
fn sizes_add_up_and_the_walk_stops_when_asked() {
    let h = home();
    let stop = AtomicBool::new(false);
    let mods = measure(&h.paths, Row::Mods, &stop).unwrap();
    assert_eq!(mods, Size { bytes: 2, files: 1 });
    let cache = measure(&h.paths, Row::Cache, &stop).unwrap();
    assert_eq!(cache.files, 5);
    assert_eq!(measure(&h.paths, Row::Game, &stop).unwrap().bytes, 6);
    stop.store(true, Ordering::Relaxed);
    assert_eq!(measure(&h.paths, Row::Cache, &stop), None);
    let nothing = tempfile::tempdir().unwrap();
    assert_eq!(measure(&Paths::portable(nothing.path()), Row::Game, &AtomicBool::new(false)), Some(Size::default()));
}

#[cfg(unix)]
#[test]
fn a_link_adds_nothing_to_the_size() {
    let h = home();
    write(&h.outside.join("big.bin"), &"x".repeat(5000));
    std::os::unix::fs::symlink(&h.outside, h.paths.mods().join("out")).unwrap();
    assert_eq!(measure(&h.paths, Row::Mods, &AtomicBool::new(false)).unwrap().files, 1);
}

fn exe_in(dir: &Path) -> PathBuf {
    let exe = dir.join("castanea-launcher.exe");
    write(&exe, "exe");
    exe
}

#[test]
fn the_uninstall_plan_for_an_own_folder_lists_everything_it_removes() {
    let h = home();
    let install = h._dir.path().join("Apps/castanea-launcher");
    let exe = exe_in(&install);
    write(&install.join("LICENSE"), "mit");
    write(&install.join("castanea-launcher.old.exe"), "old");
    write(&install.join("notes.txt"), "not ours");
    let menu = h._dir.path().join("Start Menu");
    write(&menu.join("Castanea Launcher.lnk"), "lnk");
    write(&menu.join("Other.lnk"), "lnk");
    let list = uninstall_plan(
        &h.paths,
        &exe,
        &[h._dir.path().join("Downloads")],
        &[menu.clone(), h._dir.path().join("missing")],
    );
    assert_eq!(list.dir, InstallDir::Owned(install.clone()));
    assert_eq!(list.exe, exe);
    assert_eq!(list.leftovers, [install.join("castanea-launcher.old.exe")]);
    assert_eq!(list.shipped, [install.join("LICENSE")]);
    assert_eq!(list.shortcuts, [menu.join("Castanea Launcher.lnk")]);
    assert!(list.registry.is_empty());
    let rows: Vec<Row> = list.rows.iter().map(|(r, _)| *r).collect();
    assert_eq!(rows, Row::OWN);
    assert!(list.rows.iter().all(|(_, found)| !found.is_empty()), "every row has something here");
    assert!(!list.rows.iter().flat_map(|(_, f)| f).any(|f| f.starts_with(h.paths.game().join("saves"))));
}

#[test]
fn an_exe_in_a_shared_folder_is_the_only_thing_removed_from_it() {
    let h = home();
    let downloads = h._dir.path().join("Downloads");
    let exe = exe_in(&downloads);
    write(&downloads.join("LICENSE"), "someone else's");
    write(&downloads.join("castanea-launcher.old.exe"), "old");
    let list = uninstall_plan(&h.paths, &exe, std::slice::from_ref(&downloads), &[]);
    assert_eq!(list.dir, InstallDir::Shared(downloads.clone()));
    assert!(list.shipped.is_empty(), "a LICENSE in a shared folder is not ours");
    assert_eq!(list.leftovers.len(), 1);
    // A folder with another name is shared too, even when nothing lists it.
    let other = h._dir.path().join("Games");
    let list = uninstall_plan(&h.paths, &exe_in(&other), &[], &[]);
    assert!(matches!(list.dir, InstallDir::Shared(_)));
    // And a folder named for the launcher that is on the shared list is shared.
    let named = h._dir.path().join("castanea-launcher");
    let list = uninstall_plan(&h.paths, &exe_in(&named), std::slice::from_ref(&named), &[]);
    assert!(matches!(list.dir, InstallDir::Shared(_)));
}

#[test]
fn finishing_an_uninstall_removes_the_plan_and_nothing_else() {
    let h = home();
    let install = h._dir.path().join("castanea-launcher");
    let exe = exe_in(&install);
    write(&install.join("LICENSE"), "mit");
    write(&install.join("castanea-launcher.old.exe"), "old");
    write(&install.join("notes.txt"), "not ours");
    let list = uninstall_plan(&h.paths, &exe, &[], &[]);
    let report = finish_steps(&list, &h.paths, &scope(&h), 0, Duration::ZERO);
    assert!(report.is_clean(), "{report:?}");
    assert!(gone(&[
        exe,
        install.join("LICENSE"),
        install.join("castanea-launcher.old.exe"),
        h.paths.build(),
        h.paths.settings_file()
    ]));
    assert!(install.join("notes.txt").exists(), "the folder stays while it holds something else");
    assert!(h.paths.game().join("saves/world1/level.dat").exists(), "worlds stay");
    assert!(h.outside.join("precious.txt").exists());
    // With nothing else inside, the folder goes too; running it twice is fine.
    fs::remove_file(install.join("notes.txt")).unwrap();
    let again = finish_steps(&list, &h.paths, &scope(&h), 0, Duration::ZERO);
    assert!(again.is_clean(), "{again:?}");
    assert!(!install.exists());
}

#[test]
fn a_shared_folder_stays_after_the_uninstall() {
    let h = home();
    let downloads = h._dir.path().join("Downloads");
    let exe = exe_in(&downloads);
    let list = uninstall_plan(&h.paths, &exe, std::slice::from_ref(&downloads), &[]);
    finish_steps(&list, &h.paths, &scope(&h), 0, Duration::ZERO);
    assert!(!exe.exists());
    assert!(downloads.is_dir());
}
