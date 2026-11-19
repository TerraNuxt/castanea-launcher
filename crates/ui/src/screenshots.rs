//! `--screenshots <dir>`: walks through the pages, saves one PNG per view, then closes. For docs/screenshots
//! (tools/screenshots.sh). The update states are set directly, so no network is needed.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use eframe::egui;

use castanea_launcher_core::history::{self, Entry, History};
use castanea_launcher_core::launch::Start;

use crate::app::{App, Confirm, ModsView, Page};
use crate::game::Update;
use crate::jobs::{self, Progress};
use crate::news::Phase;

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

/// The made-up news in the current language.
fn news(app: &mut App, phase: Phase) {
    app.show_demo_news(phase);
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

/// Writes made-up screenshots (a flat low-poly scene in a different colour mood each) into the demo game folder: five today,
/// four yesterday and the rest on earlier days, named as the game names them.
fn demo_screenshots(app: &mut App) {
    let dir = app.paths.screenshots();
    let _ = std::fs::create_dir_all(&dir);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
    let day = 86_400_000;
    for n in 0..14i64 {
        let at = match n {
            0..=4 => now - n * 47 * 60_000,
            5..=8 => now - day - (n - 5) * 53 * 60_000,
            _ => now - (n - 6) * day - n * 31 * 60_000,
        };
        let _ = demo_picture(n).save(dir.join(format!("castanea-{at}.png")));
    }
    app.gallery.stale = true;
}

/// A made-up flat low-poly scene, a different colour mood for each `n`.
fn demo_picture(n: i64) -> image::RgbaImage {
    let moods: [([u8; 3], [u8; 3]); 6] = [
        ([232, 170, 120], [70, 90, 60]),
        ([120, 160, 210], [60, 100, 70]),
        ([210, 120, 110], [90, 70, 80]),
        ([150, 190, 220], [80, 110, 90]),
        ([40, 50, 90], [30, 50, 60]),
        ([225, 205, 150], [100, 90, 60]),
    ];
    let (sky, hills) = moods[(n as usize) % moods.len()];
    image::RgbaImage::from_fn(960, 540, |x, y| {
        let (fx, fy) = (x as f32 / 960.0, y as f32 / 540.0);
        let ridge = 0.62 + 0.10 * (fx * (5.0 + n as f32 * 0.7) + n as f32).sin() + 0.05 * (fx * 17.0).sin();
        let sun = ((fx - 0.25 - 0.05 * n as f32).powi(2) + (fy - 0.25).powi(2)).sqrt() < 0.06;
        let colour = if fy > ridge {
            let shade = 0.75 + 0.25 * (fy - ridge).min(0.4) / 0.4;
            hills.map(|c| (f32::from(c) * shade) as u8)
        } else if sun {
            [250, 240, 200]
        } else {
            sky.map(|c| (f32::from(c) * (1.1 - 0.35 * fy)).min(255.0) as u8)
        };
        image::Rgba([colour[0], colour[1], colour[2], 255])
    })
}

/// A world that was played an hour ago, with its own picture: the Continue card shows it.
fn demo_played(app: &mut App) {
    let world = app.paths.game().join("saves/world1");
    let _ = std::fs::create_dir_all(&world);
    let _ = std::fs::write(world.join("world.meta"), "demo");
    let _ = demo_picture(2).save(world.join("preview.png"));
    let now = history::now_ms();
    let mut record = History::default();
    let (started, ended) = (now - 100 * 60_000, now - 40 * 60_000);
    let version = "0.2.0".to_owned();
    record.record_start(Entry { started, ended: Some(ended), version, start: Start::World("world1".into()) });
    let _ = record.save(&app.paths);
    app.quick.stale = true;
}

const STEPS: [Step; 30] = [
    Step {
        name: "home",
        wait: Wait::Idle,
        setup: |app| {
            demo_played(app);
            up_to_date(app);
            news(app, Phase::Fresh);
            app.go(Page::Home);
        },
    },
    Step {
        name: "home-de",
        wait: Wait::Idle,
        setup: |app| {
            language(app, "de");
            news(app, Phase::Fresh);
        },
    },
    Step {
        name: "home-update",
        wait: Wait::Idle,
        setup: |app| {
            language(app, "en");
            downloading(app);
            news(app, Phase::Fresh);
        },
    },
    Step {
        name: "home-update-de",
        wait: Wait::Idle,
        setup: |app| {
            language(app, "de");
            news(app, Phase::Fresh);
        },
    },
    Step {
        name: "home-running",
        wait: Wait::Running,
        setup: |app| {
            language(app, "en");
            up_to_date(app);
            news(app, Phase::Fresh);
            app.play_blocking();
        },
    },
    Step {
        name: "home-running-de",
        wait: Wait::Running,
        setup: |app| {
            language(app, "de");
            news(app, Phase::Fresh);
        },
    },
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
            news(app, Phase::Offline);
            app.go(Page::Home);
        },
    },
    Step {
        name: "home-offline-de",
        wait: Wait::Idle,
        setup: |app| {
            language(app, "de");
            news(app, Phase::Offline);
        },
    },
    Step {
        name: "home-no-news",
        wait: Wait::Idle,
        setup: |app| {
            language(app, "en");
            news(app, Phase::Fresh);
            app.news.items.clear();
        },
    },
    Step {
        name: "home-news-open",
        wait: Wait::Idle,
        setup: |app| {
            news(app, Phase::Fresh);
            app.news.open.insert("steam-age".to_owned());
        },
    },
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
    Step {
        name: "screenshots-empty",
        wait: Wait::Idle,
        setup: |app| {
            language(app, "en");
            app.gallery.stale = true;
            app.go(Page::Screenshots);
        },
    },
    Step {
        name: "screenshots",
        wait: Wait::Idle,
        setup: |app| {
            demo_screenshots(app);
            app.go(Page::Screenshots);
        },
    },
    Step {
        name: "screenshot-view",
        wait: Wait::Idle,
        setup: |app| app.gallery.viewer = app.gallery.shots.get(1).map(|s| s.path.clone()),
    },
    Step {
        name: "screenshots-de",
        wait: Wait::Idle,
        setup: |app| {
            language(app, "de");
            app.close_viewer();
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
    Step {
        name: "settings-storage",
        wait: Wait::Idle,
        setup: |app| {
            app.advanced_open = false;
            app.reveal_storage = Some(egui::Align::Min);
        },
    },
    Step { name: "settings-storage-end", wait: Wait::Idle, setup: |app| app.reveal_storage = Some(egui::Align::Max) },
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
    Step {
        name: "settings-storage-de",
        wait: Wait::Idle,
        setup: |app| {
            app.go(Page::Settings);
            app.reveal_storage = Some(egui::Align::Min);
        },
    },
    Step {
        name: "storage-everything-de",
        wait: Wait::Idle,
        setup: |app| app.confirm = Some(Confirm::DeleteEverything),
    },
    Step {
        name: "storage-everything",
        wait: Wait::Idle,
        setup: |app| {
            language(app, "en");
            app.confirm = Some(Confirm::DeleteEverything);
        },
    },
    Step {
        name: "storage-game-data",
        wait: Wait::Idle,
        setup: |app| {
            app.storage.typed = "DELE".to_owned();
            app.confirm = Some(Confirm::DeleteGameData);
        },
    },
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
            Wait::Idle => app.jobs.running == 0 && !app.is_running() && app.gallery_settled(),
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
            self.fail(format!("{}: never settled (size {size:?}, sized {sized}, ready {ready})", step.name));
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
