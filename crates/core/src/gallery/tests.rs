use std::collections::HashSet;
use std::fs;
use std::path::Path;

use super::thumb::{self, THUMB_EDGE, VIEW_EDGE};
use super::time::{civil_from_days, days_from_civil};
use super::*;

const T0: i64 = 1_700_000_000_000;

fn write_png(path: &Path, width: u32, height: u32) {
    let image =
        image::RgbaImage::from_fn(width, height, |x, y| image::Rgba([(x % 251) as u8, (y % 241) as u8, 90, 255]));
    image.save(path).unwrap();
}

fn game_name(ms: i64) -> String {
    format!("castanea-{ms}.png")
}

fn shot(ms: i64) -> Shot {
    let name = game_name(ms);
    Shot { path: PathBuf::from("/s").join(&name), name, taken_ms: ms, size: 1, modified_ms: ms }
}

#[test]
fn the_time_in_a_game_file_name_is_read_and_nothing_else_is() {
    assert_eq!(taken_from_name("castanea-1700000000000.png"), Some(T0));
    assert_eq!(taken_from_name("castanea-1700000000000.jpg"), Some(T0));
    for bad in [
        "castanea-1.png",
        "castanea-17000000000000.png",
        "castanea-170000000000a.png",
        "castanea-0000000000000.png",
        "castanea-9999999999999.png",
        "holiday.png",
        "castanea-.png",
        "x-1700000000000.png",
    ] {
        assert_eq!(taken_from_name(bad), None, "{bad}");
    }
    assert!(is_image_name("a.PNG") && is_image_name("a.jpeg") && is_image_name("a.JPG"));
    assert!(!is_image_name("a.txt") && !is_image_name("png") && !is_image_name("a.png.part"));
}

#[test]
fn a_scan_lists_images_newest_first_and_prefers_the_time_in_the_name() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("screenshots");
    fs::create_dir_all(folder.join("sub")).unwrap();
    for ms in [T0, T0 + 5000, T0 - 86_400_000] {
        fs::write(folder.join(game_name(ms)), "x").unwrap();
    }
    fs::write(folder.join("holiday.JPG"), "x").unwrap();
    fs::write(folder.join("notes.txt"), "x").unwrap();
    fs::write(folder.join("sub").join(game_name(T0 + 1)), "x").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(folder.join(game_name(T0)), folder.join(game_name(T0 + 9000))).unwrap();
    let scan = scan(&folder);
    let names: Vec<&str> = scan.shots.iter().map(|s| s.name.as_str()).collect();
    // `holiday.JPG` has no time in its name: its file time is now, which is newer than the others.
    assert_eq!(names[0], "holiday.JPG", "{names:?}");
    let rest: Vec<String> = names[1..].iter().map(|n| (*n).to_owned()).collect();
    assert_eq!(rest, [game_name(T0 + 5000), game_name(T0), game_name(T0 - 86_400_000)]);
    assert_eq!(scan.unreadable, 0);
    assert!(scan.shots[2].taken_ms == T0 && scan.shots[2].modified_ms > T0, "the name's time, not the file's");
}

#[test]
fn a_missing_folder_is_an_empty_list() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(scan(&dir.path().join("nope")), Scan::default());
}

#[test]
fn shots_of_one_instant_keep_a_stable_order() {
    let mut shots = vec![shot(T0), shot(T0 + 1), shot(T0)];
    shots[2].name = "castanea-b.png".to_owned();
    shots[0].name = "castanea-a.png".to_owned();
    sort(&mut shots);
    let order: Vec<&str> = shots.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(order, [game_name(T0 + 1).as_str(), "castanea-b.png", "castanea-a.png"]);
}

#[test]
fn days_convert_both_ways() {
    assert_eq!(days_from_civil(1970, 1, 1), 0);
    assert_eq!(days_from_civil(2000, 3, 1) - days_from_civil(2000, 2, 28), 2, "2000 is a leap year");
    assert_eq!(days_from_civil(1900, 3, 1) - days_from_civil(1900, 2, 28), 1, "1900 is not");
    assert_eq!(civil_from_days(0), Day { year: 1970, month: 1, day: 1 });
    assert_eq!(time::day_of(T0 / 1000).iso(), "2023-11-14");
    assert_eq!(time::day_of(-1).iso(), "1969-12-31");
    for days in (-40_000..80_000).step_by(37) {
        assert_eq!(civil_from_days(days).days(), days);
    }
    assert!(Day { year: 2026, month: 11, day: 19 } > Day { year: 2026, month: 2, day: 28 });
}

