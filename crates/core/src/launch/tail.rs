//! Following the game's log file: the game writes it itself (no pipe to the launcher), this reads new lines into the buffer.

use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use super::run::LogBuffer;

const POLL: Duration = Duration::from_millis(100);

/// A thread that reads the log file from an offset on; dropping it leaves the game alone.
pub struct Tail {
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Tail {
    pub fn start(path: PathBuf, from: u64, log: Arc<Mutex<LogBuffer>>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let handle = thread::spawn(move || follow(&path, from, &log, &flag));
        Self { stop, handle: Some(handle) }
    }

    /// Reads what is left, then ends the thread.
    pub fn finish(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn follow(path: &PathBuf, from: u64, log: &Mutex<LogBuffer>, stop: &AtomicBool) {
    let Ok(mut file) = fs::File::open(path) else { return };
    if file.seek(SeekFrom::Start(from)).is_err() {
        return;
    }
    let mut pending = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let last = stop.load(Ordering::SeqCst);
        loop {
            match file.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => pending.extend_from_slice(&chunk[..n]),
            }
        }
        while let Some(at) = pending.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = pending.drain(..=at).collect();
            push(log, &line);
        }
        if last {
            if !pending.is_empty() {
                push(log, &pending);
            }
            return;
        }
        thread::sleep(POLL);
    }
}

fn push(log: &Mutex<LogBuffer>, raw: &[u8]) {
    let line = String::from_utf8_lossy(raw).trim_end_matches(['\n', '\r']).to_owned();
    if let Ok(mut log) = log.lock() {
        log.push(line);
    }
}
