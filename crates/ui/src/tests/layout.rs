//! Layout checks without snapshots: the named regions each page records (`probe.rs`) stay inside their parents and do
//! not overlap, at every window size.

use eframe::egui::{self, Rect};

use super::{Harness, SIZES};
use crate::app::{ModsView, Page};
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

fn shoot(h: &mut Harness) -> Shot {
    h.draw();
    Shot(probe::take(&h.ctx))
}

fn named(shot: &Shot, names: &[&str]) -> Vec<(String, Rect)> {
    names.iter().filter_map(|n| shot.find(n).map(|r| ((*n).to_owned(), r))).collect()
}

#[test]
fn frame_and_home_regions_do_not_overlap() {
    for size in SIZES {
        let mut h = Harness::new("de");
        h.size = egui::vec2(size[0], size[1]);
        let screen = Rect::from_min_size(egui::Pos2::ZERO, h.size);
        h.go(Page::Home);
        let shot = shoot(&mut h);
        let context = format!("{size:?}");
        let frame = named(&shot, &["title_bar", "sidebar", "content"]);
        disjoint(&context, &frame);
        for (name, rect) in &frame {
            assert!(inside(*rect, screen), "{context}: {name} {rect:?} leaves the window");
        }
        let sidebar = shot.get("sidebar");
        let nav = shot.starting("nav:");
        assert_eq!(nav.len(), Page::ALL.len() + 1, "{context}: every page and Play in the sidebar");
        for (name, rect) in &nav {
            assert!(inside(*rect, sidebar), "{context}: {name} {rect:?} leaves the sidebar {sidebar:?}");
        }
        disjoint(&context, &nav);

        let content = shot.get("content");
        let wide = shot.find("home:right").is_some();
        assert_eq!(
            wide,
            content.width() >= crate::theme::layout::WIDE,
            "{context}: the right column folds when narrow"
        );
        let regions = named(&shot, &["home:welcome", "home:hero", "home:list", "home:actions", "home:right"]);
        assert!(regions.len() >= 4, "{context}: {:?}", shot.names());
        for (name, rect) in &regions {
            assert!(inside(*rect, content), "{context}: {name} {rect:?} leaves the content {content:?}");
        }
        disjoint(&context, &regions);
        let hero = shot.get("home:hero");
        for name in ["home:hero:play", "home:hero:edit"] {
            assert!(inside(shot.get(name), hero), "{context}: {name} leaves the hero");
        }
        disjoint(&context, &named(&shot, &["home:hero:play", "home:hero:edit"]));

        let cards: Vec<(String, Rect)> =
            shot.starting("card:").into_iter().filter(|(n, _)| n.matches(':').count() == 1).collect();
        assert_eq!(cards.len(), 2, "{context}: one card per profile");
        disjoint(&context, &cards);
        for (card, rect) in &cards {
            let parts = shot.starting(&format!("{card}:"));
            assert!(parts.len() >= 3, "{context}: {card} has text and buttons: {parts:?}");
            for (name, part) in &parts {
                assert!(inside(*part, *rect), "{context}: {name} {part:?} leaves {card} {rect:?}");
            }
            disjoint(&context, &parts);
        }
        let side_cards = shot.starting("home:card:");
        assert_eq!(side_cards.len(), 4, "{context}: four cards on the right or under the list");
        disjoint(&context, &side_cards);
    }
}

#[test]
fn page_columns_do_not_overlap() {
    for size in SIZES {
        let mut h = Harness::new("en");
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
        for (name, rect) in &columns {
            assert!(inside(*rect, content), "{context}: {name} {rect:?} leaves the content {content:?}");
        }

        h.go(Page::Profiles);
        let shot = shoot(&mut h);
        disjoint(&context, &named(&shot, &["profiles:list", "profiles:editor"]));

        h.go(Page::Settings);
        let shot = shoot(&mut h);
        let sections = shot.starting("settings:");
        assert_eq!(sections.len(), 4, "{context}: {:?}", shot.names());
        disjoint(&context, &sections);

        h.app.mods_view = ModsView::Installed;
        h.go(Page::Mods);
        let shot = shoot(&mut h);
        let rows = shot.starting("mod:");
        assert_eq!(rows.len(), 2, "{context}: {:?}", shot.names());
        disjoint(&context, &rows);
    }
}