#[test]
fn shots_group_by_local_day_and_the_zone_moves_the_edge() {
    // 2023-11-14 23:30 UTC, 2023-11-15 00:30 UTC and 2023-11-14 01:00 UTC, newest first.
    let utc_midnight = days_from_civil(2023, 11, 15) * 86_400_000;
    let mut shots =
        vec![shot(utc_midnight - 1_800_000), shot(utc_midnight + 1_800_000), shot(utc_midnight - 82_800_000)];
    sort(&mut shots);
    let utc = group_by_day(&shots, |_| 0);
    assert_eq!(utc.len(), 2);
    assert_eq!((utc[0].day.iso(), utc[0].range.clone()), ("2023-11-15".to_owned(), 0..1));
    assert_eq!((utc[1].day.iso(), utc[1].range.clone()), ("2023-11-14".to_owned(), 1..3));
    let berlin = group_by_day(&shots, |_| 3600);
    assert_eq!(berlin.len(), 2);
    assert_eq!(
        (berlin[0].day.iso(), berlin[0].range.clone()),
        ("2023-11-15".to_owned(), 0..2),
        "23:30 UTC is 00:30 there"
    );
    assert_eq!((berlin[1].day.iso(), berlin[1].range.clone()), ("2023-11-14".to_owned(), 2..3));
    let ranges: Vec<_> = utc.iter().map(|g| g.range.clone()).collect();
    assert_eq!(ranges.iter().map(|r| r.len()).sum::<usize>(), shots.len(), "every shot is in exactly one group");
    assert!(group_by_day(&[], |_| 0).is_empty());
}

#[test]
fn the_local_offset_is_a_sane_number() {
    let offset = time::local_offset(T0 / 1000);
    assert!((-14 * 3600..=14 * 3600).contains(&offset), "{offset}");
}

#[test]
fn the_picture_of_a_session_is_its_newest_shot() {
    let shots = vec![shot(T0 + 9000), shot(T0 + 5000), shot(T0 + 1000), shot(T0 - 1)];
    assert_eq!(newest_between(&shots, T0, T0 + 6000).map(|s| s.taken_ms), Some(T0 + 5000));
    assert_eq!(newest_between(&shots, T0, T0 + 1000).map(|s| s.taken_ms), Some(T0 + 1000), "both ends count");
    assert_eq!(newest_between(&shots, T0 + 10_000, T0 + 20_000), None);
    assert_eq!(newest_between(&[], 0, i64::MAX), None);
}

#[test]
fn a_folder_of_thousands_of_files_is_listed_and_grouped_quickly() {
    let dir = tempfile::tempdir().unwrap();
    for n in 0..4000 {
        fs::write(dir.path().join(game_name(T0 + n * 600_000)), "").unwrap();
    }
    let start = std::time::Instant::now();
    let scan = scan(dir.path());
    let groups = group_by_day(&scan.shots, |_| 0);
    assert_eq!(scan.shots.len(), 4000);
    assert_eq!(groups.iter().map(|g| g.range.len()).sum::<usize>(), 4000);
    assert!(scan.shots[0].taken_ms > scan.shots[3999].taken_ms);
    assert!(start.elapsed() < std::time::Duration::from_secs(5), "{:?}", start.elapsed());
}

fn real(dir: &Path, name: &str, width: u32, height: u32) -> Shot {
    let path = dir.join(name);
    write_png(&path, width, height);
    let scan = scan(dir);
    scan.shots.into_iter().find(|s| s.name == name).unwrap()
}

#[test]
fn a_thumbnail_is_scaled_cached_and_read_from_the_cache_next_time() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let shot = real(dir.path(), "castanea-1700000000000.png", 1600, 900);
    assert!(!thumb::is_cached(&cache, &shot));
    let first = thumb::load_thumbnail(&cache, &shot).unwrap();
    assert_eq!((first.width, first.height), (THUMB_EDGE, 216), "scaled down, same proportions");
    assert_eq!(first.rgba.len(), (first.width * first.height * 4) as usize);
    assert!(thumb::is_cached(&cache, &shot));
    let cached = fs::read_dir(&cache).unwrap().flatten().map(|e| e.metadata().unwrap().len()).collect::<Vec<_>>();
    assert_eq!(cached.len(), 1);
    assert!(cached[0] < 60_000, "a small JPEG, {} bytes", cached[0]);
    fs::remove_file(&shot.path).unwrap();
    let again = thumb::load_thumbnail(&cache, &shot).unwrap();
    assert_eq!((again.width, again.height), (first.width, first.height), "the source is not read again");
    assert_eq!(thumb::read_cached(&cache, &shot).unwrap().width, THUMB_EDGE);
}

