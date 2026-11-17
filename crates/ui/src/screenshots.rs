//! `--screenshots <dir>`: walks through the tabs, saves one PNG per view, then closes. For docs/screenshots.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::app::{App, Confirm, ModsView, Tab};

struct Step {
    name: &'static str,
    setup: fn(&mut App),
}

const STEPS: [Step; 9] = [
    Step { name: "play", setup: |app| app.tab = Tab::Play },
    Step {
        name: "play-log",
        setup: |app| {
            app.tab = Tab::Play;
            app.play();
        },
    },
    Step {
        name: "profile",
        setup: |app| {
            app.tab = Tab::Play;
            app.editing_profile = true;
        },
    },
    Step {
        name: "mods-installed",
        setup: |app| {
            app.editing_profile = false;
            app.tab = Tab::Mods;
            app.mods_view = ModsView::Installed;
        },
    },
    Step {
        name: "mods-browse",
        setup: |app| {
            app.tab = Tab::Mods;
            app.mods_view = ModsView::Browse;
            app.browse.selected = app.index.mods.first().map(|m| m.id.clone());
        },
    },
    Step {
        name: "mods-enable",
        setup: |app| {
            app.tab = Tab::Mods;
            app.mods_view = ModsView::Installed;
            app.confirm =
                app.mods.iter().find(|m| !m.enabled).map(|m| Confirm::EnableMod { file_name: m.file_name.clone() });
        },
    },
    Step {
        name: "versions",
        setup: |app| {
            app.confirm = None;
            app.tab = Tab::Versions;
        },
    },
    Step { name: "settings", setup: |app| app.tab = Tab::Settings },
    Step {
        name: "mods-installed-de",
        setup: |app| {
            app.settings.language = "de".to_owned();
            app.reload_language();
            app.tab = Tab::Mods;
            app.mods_view = ModsView::Installed;
        },
    },
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

    /// Called after each frame is drawn.
    pub fn tick(&mut self, app: &mut App, ctx: &egui::Context) {
        ctx.request_repaint();
        let Some(step) = STEPS.get(self.step) else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        };
        if self.frames == 0 {
            (step.setup)(app);
        }
        self.frames += 1;
        let settled = self.frames > 30 && app.jobs.running == 0 && app.game.is_none();
        if settled && !self.requested {
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
                *self.failed.lock().unwrap_or_else(|e| e.into_inner()) = Some(error);
            }
            eprintln!("screenshot {}", path.display());
            self.step += 1;
            self.frames = 0;
            self.requested = false;
        }
        if self.frames > 2000 {
            *self.failed.lock().unwrap_or_else(|e| e.into_inner()) = Some(format!("{}: never settled", step.name));
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn save(image: &egui::ColorImage, path: &std::path::Path) -> Result<(), String> {
    let [w, h] = image.size;
    let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| [c.r(), c.g(), c.b()]).collect();
    std::fs::create_dir_all(path.parent().unwrap_or(path)).map_err(|e| e.to_string())?;
    image::save_buffer(path, &bytes, w as u32, h as u32, image::ExtendedColorType::Rgb8)
        .map_err(|e| format!("{}: {e}", path.display()))
}
