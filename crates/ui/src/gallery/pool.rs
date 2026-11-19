//! The thumbnail workers: two threads that decode pictures off the UI thread. The UI says what it wants each frame; what is
//! no longer wanted is dropped from the queue, so a fast scroll never builds a backlog.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};

use castanea_launcher_core::gallery::Shot;
use castanea_launcher_core::gallery::thumb::{self, Pixels};
use eframe::egui;

/// What a worker made: a grid thumbnail, or the picture of the large view.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Thumb(PathBuf),
    View(PathBuf),
}

pub struct Loaded {
    pub key: Key,
    pub result: Result<Pixels, String>,
}

#[derive(Default)]
struct Queue {
    thumbs: VecDeque<Shot>,
    view: Option<PathBuf>,
    running: HashSet<Key>,
}

struct Shared {
    queue: Mutex<Queue>,
    wake: Condvar,
    stop: AtomicBool,
}

pub struct Pool {
    shared: Arc<Shared>,
    results: Receiver<Loaded>,
}

/// Workers: enough to keep the grid filling, few enough to leave the machine to the game.
const WORKERS: usize = 2;

impl Pool {
    pub fn new(ctx: &egui::Context, cache: PathBuf) -> Self {
        let shared = Arc::new(Shared { queue: Mutex::default(), wake: Condvar::new(), stop: AtomicBool::new(false) });
        let (tx, results) = channel();
        for n in 0..WORKERS {
            let (shared, tx, ctx, cache) = (shared.clone(), tx.clone(), ctx.clone(), cache.clone());
            let _ =
                std::thread::Builder::new().name(format!("thumbs-{n}")).spawn(move || work(&shared, &tx, &ctx, &cache));
        }
        Self { shared, results }
    }

    /// The thumbnails wanted now, most wanted first; the queue is replaced by them (a picture a worker already has goes on).
    pub fn want_thumbs(&self, shots: Vec<Shot>) {
        let Ok(mut queue) = self.shared.queue.lock() else { return };
        let fresh: VecDeque<Shot> =
            shots.into_iter().filter(|s| !queue.running.contains(&Key::Thumb(s.path.clone()))).collect();
        if fresh != queue.thumbs {
            queue.thumbs = fresh;
            self.shared.wake.notify_all();
        }
    }

    /// The picture for the large view; it goes before any thumbnail.
    pub fn want_view(&self, path: &Path) {
        let Ok(mut queue) = self.shared.queue.lock() else { return };
        if !queue.running.contains(&Key::View(path.to_path_buf())) && queue.view.as_deref() != Some(path) {
            queue.view = Some(path.to_path_buf());
            self.shared.wake.notify_all();
        }
    }

    /// What the workers finished since the last call.
    pub fn poll(&self) -> Vec<Loaded> {
        self.results.try_iter().collect()
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        self.shared.wake.notify_all();
    }
}

enum Job {
    Thumb(Shot),
    View(PathBuf),
}

fn next(shared: &Shared) -> Option<(Job, Key)> {
    let mut queue = shared.queue.lock().ok()?;
    loop {
        if shared.stop.load(Ordering::Relaxed) {
            return None;
        }
        let job = if let Some(path) = queue.view.take() {
            Some((Job::View(path.clone()), Key::View(path)))
        } else {
            queue.thumbs.pop_front().map(|shot| {
                let key = Key::Thumb(shot.path.clone());
                (Job::Thumb(shot), key)
            })
        };
        if let Some((job, key)) = job {
            queue.running.insert(key.clone());
            return Some((job, key));
        }
        queue = shared.wake.wait(queue).ok()?;
    }
}

fn work(shared: &Shared, tx: &Sender<Loaded>, ctx: &egui::Context, cache: &Path) {
    while let Some((job, key)) = next(shared) {
        let result = match &job {
            Job::Thumb(shot) => thumb::load_thumbnail(cache, shot),
            Job::View(path) => thumb::load_view(path),
        };
        if let Ok(mut queue) = shared.queue.lock() {
            queue.running.remove(&key);
        }
        let _ = tx.send(Loaded { key, result: result.map_err(|e| e.to_string()) });
        ctx.request_repaint();
    }
}
