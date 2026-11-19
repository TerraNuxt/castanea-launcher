//! The screenshot gallery: the grid fills in the background, only the rows in view cost anything, the large view steps and
//! closes from the keyboard, files that appear or cannot be read are handled, and the actions work.

use std::fs;
use std::time::Duration;

use eframe::egui::{self, Key, Modifiers};

use super::Harness;
use crate::app::{Confirm, Page};
use crate::gallery::TEXTURE_CAP;
use crate::probe;

fn press(h: &mut Harness, key: Key) {
    h.frame_with(vec![egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }]);
}

/// Draws until the scan is in and the page's pictures are decoded.
fn settle(h: &mut Harness) {
    for _ in 0..600 {
        h.frame();
        if h.app.jobs.running == 0 && h.app.gallery.scanned && h.gallery_loaded() {
            h.draw();
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the gallery never settled");
}

fn names(h: &Harness) -> Vec<String> {
    h.app.gallery.shots.iter().map(|s| s.name.clone()).collect()
}

#[test]
fn an_empty_folder_says_how_to_take_a_screenshot_in_both_languages() {
    for language in ["en", "de"] {
        let mut h = Harness::new(language);
        fs::remove_dir_all(h.app.paths.screenshots()).unwrap();
        h.app.gallery.stale = true;
        h.go(Page::Screenshots);
        settle(&mut h);
        assert!(h.app.gallery.shots.is_empty());
        assert!(probe::take(&h.ctx).iter().any(|(n, _)| n == "shots:empty"), "{language}: the empty text is drawn");
        assert!(h.app.tr.get("shots.empty").contains("F2"), "{language}");
    }
}

#[test]
fn screenshots_are_listed_newest_first_in_day_groups_with_thumbnails() {
    let mut h = Harness::new("en");
    h.go(Page::Screenshots);
    settle(&mut h);
    let list = names(&h);
    assert_eq!(list.len(), 3);
    let times: Vec<i64> = h.app.gallery.shots.iter().map(|s| s.taken_ms).collect();
    assert!(times.windows(2).all(|w| w[0] > w[1]), "newest first: {times:?}");
    assert!(!h.app.gallery.groups.is_empty());
    let covered: usize = h.app.gallery.groups.iter().map(|g| g.range.len()).sum();
    assert_eq!(covered, 3, "every shot is in a group");
    assert!(probe::take(&h.ctx).iter().any(|(n, _)| n == "shots:grid"));
    for shot in &h.app.gallery.shots.clone() {
        assert!(h.app.gallery.thumbs.get(&shot.path).is_some(), "{} has a thumbnail", shot.name);
    }
    assert!(h.app.paths.thumbnails().is_dir(), "thumbnails are cached on disk");
}

#[test]
fn a_big_folder_decodes_only_what_is_in_view_and_keeps_memory_bounded() {
    let mut h = Harness::new("en");
    // 3000 files that are not pictures: every one that is tried fails, so the failures count what was tried.
    let dir = h.app.paths.screenshots();
    let base = 1_700_000_000_000i64;
    for n in 0..3000 {
        fs::write(dir.join(format!("castanea-{}.png", base + n * 1000)), b"not a picture").unwrap();
    }
    h.app.gallery.stale = true;
    h.go(Page::Screenshots);
    // Only the first screenful is tried, so wait until the number of tried files stops growing.
    let mut last = (usize::MAX, 0);
    for _ in 0..2000 {
        h.frame();
        let tried = h.app.gallery.failed.len() + h.app.gallery.thumbs.len();
        last = (tried, if tried == last.0 { last.1 + 1 } else { 0 });
        if h.app.gallery.scanned && h.app.jobs.running == 0 && tried > 0 && last.1 >= 30 {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(h.app.gallery.shots.len(), 3003);
    let start = std::time::Instant::now();
    for _ in 0..20 {
        h.frame();
    }
    assert!(start.elapsed() < Duration::from_secs(10), "{:?}", start.elapsed());
    let tried = h.app.gallery.failed.len() + h.app.gallery.thumbs.len();
    assert!(tried < 100, "{tried} of 3003 were decoded for one screenful");
    assert!(h.app.gallery.thumbs.len() <= TEXTURE_CAP);
    let layout = h.app.gallery.layout.clone().unwrap();
    assert!(layout.rows.len() > 500, "{} rows", layout.rows.len());
    assert!(layout.total > 10_000.0);
}

#[test]
fn a_damaged_file_gets_a_placeholder_and_its_neighbours_still_load() {
    let mut h = Harness::new("de");
    let dir = h.app.paths.screenshots();
    let broken = dir.join("castanea-9999999999999.png");
    fs::write(&broken, b"broken").unwrap();
    h.app.gallery.stale = true;
    h.go(Page::Screenshots);
    settle(&mut h);
    let good = h.app.gallery.shots.iter().find(|s| s.name != "castanea-9999999999999.png").unwrap().path.clone();
    let broken_shot = h.app.gallery.shots.iter().find(|s| s.name == "castanea-9999999999999.png");
    // Its name carries no valid time, so it sorts by the file's time; it is listed either way.
    assert!(broken_shot.is_some());
    assert!(h.app.gallery.failed.contains(&broken), "the damaged file is marked");
    assert!(h.app.gallery.thumbs.get(&good).is_some(), "a good neighbour has its thumbnail");
}

#[test]
fn a_file_that_appears_while_the_page_is_open_shows_up_after_a_refresh() {
    let mut h = Harness::new("en");
    h.go(Page::Screenshots);
    settle(&mut h);
    assert_eq!(h.app.gallery.shots.len(), 3);
    let name = "castanea-2000000000000.png";
    let image = image::RgbaImage::from_pixel(32, 18, image::Rgba([200, 50, 50, 255]));
    image.save(h.app.paths.screenshots().join(name)).unwrap();
    assert_eq!(h.app.gallery.shots.len(), 3, "nothing changes by itself");
    h.app.gallery.stale = true;
    settle(&mut h);
    assert_eq!(h.app.gallery.shots.len(), 4);
    assert!(names(&h).contains(&name.to_owned()));
}

#[test]
fn the_window_getting_the_focus_back_scans_again() {
    let mut h = Harness::new("en");
    h.go(Page::Screenshots);
    settle(&mut h);
    h.app.gallery.was_focused = false;
    fs::remove_file(&h.app.gallery.shots[0].path).unwrap();
    for _ in 0..3 {
        h.frame();
    }
    settle(&mut h);
    assert_eq!(h.app.gallery.shots.len(), 2, "the file that went away is gone from the list");
}

#[test]
fn the_large_view_steps_with_the_arrows_and_closes_with_escape() {
    let mut h = Harness::new("en");
    h.go(Page::Screenshots);
    settle(&mut h);
    let paths: Vec<_> = h.app.gallery.shots.iter().map(|s| s.path.clone()).collect();
    h.app.open_viewer(paths[0].clone());
    settle(&mut h);
    assert_eq!(h.app.gallery.viewer.as_ref(), Some(&paths[0]));
    assert!(h.app.gallery.view.as_ref().is_some_and(|(p, _)| *p == paths[0]), "the full picture is loaded");
    press(&mut h, Key::ArrowLeft);
    assert_eq!(h.app.gallery.viewer.as_ref(), Some(&paths[0]), "no newer picture than the newest");
    press(&mut h, Key::ArrowRight);
    assert_eq!(h.app.gallery.viewer.as_ref(), Some(&paths[1]));
    press(&mut h, Key::End);
    assert_eq!(h.app.gallery.viewer.as_ref(), Some(&paths[2]));
    press(&mut h, Key::ArrowRight);
    assert_eq!(h.app.gallery.viewer.as_ref(), Some(&paths[2]), "no older picture than the oldest");
    press(&mut h, Key::Home);
    assert_eq!(h.app.gallery.viewer.as_ref(), Some(&paths[0]));
    press(&mut h, Key::Escape);
    assert!(h.app.gallery.viewer.is_none(), "Esc leaves the large view");
    assert_eq!(h.app.page, Page::Screenshots);
}

#[test]
fn escape_closes_a_question_before_the_large_view_and_a_rename_before_both() {
    let mut h = Harness::new("en");
    h.go(Page::Screenshots);
    settle(&mut h);
    let path = h.app.gallery.shots[0].path.clone();
    h.app.open_viewer(path.clone());
    settle(&mut h);
    h.app.gallery.rename = Some("x".into());
    press(&mut h, Key::Escape);
    assert!(h.app.gallery.rename.is_none() && h.app.gallery.viewer.is_some());
    h.app.ask_delete_shot(path);
    h.draw();
    press(&mut h, Key::ArrowRight);
    assert_eq!(
        h.app.gallery.viewer.as_ref().map(|p| p == &h.app.gallery.shots[0].path),
        Some(true),
        "no stepping behind a question"
    );
    press(&mut h, Key::Escape);
    h.draw();
    assert!(h.app.confirm.is_none() && h.app.gallery.viewer.is_some(), "Esc answered the question only");
}

#[test]
fn the_delete_question_names_the_file_and_never_answers_to_enter() {
    for language in ["en", "de"] {
        let mut h = Harness::new(language);
        h.go(Page::Screenshots);
        settle(&mut h);
        let path = h.app.gallery.shots[0].path.clone();
        h.app.confirm = Some(Confirm::DeleteShot(path.clone()));
        h.draw();
        press(&mut h, Key::Enter);
        assert!(matches!(h.app.confirm, Some(Confirm::DeleteShot(_))), "{language}: Enter does not delete");
        assert!(path.exists());
        h.app.confirm = Some(Confirm::DeleteShotForever { path: path.clone(), reason: "no trash".into() });
        h.draw();
        press(&mut h, Key::Enter);
        assert!(path.exists() && h.app.confirm.is_some(), "{language}");
    }
}

#[test]
fn deleting_for_good_takes_the_file_out_of_the_list_and_the_large_view_moves_on() {
    let mut h = Harness::new("en");
    h.go(Page::Screenshots);
    settle(&mut h);
    let paths: Vec<_> = h.app.gallery.shots.iter().map(|s| s.path.clone()).collect();
    h.app.open_viewer(paths[1].clone());
    h.app.delete_shot_forever(paths[1].clone());
    assert!(!paths[1].exists());
    assert_eq!(h.app.gallery.shots.len(), 2);
    assert_eq!(h.app.gallery.viewer.as_ref(), Some(&paths[2]), "the older neighbour takes its place");
    h.app.delete_shot_forever(paths[2].clone());
    assert_eq!(h.app.gallery.viewer.as_ref(), Some(&paths[0]), "at the end it falls back to the one before");
    h.app.delete_shot_forever(paths[0].clone());
    assert!(h.app.gallery.viewer.is_none() && h.app.gallery.shots.is_empty(), "the last one closes the view");
    h.draw();
    assert!(
        h.app.notes.iter().all(|n| n.kind != crate::app::NoteKind::Error),
        "{:?}",
        h.app.notes.iter().map(|n| &n.text).collect::<Vec<_>>()
    );
}

#[test]
fn the_gallery_refuses_to_delete_a_file_that_is_not_in_the_screenshots_folder() {
    let mut h = Harness::new("en");
    let outside = h._dir.path().join("keep.png");
    fs::write(&outside, "x").unwrap();
    h.app.delete_shot_forever(outside.clone());
    assert!(outside.exists());
    assert!(h.app.notes.iter().any(|n| n.kind == crate::app::NoteKind::Error));
}

#[test]
fn renaming_keeps_the_view_on_the_file_under_its_new_name() {
    let mut h = Harness::new("en");
    h.go(Page::Screenshots);
    settle(&mut h);
    let old = h.app.gallery.shots[0].path.clone();
    h.app.open_viewer(old.clone());
    let ctx = h.ctx.clone();
    h.app.rename_shot(&ctx, &old, "Lake at dawn");
    settle(&mut h);
    let new = h.app.paths.screenshots().join("Lake at dawn.png");
    assert!(new.exists() && !old.exists());
    assert_eq!(h.app.gallery.viewer.as_ref(), Some(&new));
    assert!(h.app.gallery.rename.is_none());
    h.app.rename_shot(&ctx, &new, "bad/name");
    assert!(new.exists(), "a refused name changes nothing");
    assert!(h.app.notes.iter().any(|n| n.kind == crate::app::NoteKind::Error));
}

#[test]
fn copying_decodes_the_real_picture_and_hands_it_to_the_clipboard() {
    let mut h = Harness::new("en");
    h.go(Page::Screenshots);
    settle(&mut h);
    let path = h.app.gallery.shots[0].path.clone();
    let ctx = h.ctx.clone();
    h.app.copy_shot(&ctx, path);
    let mut copied = false;
    for _ in 0..400 {
        let output = h.frame();
        copied |= output
            .platform_output
            .commands
            .iter()
            .any(|c| matches!(c, egui::OutputCommand::CopyImage(image) if image.size == [64, 36]));
        if copied {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(copied, "the clipboard got the 64x36 picture");
}

#[test]
fn the_gallery_pages_draw_at_every_size_in_both_languages() {
    for language in ["en", "de"] {
        let mut h = Harness::new(language);
        for size in super::SIZES {
            h.size = egui::vec2(size[0], size[1]);
            h.go(Page::Screenshots);
            h.app.close_viewer();
            settle(&mut h);
            let shot = h.app.gallery.shots[0].path.clone();
            h.app.open_viewer(shot);
            settle(&mut h);
            let region = probe::take(&h.ctx).into_iter().find(|(n, _)| n == "shots:view").map(|(_, r)| r);
            let region = region.unwrap_or_else(|| panic!("{language} {size:?}: no large view"));
            assert!(region.width() > 300.0 && region.height() > 150.0, "{language} {size:?}: {region:?}");
            assert!(
                region.right() <= size[0] + 0.5 && region.bottom() <= size[1] + 0.5,
                "{language} {size:?}: {region:?}"
            );
            assert!(h.app.notes.iter().all(|n| n.kind != crate::app::NoteKind::Error));
        }
    }
}
