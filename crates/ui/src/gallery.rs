//! The screenshot gallery's state: the scanned list, the thumbnails the workers made (a bounded number of textures), the
//! large view and its actions. The page that draws it is `pages/screenshots.rs`; the scanning, the cache and the file
//! actions are in the core (`castanea_launcher_core::gallery`).

pub mod pool;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use castanea_launcher_core::gallery::thumb::{self, Pixels};
use castanea_launcher_core::gallery::{self, Group, Scan, Shot, files, time};
use eframe::egui;

use crate::app::{App, Confirm, Page};
use crate::jobs::Done;
use pool::{Key, Loaded, Pool};

/// Thumbnail textures kept at once: about 50 MB, far more than a screenful, so scrolling back is instant.
pub const TEXTURE_CAP: usize = 160;

/// A decoded picture as a texture.
fn texture(ctx: &egui::Context, name: String, pixels: &Pixels) -> egui::TextureHandle {
    let image = egui::ColorImage::from_rgba_unmultiplied([pixels.width as usize, pixels.height as usize], &pixels.rgba);
    ctx.load_texture(name, image, egui::TextureOptions::LINEAR)
}

/// The thumbnail textures, least recently used out first.
#[derive(Default)]
pub struct Textures {
    map: HashMap<PathBuf, (egui::TextureHandle, u64)>,
    tick: u64,
}

impl Textures {
    pub fn get(&mut self, path: &Path) -> Option<&egui::TextureHandle> {
        self.tick += 1;
        let tick = self.tick;
        self.map.get_mut(path).map(|(handle, used)| {
            *used = tick;
            &*handle
        })
    }

    fn insert(&mut self, path: PathBuf, handle: egui::TextureHandle) {
        self.tick += 1;
        self.map.insert(path, (handle, self.tick));
        while self.map.len() > TEXTURE_CAP {
            let Some(oldest) = self.map.iter().min_by_key(|(_, (_, used))| *used).map(|(p, _)| p.clone()) else {
                break;
            };
            self.map.remove(&oldest);
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.map.len()
    }
}

#[derive(Default)]
pub struct GalleryState {
    /// Newest first.
    pub shots: Vec<Shot>,
    pub groups: Vec<Group>,
    /// Bumped whenever `shots` changes, so the page lays itself out again.
    pub version: u64,
    pub unreadable: usize,
    /// A scan has finished at least once.
    pub scanned: bool,
    pub scanning: bool,
    /// A new scan is wanted at the next frame (the page was opened, the game ended, Refresh).
    pub stale: bool,
    pub was_focused: bool,
    /// The file in the large view.
    pub viewer: Option<PathBuf>,
    /// The new name being typed, without its extension.
    pub rename: Option<String>,
    /// After a scan, open this file in the large view (a rename).
    pub reopen: Option<PathBuf>,
    pub pool: Option<Pool>,
    pub thumbs: Textures,
    /// Files that could not be decoded: they show a placeholder and are not tried again until the next scan.
    pub failed: HashSet<PathBuf>,
    /// The picture of the large view, scaled to fit the texture limit.
    pub view: Option<(PathBuf, egui::TextureHandle)>,
    pub view_failed: Option<PathBuf>,
    /// The grid's rows for the current list and width.
    pub layout: Option<Layout>,
    /// How many of the thumbnails the page asked for last are still on their way; none before the page has drawn.
    pub missing: Option<usize>,
}

impl GalleryState {
    /// The position of `path` in the list.
    pub fn index_of(&self, path: &Path) -> Option<usize> {
        self.shots.iter().position(|s| s.path == path)
    }

    /// The shot in the large view.
    pub fn open_shot(&self) -> Option<&Shot> {
        let path = self.viewer.as_deref()?;
        self.index_of(path).map(|i| &self.shots[i])
    }

