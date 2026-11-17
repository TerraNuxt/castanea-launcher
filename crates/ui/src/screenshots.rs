//! `--screenshots <dir>`: walks through the pages at two window sizes, saves one PNG per view, then closes. For
//! docs/screenshots (tools/screenshots.sh).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::app::{App, Confirm, ModsView, Page};

const BIG: [f32; 2] = [1280.0, 720.0];
const SMALL: [f32; 2] = [1000.0, 640.0];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Wait {
    /// No job and no game.
    Idle,
    /// A game is running.
    Running,
}

struct Step {
    name: &'static str,
    size: [f32; 2],
    wait: Wait,
    setup: fn(&mut App),
}

fn language(app: &mut App, code: &str) {
    if app.settings.language != code {
        app.settings.language = code.to_owned();
        app.reload_language();
    }
}

const STEPS: [Step; 16] = [
    Step { name: "home", size: BIG, wait: Wait::Idle, setup: |app| app.go(Page::Home) },
    Step { name: "home-de", size: BIG, wait: Wait::Idle, setup: |app| language(app, "de") },
    Step { name: "home-1000", size: SMALL, wait: Wait::Idle, setup: |app| language(app, "en") },
    Step { name: "home-1000-de", size: SMALL, wait: Wait::Idle, setup: |app| language(app, "de") },
    Step {
        name: "home-running",
        size: BIG,
        wait: Wait::Running,
        setup: |app| {
            language(app, "en");
            app.play();
        },
    },
    Step { name: "log", size: BIG, wait: Wait::Running, setup: |app| app.go(Page::Log) },
    Step {
        name: "mods-installed",
        size: BIG,
        wait: Wait::Idle,
        setup: |app| {
            let id = app.profile_id.clone();
            app.stop(&id);
            app.mods_view = ModsView::Installed;
            app.go(Page::Mods);
        },
    },
    Step {
        name: "mods-enable",
        size: BIG,
        wait: Wait::Idle,
        setup: |app| {
            app.confirm =
                app.mods.iter().find(|m| !m.enabled).map(|m| Confirm::EnableMod { file_name: m.file_name.clone() });
        },
    },
    Step {
        name: "mods-browse",
        size: BIG,
        wait: Wait::Idle,
        setup: |app| {
            app.confirm = None;
            app.mods_view = ModsView::Browse;
            app.browse.selected = app.index.mods.first().map(|m| m.id.clone());
        },
    },
    Step { name: "mods-browse-1000", size: SMALL, wait: Wait::Idle, setup: |_| {} },
    Step { name: "profiles", size: BIG, wait: Wait::Idle, setup: |app| app.go(Page::Profiles) },
    Step { name: "versions", size: BIG, wait: Wait::Idle, setup: |app| app.go(Page::Versions) },
    Step { name: "settings", size: BIG, wait: Wait::Idle, setup: |app| app.go(Page::Settings) },
    Step { name: "settings-1000", size: SMALL, wait: Wait::Idle, setup: |_| {} },
    Step {
        name: "notifications",
        size: BIG,
        wait: Wait::Idle,
        setup: |app| {
            let text = app.tr.fill("mods.installed_disabled", &[("id", "frost")]);
            app.info(text);
            app.error("index.json: https://mods.example.org/index.json: connection refused");
            app.go(Page::Notifications);
        },
    },
    Step { name: "about", size: BIG, wait: Wait::Idle, setup: |app| app.go(Page::About) },
];

pub struct Shots {
    dir: PathBuf,
    step: usize,
    frames: u32,
    requested: bool,
    image: Arc<Mutex<Option<Arc<egui::ColorImage>>>>,
    pub failed: Arc<Mutex<Option<String>>>,
}

impl Shots {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir, step: 0, frames: 0, requested: false, image: Arc::default(), failed: Arc::default() }
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
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(step.size[0], step.size[1])));
        }
        self.frames += 1;
        let size = ctx.content_rect().size();
        let sized = (size.x - step.size[0]).abs() < 1.0 && (size.y - step.size[1]).abs() < 1.0;
        let ready = match step.wait {
            Wait::Idle => app.jobs.running == 0 && !app.any_running(),
            Wait::Running => app.any_running(),
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
