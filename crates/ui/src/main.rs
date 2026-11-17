//! The castanea launcher: a native window to start the game, manage builds and mods, and browse a mod index.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod chrome;
mod hero;
mod i18n;
mod icons;
mod jobs;
mod pages;
mod probe;
mod screenshots;
mod shell;
mod theme;
mod widgets;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

use castanea_launcher_core::paths::Paths;
use castanea_launcher_core::settings::Settings;
use eframe::egui;

const USAGE: &str = "usage: castanea-launcher [--screenshots DIR] [--version]
  --screenshots DIR   draw each page once, save PNGs to DIR, then quit
Environment: CASTANEA_LAUNCHER_HOME puts config, data and cache below one folder (portable mode).";

struct Launcher {
    app: app::App,
    shots: Option<screenshots::Shots>,
}

impl eframe::App for Launcher {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        shell::ui(&mut self.app, ui);
        if let Some(shots) = &mut self.shots {
            shots.tick(&mut self.app, ui.ctx());
        }
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        // Screenshots show no hover or tooltip from wherever the pointer happens to be.
        if self.shots.is_some() {
            raw_input.events.retain(|e| !matches!(e, egui::Event::PointerMoved(_) | egui::Event::PointerButton { .. }));
        }
    }
}

fn main() -> std::process::ExitCode {
    let mut args = std::env::args().skip(1);
    let mut shots_dir: Option<PathBuf> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--screenshots" => shots_dir = args.next().map(PathBuf::from),
            "--version" => {
                println!("castanea-launcher {}", env!("CARGO_PKG_VERSION"));
                return std::process::ExitCode::SUCCESS;
            }
            _ => {
                eprintln!("{USAGE}");
                return std::process::ExitCode::from(2);
            }
        }
    }
    let Some(paths) = Paths::for_user() else {
        eprintln!("no home folder: set CASTANEA_LAUNCHER_HOME");
        return std::process::ExitCode::FAILURE;
    };
    let screenshot_mode = shots_dir.is_some();
    let system_frame = Settings::load(&paths).map(|s| s.system_frame).unwrap_or(false);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("castanea launcher")
            .with_inner_size(theme::layout::START_WINDOW)
            .with_min_inner_size(theme::layout::MIN_WINDOW)
            .with_decorations(system_frame && !screenshot_mode)
            .with_app_id("castanea-launcher"),
        persist_window: !screenshot_mode,
        persistence_path: (!screenshot_mode).then(|| paths.data.join("window")),
        ..Default::default()
    };
    let shots = shots_dir.map(screenshots::Shots::new);
    let failed = shots.as_ref().map(|s| s.failed.clone());
    let result = eframe::run_native(
        "castanea-launcher",
        options,
        Box::new(move |cc| {
            theme::apply(&cc.egui_ctx);
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(Launcher { app: app::App::new(paths), shots }))
        }),
    );
    if let Err(error) = result {
        eprintln!("castanea-launcher: {error}");
        return std::process::ExitCode::FAILURE;
    }
    if let Some(error) = failed.and_then(|f| f.lock().ok().and_then(|e| e.clone())) {
        eprintln!("screenshots: {error}");
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}
