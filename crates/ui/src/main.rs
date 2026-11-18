//! The castanea launcher: a native window that keeps the game at its newest release, starts it, and manages mods.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod bmfont;
mod game;
mod i18n;
mod icons;
mod jobs;
mod launcher;
mod news;
mod pages;
mod probe;
mod scene;
mod screenshots;
mod shell;
mod sidebar;
mod theme;
mod widgets;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

use castanea_launcher_core::install::local::GAME_DIR_ENV;
use castanea_launcher_core::paths::Paths;
use castanea_launcher_core::selfupdate::{self, Layout, Os, Startup};
use eframe::egui;

const USAGE: &str = "usage: castanea-launcher [--game-dir DIR] [--screenshots DIR] [--version]
  --game-dir DIR      run the game build in DIR in place (a checkout or a build folder) instead of the newest release
  --screenshots DIR   draw each page once, save PNGs to DIR, then quit
Environment: CASTANEA_LAUNCHER_HOME puts config, data and cache below one folder (portable mode);
CASTANEA_GAME_DIR is the same as --game-dir.";

struct Launcher {
    app: app::App,
    shots: Option<screenshots::Shots>,
}

impl eframe::App for Launcher {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        shell::ui(&mut self.app, ui);
        self.app.frame_drawn();
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
    let mut game_dir = std::env::var_os(GAME_DIR_ENV).filter(|v| !v.is_empty()).map(PathBuf::from);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--screenshots" => shots_dir = args.next().map(PathBuf::from),
            "--game-dir" => game_dir = args.next().map(PathBuf::from),
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
    // Screenshots never touch the installed launcher.
    let layout = if screenshot_mode { None } else { Layout::detect() };
    if let Some(layout) = &layout {
        match selfupdate::startup(Os::current(), &paths, layout, launcher::VERSION) {
            Startup::RolledBack { failed, restored } => {
                eprintln!("castanea-launcher: {failed} did not start, back to {restored}");
                if selfupdate::relaunch(layout, std::env::args_os().skip(1)).is_ok() {
                    return std::process::ExitCode::SUCCESS;
                }
            }
            Startup::NoBackup { failed } => {
                eprintln!("castanea-launcher: {failed} did not start and there is no backup")
            }
            Startup::Normal => {}
        }
    }
    let viewport = egui::ViewportBuilder::default()
        .with_title("castanea")
        .with_inner_size(theme::layout::START_WINDOW)
        .with_app_id("castanea-launcher");
    // Screenshots may ask for a window below the minimum, to show the narrow layout.
    let viewport = if screenshot_mode { viewport } else { viewport.with_min_inner_size(theme::layout::MIN_WINDOW) };
    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions {
        viewport,
        persist_window: !screenshot_mode,
        persistence_path: (!screenshot_mode).then(|| paths.data.join("window")),
        ..Default::default()
    };
    // Windows: DX12 then GL, not Vulkan (an Intel UHD Vulkan driver crashes in wgpu); WGPU_BACKEND still wins.
    #[cfg(windows)]
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(create) = &mut options.wgpu_options.wgpu_setup {
        create.instance_descriptor.backends =
            eframe::wgpu::Backends::from_env().unwrap_or(eframe::wgpu::Backends::DX12 | eframe::wgpu::Backends::GL);
    }
    let shots = shots_dir.map(screenshots::Shots::new);
    let failed = shots.as_ref().map(|s| s.failed.clone());
    let result = eframe::run_native(
        "castanea-launcher",
        options,
        Box::new(move |cc| {
            theme::apply(&cc.egui_ctx);
            egui_extras::install_image_loaders(&cc.egui_ctx);
            let mut app = app::App::new(paths, game_dir);
            app.launcher_layout = layout;
            app.auto_check = shots.is_none();
            Ok(Box::new(Launcher { app, shots }))
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
