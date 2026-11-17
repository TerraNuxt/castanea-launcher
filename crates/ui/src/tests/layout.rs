//! Layout checks without snapshots: the named regions each page records (`probe.rs`) stay inside their parents and do
//! not overlap, at every window size.

use eframe::egui::{self, Rect};

use super::{Harness, SIZES};
use crate::app::{ModsView, Page};
use crate::game::Update;
use crate::probe;

struct Shot(Vec<(String, Rect)>);

impl Shot {
    fn get(&self, name: &str) -> Rect {
        self.find(name).unwrap_or_else(|| panic!("no region {name}: {:?}", self.names()))
    }

    fn find(&self, name: &str) -> Option<Rect> {
        self.0.iter().find(|(n, _)| n == name).map(|(_, r)| *r)
    }

    fn names(&self) -> Vec<&str> {
        self.0.iter().map(|(n, _)| n.as_str()).collect()
    }

    fn starting(&self, prefix: &str) -> Vec<(String, Rect)> {
        self.0.iter().filter(|(n, _)| n.starts_with(prefix)).cloned().collect()
    }
}

fn overlap(a: Rect, b: Rect) -> f32 {
    let i = a.intersect(b);
    if i.is_positive() { i.area() } else { 0.0 }
}

fn inside(inner: Rect, outer: Rect) -> bool {
    outer.expand(0.5).contains_rect(inner)
}

fn disjoint(context: &str, regions: &[(String, Rect)]) {
    for (i, (a, ra)) in regions.iter().enumerate() {
        for (b, rb) in &regions[i + 1..] {
            assert!(overlap(*ra, *rb) < 1.0, "{context}: {a} {ra:?} overlaps {b} {rb:?}");
        }
    }
}

fn all_inside(context: &str, regions: &[(String, Rect)], outer: Rect) {
    for (name, rect) in regions {
        assert!(inside(*rect, outer), "{context}: {name} {rect:?} leaves {outer:?}");
    }
}

fn shoot(h: &mut Harness) -> Shot {
    h.draw();
    Shot(probe::take(&h.ctx))
}

fn named(shot: &Shot, names: &[&str]) -> Vec<(String, Rect)> {
    names.iter().filter_map(|n| shot.find(n).map(|r| ((*n).to_owned(), r))).collect()
}

#[test]
fn the_frame_and_the_home_page_hold_together() {
    for language in ["en", "de"] {
        for size in SIZES {
            let mut h = Harness::new(language);
            h.size = egui::vec2(size[0], size[1]);
            let screen = Rect::from_min_size(egui::Pos2::ZERO, h.size);
            for state in [Update::UpToDate, Update::Downloading { version: "0.3.0".into() }, Update::Failed("x".into())]
            {
                h.app.update = state.clone();
                h.go(Page::Home);
                let shot = shoot(&mut h);
                let context = format!("{language} {size:?} {state:?}");
                let frame = named(&shot, &["sidebar", "content"]);
                disjoint(&context, &frame);
                all_inside(&context, &frame, screen);
                let nav = shot.starting("nav:");
                assert_eq!(nav.len(), Page::ALL.len(), "{context}: every page in the sidebar");
                all_inside(&context, &nav, shot.get("sidebar"));
                disjoint(&context, &nav);

                let content = shot.get("content");
                let home = named(&shot, &["home:wordmark", "home:play", "home:status", "home:links"]);
                assert!(home.len() >= 3, "{context}: {:?}", shot.names());
                all_inside(&context, &home, content);
                disjoint(&context, &home);
                for (name, rect) in &home {
                    let off = (rect.center().x - content.center().x).abs();
                    assert!(off < 1.0, "{context}: {name} is {off} off the centre");
                }
                let play = shot.get("home:play");
                assert!(play.top() > content.top() + content.height() * 0.2, "{context}: the button sits mid page");
            }
        }
    }
}

#[test]
fn page_regions_do_not_overlap() {
    for size in SIZES {
        let mut h = Harness::new("de");
        h.size = egui::vec2(size[0], size[1]);
        let context = format!("{size:?}");

        h.app.mods_view = ModsView::Browse;
        h.app.browse.selected = Some("amber".to_owned());
        h.go(Page::Mods);
        let shot = shoot(&mut h);
        let content = shot.get("content");
        let columns = named(&shot, &["browse:list", "browse:details"]);
        assert!(!columns.is_empty(), "{context}: {:?}", shot.names());
        disjoint(&context, &columns);
        all_inside(&context, &columns, content);
        let rows = shot.starting("browse:row:");
        disjoint(&context, &rows);

        h.app.mods_view = ModsView::Installed;
        let shot = shoot(&mut h);
        let rows = shot.starting("mod:");
        assert_eq!(rows.len(), 2, "{context}: {:?}", shot.names());
        disjoint(&context, &rows);
        all_inside(&context, &rows, shot.get("content"));

        h.app.advanced_open = true;
        h.go(Page::Settings);
        let shot = shoot(&mut h);
        let groups = shot.starting("settings:");
        assert_eq!(groups.len(), 4, "{context}: {:?}", shot.names());
        disjoint(&context, &groups);
        all_inside_x(&context, &groups, shot.get("content"));

        h.go(Page::About);
        let shot = shoot(&mut h);
        let groups = shot.starting("about:");
        assert_eq!(groups.len(), 3, "{context}: {:?}", shot.names());
        disjoint(&context, &groups);
        all_inside_x(&context, &groups, shot.get("content"));
    }
}

#[cfg(unix)]
#[test]
fn the_output_page_holds_its_status_above_the_log() {
    for size in SIZES {
        let mut h = Harness::new("en");
        h.size = egui::vec2(size[0], size[1]);
        h.app.play();
        h.go(Page::Log);
        let shot = shoot(&mut h);
        let context = format!("{size:?}");
        let regions = named(&shot, &["log:status", "log:view"]);
        assert_eq!(regions.len(), 2, "{context}: {:?}", shot.names());
        disjoint(&context, &regions);
        all_inside(&context, &regions, shot.get("content"));
        h.app.stop();
    }
}

/// For regions in a scroll area: they stay inside `outer` from left to right and start below its top.
fn all_inside_x(context: &str, regions: &[(String, Rect)], outer: Rect) {
    for (name, rect) in regions {
        let fits =
            rect.left() >= outer.left() - 0.5 && rect.right() <= outer.right() + 0.5 && rect.top() >= outer.top();
        assert!(fits, "{context}: {name} {rect:?} leaves {outer:?} sideways");
    }
}