    /// Opens the shot `step` places away (positive: older), stopping at the ends.
    pub fn step(&mut self, step: isize) {
        let Some(at) = self.viewer.as_deref().and_then(|p| self.index_of(p)) else { return };
        let to = at.saturating_add_signed(step).min(self.shots.len().saturating_sub(1));
        self.viewer = self.shots.get(to).map(|s| s.path.clone());
        self.rename = None;
    }

    fn set_scan(&mut self, scan: Scan) {
        self.shots = scan.shots;
        self.unreadable = scan.unreadable;
        self.groups = gallery::group_by_day(&self.shots, time::local_offset);
        self.version += 1;
        self.scanned = true;
        self.scanning = false;
        self.missing = None;
        let live: HashSet<&Path> = self.shots.iter().map(|s| s.path.as_path()).collect();
        self.failed.retain(|p| live.contains(p.as_path()));
        if self.view_failed.as_ref().is_some_and(|p| !live.contains(p.as_path())) {
            self.view_failed = None;
        }
        if let Some(path) = self.reopen.take()
            && self.index_of(&path).is_some()
        {
            self.viewer = Some(path);
        }
        if self.viewer.as_deref().is_some_and(|p| self.index_of(p).is_none()) {
            self.viewer = None;
            self.rename = None;
        }
    }

    /// Takes a file out of the list at once (it was deleted): the large view moves to its neighbour, or closes.
    fn remove(&mut self, path: &Path) {
        let Some(at) = self.index_of(path) else { return };
        let was_open = self.viewer.as_deref() == Some(path);
        self.shots.remove(at);
        self.groups = gallery::group_by_day(&self.shots, time::local_offset);
        self.version += 1;
        self.failed.remove(path);
        if was_open {
            self.rename = None;
            self.viewer = self.shots.get(at.min(self.shots.len().saturating_sub(1))).map(|s| s.path.clone());
        }
    }
}

/// Now as a local calendar day, for "Today" and "Yesterday".
pub fn today() -> time::Day {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
    time::day_of(now + time::local_offset(now))
}

impl App {
    /// Starts a scan when one is due: at the first frame, after the page was opened or the game ended, when the window gets
    /// the focus back on the page (files appear while it is open), and at Refresh.
    pub fn auto_gallery(&mut self, ctx: &egui::Context) {
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        let regained = focused && !self.gallery.was_focused;
        self.gallery.was_focused = focused;
        let due = !self.gallery.scanned || self.gallery.stale || (regained && self.page == Page::Screenshots);
        if due && !self.gallery.scanning {
            self.rescan(ctx);
        }
    }

    pub fn rescan(&mut self, ctx: &egui::Context) {
        self.gallery.stale = false;
        self.gallery.scanning = true;
        let (dir, cache) = (self.paths.screenshots(), self.paths.thumbnails());
        self.jobs.spawn(ctx, move || {
            let scan = gallery::scan(&dir);
            // The cache keeps only thumbnails of files that are still there, and stays under its size limit.
            thumb::prune_cache(&cache, &thumb::live_keys(&scan.shots), thumb::CACHE_LIMIT);
            Done::ShotsScanned(scan)
        });
    }

    pub fn on_shots_scanned(&mut self, scan: Scan) {
        self.gallery.set_scan(scan);
    }

    fn pool(&mut self, ctx: &egui::Context) -> &Pool {
        let cache = self.paths.thumbnails();
        self.gallery.pool.get_or_insert_with(|| Pool::new(ctx, cache))
    }

    /// Takes in what the workers finished: thumbnails go into the texture cache, failures are remembered.
    pub fn collect_thumbs(&mut self, ctx: &egui::Context) {
        let Some(pool) = &self.gallery.pool else { return };
        let loaded: Vec<Loaded> = pool.poll();
        for Loaded { key, result } in loaded {
            match (key, result) {
                (Key::Thumb(path), Ok(pixels)) => {
                    let handle = texture(ctx, format!("thumb:{}", path.display()), &pixels);
                    self.gallery.thumbs.insert(path, handle);
                }
                (Key::Thumb(path), Err(_)) => {
                    self.gallery.failed.insert(path);
                }
                (Key::View(path), Ok(pixels)) => {
                    let handle = texture(ctx, format!("view:{}", path.display()), &pixels);
                    self.gallery.view = Some((path, handle));
                }
                (Key::View(path), Err(_)) => self.gallery.view_failed = Some(path),
            }
        }
    }