#[test]
fn a_small_picture_is_not_scaled_up_and_a_tall_one_fits_the_edge() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let small = real(dir.path(), "small.png", 100, 50);
    let tall = real(dir.path(), "tall.png", 500, 1000);
    let a = thumb::load_thumbnail(&cache, &small).unwrap();
    assert_eq!((a.width, a.height), (100, 50));
    let b = thumb::load_thumbnail(&cache, &tall).unwrap();
    assert_eq!((b.width, b.height), (192, THUMB_EDGE));
}

#[test]
fn a_changed_or_renamed_file_gets_a_new_thumbnail_key() {
    let dir = tempfile::tempdir().unwrap();
    let a = real(dir.path(), "a.png", 64, 64);
    let mut changed = a.clone();
    changed.modified_ms += 1;
    let mut bigger = a.clone();
    bigger.size += 1;
    let mut renamed = a.clone();
    renamed.path = dir.path().join("b.png");
    let keys: HashSet<String> =
        [&a, &changed, &bigger, &renamed].iter().map(|s| thumb::cache_key(s, THUMB_EDGE)).collect();
    assert_eq!(keys.len(), 4);
    assert_ne!(thumb::cache_key(&a, THUMB_EDGE), thumb::cache_key(&a, 96));
}

#[test]
fn damaged_empty_and_oversized_files_fail_cleanly_without_a_cache_entry() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    fs::write(dir.path().join("text.png"), "this is not a picture").unwrap();
    fs::write(dir.path().join("empty.png"), "").unwrap();
    write_png(&dir.path().join("whole.png"), 64, 64);
    let whole = fs::read(dir.path().join("whole.png")).unwrap();
    fs::write(dir.path().join("cut.png"), &whole[..whole.len() / 2]).unwrap();
    write_png(&dir.path().join("wide.png"), 13_000, 1);
    let scan = scan(dir.path());
    for name in ["text.png", "empty.png", "cut.png", "wide.png"] {
        let shot = scan.shots.iter().find(|s| s.name == name).unwrap();
        let error = thumb::load_thumbnail(&cache, shot).unwrap_err().to_string();
        assert!(error.contains(name) || error.contains("empty"), "{name}: {error}");
        assert!(!thumb::is_cached(&cache, shot), "{name}");
        assert!(thumb::load_view(&shot.path).is_err(), "{name}");
    }
    let whole = scan.shots.iter().find(|s| s.name == "whole.png").unwrap();
    assert!(thumb::load_thumbnail(&cache, whole).is_ok(), "the good neighbour still loads");
}

#[test]
fn the_large_view_is_scaled_to_its_edge_and_the_clipboard_copy_is_not() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.png");
    write_png(&path, 3200, 1000);
    let view = thumb::load_view(&path).unwrap();
    assert_eq!((view.width, view.height), (VIEW_EDGE, 800));
    let full = thumb::load_full(&path).unwrap();
    assert_eq!((full.width, full.height), (3200, 1000));
    assert_eq!(full.rgba.len(), 3200 * 1000 * 4);
}

#[test]
fn pruning_drops_thumbnails_of_files_that_are_gone_and_the_oldest_over_the_limit() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    fs::create_dir_all(&cache).unwrap();
    for (key, age) in [("live_old", 300), ("live_mid", 200), ("live_new", 100), ("gone", 50)] {
        let file = cache.join(format!("{key}.jpg"));
        fs::write(&file, vec![0u8; 1000]).unwrap();
        let time = std::time::SystemTime::now() - std::time::Duration::from_secs(age);
        fs::File::options().write(true).open(&file).unwrap().set_modified(time).unwrap();
    }
    fs::write(cache.join("note.txt"), "keep me").unwrap();
    let live: HashSet<String> = ["live_old", "live_mid", "live_new"].map(str::to_owned).into();
    assert_eq!(thumb::prune_cache(&cache, &live, 10_000), 1, "only the one of a gone file");
    assert!(!cache.join("gone.jpg").exists() && cache.join("live_old.jpg").exists());
    assert_eq!(thumb::prune_cache(&cache, &live, 2000), 1, "over the limit: the oldest goes");
    assert!(!cache.join("live_old.jpg").exists() && cache.join("live_mid.jpg").exists());
    assert_eq!(thumb::prune_cache(&cache, &live, 0), 2);
    assert!(cache.join("note.txt").exists(), "only thumbnails are touched");
    assert_eq!(thumb::prune_cache(&dir.path().join("none"), &live, 0), 0);
}

