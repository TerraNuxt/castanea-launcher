//! Background work: each job runs on its own thread and sends one result back; the UI never blocks.

use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use castanea_launcher_core::backend::Choice;
use castanea_launcher_core::download::Speed;
use castanea_launcher_core::index::Index;
use castanea_launcher_core::install::Build;
use castanea_launcher_core::install::update::Check;
use castanea_launcher_core::selfupdate;
use eframe::egui;

pub enum Done {
    Index {
        index: Index,
        warnings: Vec<String>,
    },
    /// The update check of the game build.
    Checked(Result<Check, String>),
    /// A newer build downloaded, checked and unpacked next to the installed one.
    Downloaded(Result<Build, String>),
    /// The downloaded build became the installed one.
    Applied(Result<Build, String>),
    /// The graphics backend for this start is decided (the probe may have run), or why not.
    Backend(Result<Choice, String>),
    /// The launcher update check.
    LauncherChecked(Result<selfupdate::Check, String>),
    /// A newer launcher is staged and in place (its version), or why not.
    LauncherApplied(Result<String, String>),
    ModInstalled {
        id: String,
        result: Result<(), String>,
    },
    Image {
        url: String,
        result: Result<Vec<u8>, String>,
    },
    /// The news feed of one language, brought up to date.
    News {
        lang: String,
        result: Result<Vec<castanea_launcher_core::news::Item>, String>,
    },
}

/// A download in progress: what, bytes done, bytes in all.
#[derive(Clone, Debug, Default)]
pub struct Progress {
    pub label: String,
    pub done: u64,
    pub total: Option<u64>,
    /// Bytes per second, once known.
    pub speed: Option<u64>,
}

impl Progress {
    /// Done as a fraction, when the size is known.
    pub fn fraction(&self) -> Option<f32> {
        self.total.map(|t| (self.done as f64 / t.max(1) as f64).clamp(0.0, 1.0) as f32)
    }
}

pub type Slot = Arc<Mutex<Option<Progress>>>;

pub struct Jobs {
    tx: Sender<Done>,
    rx: Receiver<Done>,
    pub running: usize,
    /// The mod download in progress.
    pub progress: Slot,
    pub cancel: Arc<AtomicBool>,
}

impl Default for Jobs {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self { tx, rx, running: 0, progress: Arc::default(), cancel: Arc::default() }
    }
}

impl Jobs {
    pub fn spawn(&mut self, ctx: &egui::Context, work: impl FnOnce() -> Done + Send + 'static) {
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        self.running += 1;
        std::thread::spawn(move || {
            let done = work();
            let _ = tx.send(done);
            ctx.request_repaint();
        });
    }

    pub fn finished(&mut self) -> Vec<Done> {
        let out: Vec<Done> = self.rx.try_iter().collect();
        self.running -= out.len().min(self.running);
        out
    }
}

/// A progress callback for a download that fills `slot` and redraws the window.
pub fn reporter(ctx: &egui::Context, slot: &Slot, label: String) -> impl FnMut(u64, Option<u64>) + Send + 'static {
    let slot = slot.clone();
    let ctx = ctx.clone();
    set(&slot, Some(Progress { label: label.clone(), ..Progress::default() }));
    let mut meter = Speed::default();
    move |done, total| {
        set(&slot, Some(Progress { label: label.clone(), done, total, speed: meter.sample(done) }));
        ctx.request_repaint();
    }
}

pub fn set(slot: &Slot, value: Option<Progress>) {
    if let Ok(mut current) = slot.lock() {
        *current = value;
    }
}

pub fn get(slot: &Slot) -> Option<Progress> {
    slot.lock().ok().and_then(|p| p.clone())
}