    /// Asks the workers for these thumbnails (the ones without a texture that did not fail), in the order given.
    pub fn want_thumbs(&mut self, ctx: &egui::Context, shots: &[Shot]) {
        let wanted: Vec<Shot> = shots
            .iter()
            .filter(|s| !self.gallery.failed.contains(&s.path) && self.gallery.thumbs.get(&s.path).is_none())
            .cloned()
            .collect();
        self.gallery.missing = Some(wanted.len());
        self.pool(ctx).want_thumbs(wanted);
    }

    /// Asks for the picture of the large view when it is not the one shown.
    pub fn want_view(&mut self, ctx: &egui::Context, path: &Path) {
        let shown = self.gallery.view.as_ref().is_some_and(|(p, _)| p == path);
        if !shown && self.gallery.view_failed.as_deref() != Some(path) {
            self.pool(ctx).want_view(path);
        }
    }

    pub fn open_viewer(&mut self, path: PathBuf) {
        self.gallery.viewer = Some(path);
        self.gallery.rename = None;
    }

    pub fn close_viewer(&mut self) {
        self.gallery.viewer = None;
        self.gallery.rename = None;
    }

    /// Copies the picture at its real size to the clipboard; decoding runs on a worker.
    pub fn copy_shot(&mut self, ctx: &egui::Context, path: PathBuf) {
        self.jobs.spawn(ctx, move || Done::ShotPixels(thumb::load_full(&path).map_err(|e| e.to_string())));
    }

    pub fn on_shot_pixels(&mut self, ctx: &egui::Context, result: Result<Pixels, String>) {
        match result {
            Ok(pixels) => {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [pixels.width as usize, pixels.height as usize],
                    &pixels.rgba,
                );
                ctx.copy_image(image);
                let text = self.tr.get("shots.copied").to_owned();
                self.info(text);
            }
            Err(error) => self.error(error),
        }
    }

    /// Opens the file with the program the system has for it.
    pub fn open_shot_file(&mut self, path: &Path) {
        if let Err(error) = open_file(path) {
            self.error(format!("{}: {error}", path.display()));
        }
    }

    /// Shows the file in the system's file manager (Windows selects it, Linux opens the folder).
    pub fn show_shot_in_folder(&mut self, path: &Path) {
        if let Err(error) = reveal(path) {
            self.error(format!("{}: {error}", path.display()));
        }
    }

    /// Asks before deleting: the question says the file goes to the trash.
    pub fn ask_delete_shot(&mut self, path: PathBuf) {
        self.confirm = Some(Confirm::DeleteShot(path));
    }

    /// Moves the file to the trash; when that is not possible asks again whether to delete it for good.
    pub fn delete_shot(&mut self, path: PathBuf) {
        let dir = self.paths.screenshots();
        match files::trash(&dir, &path) {
            Ok(()) => self.after_shot_removed(&path),
            Err(reason) if path.exists() => self.confirm = Some(Confirm::DeleteShotForever { path, reason }),
            Err(reason) => self.error(reason),
        }
    }

    /// Deletes the file for good, after the second question.
    pub fn delete_shot_forever(&mut self, path: PathBuf) {
        match files::delete(&self.paths.screenshots(), &path) {
            Ok(()) => self.after_shot_removed(&path),
            Err(error) => self.error(error.to_string()),
        }
    }

    fn after_shot_removed(&mut self, path: &Path) {
        let text = self.tr.fill("shots.deleted", &[("name", &file_name(path))]);
        self.gallery.remove(path);
        self.info(text);
    }