#[test]
fn rename_keeps_the_extension_and_refuses_what_could_hurt() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path();
    let a = folder.join("castanea-1700000000000.png");
    fs::write(&a, "x").unwrap();
    fs::write(folder.join("taken.png"), "x").unwrap();
    for bad in ["", "  ", "a/b", "..\\x", "a:b", ".hidden", "end.", "CON", "nul.txt", &"x".repeat(101), "a\u{7}b"] {
        assert!(files::rename(folder, &a, bad).is_err(), "{bad:?}");
    }
    assert!(files::rename(folder, &a, "taken").is_err(), "an existing file is never replaced");
    assert!(a.exists());
    let renamed = files::rename(folder, &a, "  Lake at dawn ").unwrap();
    assert_eq!(renamed, folder.join("Lake at dawn.png"));
    assert!(renamed.exists() && !a.exists());
    let upper = files::rename(folder, &renamed, "LAKE AT DAWN").unwrap();
    assert!(upper.exists(), "a change of case only is allowed");
    let elsewhere = tempfile::tempdir().unwrap();
    let outside = elsewhere.path().join("x.png");
    fs::write(&outside, "x").unwrap();
    assert!(files::rename(folder, &outside, "y").is_err(), "a file outside the folder");
    assert!(files::delete(folder, &outside).is_err() && outside.exists());
    fs::write(folder.join("readme.txt"), "x").unwrap();
    assert!(files::delete(folder, &folder.join("readme.txt")).is_err(), "not an image");
    assert!(files::delete(folder, &folder.join("..").join("x.png")).is_err(), "no way out by `..`");
}

#[test]
fn delete_removes_one_file_and_only_one() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a.png"), dir.path().join("b.png"));
    fs::write(&a, "x").unwrap();
    fs::write(&b, "x").unwrap();
    files::delete(dir.path(), &a).unwrap();
    assert!(!a.exists() && b.exists());
    assert!(files::delete(dir.path(), &a).is_err(), "already gone");
}

#[test]
fn a_trashed_file_lands_in_files_with_an_info_file_and_names_do_not_clash() {
    let dir = tempfile::tempdir().unwrap();
    let trash_dir = dir.path().join("Trash");
    let folder = dir.path().join("my shots");
    fs::create_dir_all(&folder).unwrap();
    let file = folder.join("Jürgen.png");
    fs::write(&file, "one").unwrap();
    let target = trash::trash_into(&trash_dir, &file, 1_700_000_000).unwrap();
    assert_eq!(target, trash_dir.join("files/Jürgen.png"));
    assert!(!file.exists());
    let info = fs::read_to_string(trash_dir.join("info/Jürgen.png.trashinfo")).unwrap();
    let original = fs::canonicalize(&folder).unwrap().join("Jürgen.png");
    let encoded = trash::encode_path(&original.to_string_lossy());
    assert_eq!(info, format!("[Trash Info]\nPath={encoded}\nDeletionDate=2023-11-14T22:13:20\n"));
    assert!(info.contains("my%20shots") && info.contains("J%C3%BCrgen"), "{info}");
    fs::write(&file, "two").unwrap();
    let second = trash::trash_into(&trash_dir, &file, 1_700_000_001).unwrap();
    assert_eq!(second, trash_dir.join("files/Jürgen.2.png"));
    assert_eq!(fs::read_to_string(&target).unwrap(), "one");
    assert_eq!(fs::read_to_string(&second).unwrap(), "two");
    assert!(trash::trash_into(&trash_dir, &folder.join("missing.png"), 0).is_err());
    assert_eq!(fs::read_dir(trash_dir.join("info")).unwrap().count(), 2, "a failed trash leaves no info file");
}
