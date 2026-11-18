//! The news column of the home page: its place beside or below the Play block at every size, with and without news,
//! hostile and huge data, the saved feed offline, and Play never waiting for the network.

use std::fs;

use castanea_launcher_core::news::Item;
use eframe::egui::{self, Rect};

use super::{HOME_SIZES, Harness};
use crate::app::Page;
use crate::game::Update;
use crate::news::{Phase, demo};
use crate::probe;
use crate::theme::layout;

struct Shot(Vec<(String, Rect)>);

impl Shot {
    fn get(&self, name: &str) -> Rect {
        self.find(name)
            .unwrap_or_else(|| panic!("no region {name}: {:?}", self.0.iter().map(|r| &r.0).collect::<Vec<_>>()))
    }

    fn find(&self, name: &str) -> Option<Rect> {
        self.0.iter().find(|(n, _)| n == name).map(|(_, r)| *r)
    }

    fn items(&self) -> Vec<(String, Rect)> {
        self.0.iter().filter(|(n, _)| n.starts_with("home:news:item:")).cloned().collect()
    }
}

fn shoot(h: &mut Harness) -> Shot {
    h.draw();
    Shot(probe::take(&h.ctx))
}

fn long_items(n: usize) -> Vec<Item> {
    let body =
        "Alpha **bold words** and a [link](https://castanea.xenoo.dev/x) then plain text, again and again. ".repeat(14);
    (0..n)
        .map(|i| Item {
            id: format!("long{i}"),
            title: format!("A rather long title that has to wrap onto more than one line, number {i}"),
            body: body.clone(),
            date: "2026-11-01".into(),
            tag: "release".into(),
            url: Some("https://castanea.xenoo.dev/news/x".into()),
        })
        .collect()
}

fn hostile_items() -> Vec<Item> {
    let nasty = [
        "**".repeat(400),
        "[x](".repeat(300),
        "[".repeat(700) + &"](https://a.b)".repeat(10),
        "\u{5d0}\u{5d1}\u{5d2} مرحبا 😀👩‍👩‍👧 e\u{301}\u{301}\u{301} ".repeat(60),
        "a".repeat(1500),
        "\n\n\n\n\n**a**\n\n\n\n".repeat(30),
        "<script>alert(1)</script> <b>x</b> &amp; %s {name} {0} \\u0000".repeat(20),
    ];
    nasty
        .into_iter()
        .enumerate()
        .map(|(i, body)| Item {
            id: format!("n{i}"),
            title: "t".repeat(120),
            body: body.chars().take(1500).collect(),
            date: "d".repeat(32),
            tag: "g".repeat(24),
            url: Some(format!("https://castanea.xenoo.dev/{}", "p".repeat(400))),
        })
        .collect()
}

/// A home page with `items`, release notes if `notes`, and the network off the table.
fn home(language: &str, size: [f32; 2], items: Vec<Item>, notes: bool) -> Harness {
    let mut h = Harness::new(language);
    h.size = egui::vec2(size[0], size[1]);
    h.app.auto_check = false;
    h.app.settings.offline = false;
    h.app.update = Update::UpToDate;
    h.app.news.items = items;
    h.app.news.phase = Phase::Fresh;
    h.app.installed.as_mut().unwrap().notes = if notes {
        "• Smelteries\n• Steam machines\n• Faster loading\n• A very long line that does not fit the narrow column of the home page at all, not even a little".into()
    } else {
        String::new()
    };
    h.go(Page::Home);
    h
}

fn disjoint(context: &str, regions: &[(String, Rect)]) {
    for (i, (a, ra)) in regions.iter().enumerate() {
        for (b, rb) in &regions[i + 1..] {
            let overlap = ra.intersect(*rb);
            assert!(!overlap.is_positive() || overlap.area() < 1.0, "{context}: {a} {ra:?} overlaps {b} {rb:?}");
        }
    }
}