    /// Renames the open file (the name without its extension); the list is scanned again and the view stays on the file.
    pub fn rename_shot(&mut self, ctx: &egui::Context, path: &Path, stem: &str) {
        match files::rename(&self.paths.screenshots(), path, stem) {
            Ok(new) => {
                self.gallery.rename = None;
                self.gallery.reopen = Some(new);
                self.rescan(ctx);
            }
            Err(error) => self.error(error.to_string()),
        }
    }
}

pub fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Opens a file with the system's default program.
pub fn open_file(path: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    let program = "explorer";
    #[cfg(not(windows))]
    let program = "xdg-open";
    std::process::Command::new(program).arg(path).spawn().map(|_| ())
}

/// Shows the file in the file manager: Explorer selects it; elsewhere its folder opens.
pub fn reveal(path: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("explorer").raw_arg(format!("/select,\"{}\"", path.display())).spawn().map(|_| ())
    }
    #[cfg(not(windows))]
    {
        let dir = path.parent().unwrap_or(path);
        std::process::Command::new("xdg-open").arg(dir).spawn().map(|_| ())
    }
}

/// One line of the grid: a day's header, or a run of thumbnails (indexes into the shot list).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowKind {
    Header(usize),
    Cells(std::ops::Range<usize>),
}

/// Where each row of the grid is, so only the rows in view are drawn however many files there are.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub version: u64,
    pub cols: usize,
    pub cell_h: f32,
    pub rows: Vec<RowKind>,
    /// The top of each row from the top of the content.
    pub tops: Vec<f32>,
    pub total: f32,
}

/// Height of a day's header row, and the gap between thumbnails.
pub const HEADER_H: f32 = 44.0;
pub const GAP: f32 = 12.0;

impl Layout {
    pub fn build(groups: &[Group], cols: usize, cell_h: f32, version: u64) -> Self {
        let cols = cols.max(1);
        let mut rows = Vec::new();
        for (n, group) in groups.iter().enumerate() {
            rows.push(RowKind::Header(n));
            let mut at = group.range.start;
            while at < group.range.end {
                let end = (at + cols).min(group.range.end);
                rows.push(RowKind::Cells(at..end));
                at = end;
            }
        }
        let mut tops = Vec::with_capacity(rows.len());
        let mut y = 0.0;
        for row in &rows {
            tops.push(y);
            y += Self::height_of(row, cell_h);
        }
        Self { version, cols, cell_h, rows, tops, total: y }
    }

    pub fn height_of(row: &RowKind, cell_h: f32) -> f32 {
        match row {
            RowKind::Header(_) => HEADER_H,
            RowKind::Cells(_) => cell_h + GAP,
        }
    }

    /// The rows that touch the span `from..to` of the content, plus `extra` rows on each side.
    pub fn visible(&self, from: f32, to: f32, extra: usize) -> std::ops::Range<usize> {
        let first = self.tops.partition_point(|&top| top <= from).saturating_sub(1);
        let last = self.tops.partition_point(|&top| top < to);
        first.saturating_sub(extra)..(last + extra).min(self.rows.len())
    }
}

impl App {
    /// Nothing is on its way for the page that is open: the thumbnails of its first screenful (and the large view) are
    /// decoded or failed. Screenshots of the launcher and the idle tests wait for it.
    pub fn gallery_settled(&mut self) -> bool {
        if self.page == Page::Home {
            return self.card_settled();
        }
        if self.page != Page::Screenshots {
            return true;
        }
        let g = &mut self.gallery;
        if !g.scanned || g.scanning {
            return false;
        }
        let thumbs = g.missing == Some(0) || g.shots.is_empty();
        let view = g
            .viewer
            .as_ref()
            .is_none_or(|p| g.view_failed.as_ref() == Some(p) || g.view.as_ref().is_some_and(|(shown, _)| shown == p));
        thumbs && view
    }
}
