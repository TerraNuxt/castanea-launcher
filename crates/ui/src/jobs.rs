//! Background work: each job runs on its own thread and sends one result back; the UI never blocks.

use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use castanea_launcher_core::index::Index;
use castanea_launcher_core::install::{Build, RemoteBuild};
use eframe::egui;

pub enum Done {
    Index { index: Index, warnings: Vec<String> },
    Releases { builds: Vec<(usize, RemoteBuild)>, errors: Vec<String> },
    BuildInstalled(Result<Build, String>),
    ModInstalled { id: String, result: Result<(), String> },
    Image { url: String, result: Result<Vec<u8>, String> },
}

/// The current long task for the status bar: what, bytes done, bytes in all.
#[derive(Clone, Debug, Default)]
pub struct Progress {
    pub label: String,
    pub done: u64,
    pub total: Option<u64>,
}

pub struct Jobs {
    tx: Sender<Done>,
    rx: Receiver<Done>,
    pub running: usize,
    pub progress: Arc<Mutex<Option<Progress>>>,
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

    /// A progress callback for a download that updates the status bar.
    pub fn reporter(&self, ctx: &egui::Context, label: String) -> impl FnMut(u64, Option<u64>) + Send + 'static {
        let progress = self.progress.clone();
        let ctx = ctx.clone();
        if let Ok(mut slot) = progress.lock() {
            *slot = Some(Progress { label: label.clone(), done: 0, total: None });
        }
        move |done, total| {
            if let Ok(mut slot) = progress.lock() {
                *slot = Some(Progress { label: label.clone(), done, total });
            }
            ctx.request_repaint();
        }
    }

    pub fn clear_progress(&self) {
        if let Ok(mut slot) = self.progress.lock() {
            *slot = None;
        }
    }
}
