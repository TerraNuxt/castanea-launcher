//! `--screenshots <dir>`: walks through the pages, saves one PNG per view, then closes. For docs/screenshots
//! (tools/screenshots.sh). The update states are set directly, so no network is needed.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::app::{App, ModsView, Page};
use crate::game::Update;
use crate::jobs::{self, Progress};

const SIZE: [f32; 2] = [1280.0, 720.0];

/// The window size: 1280x720, or `CASTANEA_SHOT_SIZE=WxH` for a second set at another size.
fn shot_size() -> [f32; 2] {
    let parsed = std::env::var("CASTANEA_SHOT_SIZE").ok().and_then(|v| {
        let (w, h) = v.split_once('x')?;
        Some([w.parse().ok()?, h.parse().ok()?])
    });
    parsed.unwrap_or(SIZE)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Wait {
    /// No job and no game.
    Idle,
    /// The game is running.
    Running,
}

struct Step {
    name: &'static str,
    wait: Wait,
    setup: fn(&mut App),
}

fn language(app: &mut App, code: &str) {
    if app.settings.language != code {
        app.settings.language = code.to_owned();
        app.reload_language();
    }
}

fn downloading(app: &mut App) {
    app.update = Update::Downloading { version: "0.3.0".to_owned() };
    let progress = Progress { label: "0.3.0".to_owned(), done: 43, total: Some(100), speed: Some(5_200_000) };
    jobs::set(&app.update_progress, Some(progress));
}

fn up_to_date(app: &mut App) {
    app.update = Update::UpToDate;
    jobs::set(&app.update_progress, None);
}

const STEPS: [Step; 18] = [
    Step {
        name: "home",
        wait: Wait::Idle,
        setup: |app| {
            up_to_date(app);
            app.go(Page::Home);
        },
    },
    Step { name: "home-de", wait: Wait::Idle, setup: |app| language(app, "de") },
    Step {
        name: "home-update",
        wait: Wait::Idle,
        setup: |app| {
            language(app, "en");
            downloading(app);
        },
    },
    Step { name: "home-update-de", wait: Wait::Idle, setup: |app| language(app, "de") },
    Step {
        name: "home-running",
        wait: Wait::Running,
        setup: |app| {
            language(app, "en");
            up_to_date(app);
            app.play_blocking();
        },
    },
    Step { name: "home-running-de", wait: Wait::Running, setup: |app| language(app, "de") },
    Step {
        name: "output",
        wait: Wait::Running,
        setup: |app| {
            language(app, "en");
            app.go(Page::Log);
        },
    },
    Step {
        name: "home-offline",
        wait: Wait::Idle,
        setup: |app| {
            app.stop();
            app.settings.offline = true;
            app.update = Update::Offline;
            app.go(Page::Home);
        },
    },
    Step { name: "home-offline-de", wait: Wait::Idle, setup: |app| language(app, "de") },
    Step {
        name: "mods-installed",
        wait: Wait::Idle,
        setup: |app| {
            language(app, "en");
            app.settings.offline = false;
            up_to_date(app);
            app.mods_view = ModsView::Installed;
            app.go(Page::Mods);
        },
    },
    Step {
        name: "mods-browse",
        wait: Wait::Idle,
        setup: |app| {
            app.mods_view = ModsView::Browse;
            app.browse.selected = Some("amber".to_owned());
        },
    },
    Step { name: "settings", wait: Wait::Idle, setup: |app| app.go(Page::Settings) },
    Step {
        name: "settings-advanced",
        wait: Wait::Idle,
        setup: |app| {
            app.advanced_open = true;
            app.reveal_advanced = true;
        },
    },
    Step { name: "about", wait: Wait::Idle, setup: |app| app.go(Page::About) },
    Step {
        name: "mods-installed-de",
        wait: Wait::Idle,
        setup: |app| {
            language(app, "de");
            app.mods_view = ModsView::Installed;
            app.go(Page::Mods);
        },
    },
    Step { name: "settings-de", wait: Wait::Idle, setup: |app| app.go(Page::Settings) },
    Step { name: "output-de", wait: Wait::Idle, setup: |app| app.go(Page::Log) },
    Step { name: "about-de", wait: Wait::Idle, setup: |app| app.go(Page::About) },
];

pub struct Shots {
    size: [f32; 2],
    dir: PathBuf,
    step: usize,
    frames: u32,
    requested: bool,
    image: Arc<Mutex<Option<Arc<egui::ColorImage>>>>,
    pub failed: Arc<Mutex<Option<String>>>,
}

impl Shots {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            size: shot_size(),
            dir,
            step: 0,
            frames: 0,
            requested: false,
            image: Arc::default(),
            failed: Arc::default(),
        }
    }

    fn fail(&self, text: String) {
        *self.failed.lock().unwrap_or_else(|e| e.into_inner()) = Some(text);
    }

    /// Called after each frame is drawn.
    pub fn tick(&mut self, app: &mut App, ctx: &egui::Context) {
        ctx.request_repaint();
        let Some(step) = STEPS.get(self.step) else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        };
        if self.frames == 0 {
            (step.setup)(app);
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(self.size[0], self.size[1])));
        }
        self.frames += 1;
        let size = ctx.content_rect().size();
        let sized = (size.x - self.size[0]).abs() < 1.0 && (size.y - self.size[1]).abs() < 1.0;
        let ready = match step.wait {
            Wait::Idle => app.jobs.running == 0 && !app.is_running(),
            Wait::Running => app.is_running(),
        };
        if self.frames > 40 && sized && ready && !self.requested {
            self.requested = true;
            let slot = self.image.clone();
            ctx.request_screenshot(move |image| {
                if let Ok(mut slot) = slot.lock() {
                    *slot = Some(image);
                }
            });
        }
        let image = self.image.lock().ok().and_then(|mut slot| slot.take());
        if let Some(image) = image {
            let path = self.dir.join(format!("{}.png", step.name));
            if let Err(error) = save(&image, &path) {
                self.fail(error);
            }
            eprintln!("screenshot {}", path.display());
            self.step += 1;
            self.frames = 0;
            self.requested = false;
        }
        if self.frames > 2000 {
            self.fail(format!("{}: never settled (size {size:?})", step.name));
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn save(image: &egui::ColorImage, path: &std::path::Path) -> Result<(), String> {
    use image::ImageEncoder;
    let [w, h] = image.size;
    let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| [c.r(), c.g(), c.b()]).collect();
    std::fs::create_dir_all(path.parent().unwrap_or(path)).map_err(|e| e.to_string())?;
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let encoder = image::codecs::png::PngEncoder::new_with_quality(
        std::io::BufWriter::new(file),
        image::codecs::png::CompressionType::Best,
        image::codecs::png::FilterType::Adaptive,
    );
    encoder
        .write_image(&bytes, w as u32, h as u32, image::ExtendedColorType::Rgb8)
        .map_err(|e| format!("{}: {e}", path.display()))
}