fn check_home(context: &str, shot: &Shot, items: usize) {
    let content = shot.get("content");
    let news = shot.get("home:news");
    let play = shot.get("home:play");
    let text = ["home:wordmark", "home:play", "home:status"].map(|n| (n.to_owned(), shot.get(n)));
    let after_play = shot.find("home:links").unwrap_or_else(|| shot.get("home:status"));
    if content.width() >= layout::NARROW {
        assert!(
            news.left() > play.right() + layout::PAGE_MARGIN / 2.0,
            "{context}: the news sits right of the Play block"
        );
        let scene = shot.get("home:scene");
        assert!(scene.right() <= news.left(), "{context}: the scene ends before the column");
        assert!(
            news.top() >= content.top() - 0.5 && news.bottom() <= content.bottom() + 0.5,
            "{context}: {news:?} vs {content:?}"
        );
        assert!(news.right() <= content.right() + 0.5, "{context}");
        if let Some(notes) = shot.find("home:notes") {
            assert!(notes.top() >= news.bottom() - 0.5, "{context}: the notes sit under the news");
            assert!(
                notes.bottom() <= content.bottom() + 0.5 && notes.left() >= news.left() - 0.5,
                "{context}: {notes:?}"
            );
            if let Some(body) = shot.find("home:notes:body") {
                let fits = body.bottom() <= notes.bottom() + 0.5 && body.top() >= notes.top() - 0.5;
                assert!(fits, "{context}: the release notes {body:?} overflow their room {notes:?}");
            }
        }
    } else {
        assert!(news.top() >= after_play.bottom(), "{context}: the news starts below the Play block");
        assert!(news.left() >= content.left() - 0.5 && news.right() <= content.right() + 0.5, "{context}: {news:?}");
    }
    for (name, rect) in &text {
        assert!(
            !rect.intersects(news) || rect.intersect(news).area() < 1.0,
            "{context}: {name} {rect:?} meets the news {news:?}"
        );
    }
    let rows = shot.items();
    if items > 0 {
        assert!(!rows.is_empty(), "{context}: at least the first item is on screen");
    }
    disjoint(context, &rows);
    for (name, rect) in &rows {
        assert!(
            rect.left() >= news.left() - 0.5 && rect.right() <= news.right() + 0.5,
            "{context}: {name} {rect:?} leaves {news:?} sideways"
        );
        assert!(rect.top() >= news.top() - 0.5, "{context}: {name} {rect:?} starts above {news:?}");
    }
}

#[test]
fn the_news_sits_beside_or_below_the_play_block_without_overlap_at_every_size() {
    for language in ["en", "de"] {
        for size in HOME_SIZES {
            let cases: [(&str, Vec<Item>, bool); 5] = [
                ("no news", Vec::new(), false),
                ("no news, notes", Vec::new(), true),
                ("three items", demo(language), false),
                ("three items, notes", demo(language), true),
                ("twenty long items, notes", long_items(20), true),
            ];
            for (what, items, notes) in cases {
                let count = items.len();
                let mut h = home(language, size, items, notes);
                let shot = shoot(&mut h);
                check_home(&format!("{language} {size:?} {what}"), &shot, count);
            }
        }
    }
}

#[test]
fn hostile_and_huge_news_draws_and_stays_in_its_column() {
    for language in ["en", "de"] {
        for size in HOME_SIZES {
            let mut items = hostile_items();
            items.extend(long_items(13));
            let mut h = home(language, size, items, true);
            for item in h.app.news.items.clone() {
                h.app.news.open.insert(item.id);
            }
            let shot = shoot(&mut h);
            check_home(&format!("{language} {size:?} hostile, all open"), &shot, 20);
            assert!(h.app.notes.iter().all(|n| n.kind != crate::app::NoteKind::Error), "{:?}", h.app.notes.len());
        }
    }
}

#[test]
fn an_item_can_be_opened_and_closed_and_long_text_is_shortened_until_then() {
    let mut h = home("en", [1280.0, 720.0], long_items(1), false);
    h.ctx.enable_accesskit();
    let height = |h: &mut Harness| shoot(h).items()[0].1.height();
    let closed = height(&mut h);
    assert!(h.app.news.open.is_empty());
    h.app.news.open.insert("long0".into());
    let open = height(&mut h);
    assert!(open > closed * 2.0, "the opened item is much taller: {closed} -> {open}");
    h.app.news.open.clear();
    assert_eq!(height(&mut h), closed);
}

fn labels(h: &mut Harness) -> Vec<String> {
    h.draw();
    let nodes = h.frame().platform_output.accesskit_update.expect("accesskit is on").nodes;
    nodes.iter().flat_map(|(_, n)| [n.label(), n.value()]).flatten().map(str::to_owned).collect()
}

#[test]
fn every_empty_state_is_one_quiet_line_in_both_languages() {
    for language in ["en", "de"] {
        let cases = [
            (Phase::Loading, "news.loading"),
            (Phase::Fresh, "news.empty"),
            (Phase::Failed, "news.unavailable"),
            (Phase::Offline, "news.offline_none"),
        ];
        for (phase, key) in cases {
            let mut h = home(language, [1280.0, 720.0], Vec::new(), false);
            h.ctx.enable_accesskit();
            h.app.news.phase = phase;
            h.app.auto_check = false;
            h.app.settings.offline = phase == Phase::Offline;
            let want = h.app.tr.get(key).to_owned();
            assert!(!want.starts_with("news."), "{key} is translated");
            assert!(labels(&mut h).contains(&want), "{language} {phase:?}: no line {want:?}");
        }
    }
}

#[test]
fn saved_news_shows_from_the_first_frame_and_offline_is_marked_quietly() {
    let mut h = Harness::new("en");
    h.ctx.enable_accesskit();
    let dir = h.app.paths.news_cache();
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("en.json"), r#"{"etag":"\"x\"","items":[{"id":"a","title":"Saved title","body":"Saved **body**"},{"id":"b","title":"Second"}]}"#).unwrap();
    h.app.load_news_cache();
    assert_eq!(h.app.news.items.len(), 2, "read from disk before any request");
    assert_eq!(h.app.news.phase, Phase::Idle);
    h.app.settings.offline = true;
    h.app.auto_check = true;
    h.app.checked = true;
    let seen = labels(&mut h);
    assert_eq!(h.app.news.phase, Phase::Offline);
    assert_eq!(h.app.jobs.running, 0, "offline mode sends nothing");
    assert!(seen.iter().any(|l| l == "Saved title"), "{seen:?}");
    assert!(seen.contains(&h.app.tr.get("news.offline_saved").to_owned()), "the offline mark");
}

#[test]
fn a_failed_request_keeps_the_saved_news_and_never_blocks_play() {
    let mut h = Harness::new("en");
    let dir = h.app.paths.news_cache();
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("en.json"), r#"{"items":[{"id":"a","title":"Saved"},{"id":"b","title":"Also saved"}]}"#)
        .unwrap();
    h.app.load_news_cache();
    h.app.checked = true;
    h.app.settings.offline = false;
    h.app.settings.release_url = "http://127.0.0.1:9".into();
    let ctx = h.ctx.clone();
    h.app.auto_news(&ctx);
    if std::env::var(castanea_launcher_core::install::release_api::URL_ENV).is_ok() {
        return; // an override of the server address would make this a real request
    }
    assert_eq!((h.app.news.phase, h.app.jobs.running), (Phase::Loading, 1));
    assert!(h.app.can_play(), "Play does not wait for the news");
    for _ in 0..500 {
        h.frame();
        assert!(h.app.can_play());
        if h.app.news.phase != Phase::Loading {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(h.app.news.phase, Phase::Failed);
    assert_eq!(h.app.news.items.len(), 2, "the saved news stay");
    assert!(h.app.news.stale());
    h.app.auto_news(&ctx);
    assert_eq!(h.app.jobs.running, 0, "no second request until something changes");
}

#[test]
fn each_language_has_its_own_saved_news_and_a_late_answer_for_the_other_is_ignored() {
    let mut h = Harness::new("en");
    let dir = h.app.paths.news_cache();
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("en.json"), r#"{"items":[{"id":"a","title":"One"},{"id":"b","title":"Two"}]}"#).unwrap();
    fs::write(dir.join("de.json"), r#"{"items":[{"id":"a","title":"Eins"}]}"#).unwrap();
    h.app.load_news_cache();
    assert_eq!(h.app.news.items.len(), 2);
    h.app.settings.language = "de".into();
    h.app.reload_language();
    assert_eq!((h.app.news.items.len(), h.app.news.items[0].title.as_str()), (1, "Eins"));
    assert!(!h.app.news.requested, "the new language is asked for again");
    h.app.on_news("en".into(), Ok(Vec::new()));
    assert_eq!(h.app.news.items.len(), 1, "an answer for the old language changes nothing");
    h.app.on_news("de".into(), Err("HTTP 500".into()));
    assert_eq!((h.app.news.phase, h.app.news.items.len()), (Phase::Failed, 1));
    h.app.on_news("de".into(), Ok(vec![Item { id: "n".into(), title: "New".into(), ..Item::default() }]));
    assert_eq!((h.app.news.phase, h.app.news.items[0].title.as_str()), (Phase::Fresh, "New"));
}

#[test]
fn the_release_notes_section_follows_the_installed_build() {
    let mut h = home("en", [1280.0, 720.0], demo("en"), true);
    h.ctx.enable_accesskit();
    let title = h.app.tr.fill("news.whats_new", &[("version", "0.2.0")]);
    assert!(labels(&mut h).contains(&title));
    h.app.installed.as_mut().unwrap().notes.clear();
    assert!(!labels(&mut h).contains(&title), "no notes, no section");
    assert!(shoot(&mut h).find("home:notes").is_none_or(|r| r.height() < 1.0), "its room is given back to the news");
}
