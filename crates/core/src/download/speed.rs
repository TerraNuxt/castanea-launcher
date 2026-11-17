//! Download speed in bytes per second, smoothed over the last few seconds.

use std::time::{Duration, Instant};

const WINDOW: Duration = Duration::from_secs(4);

#[derive(Debug, Default)]
pub struct Speed {
    samples: Vec<(Instant, u64)>,
}

impl Speed {
    /// Notes that `done` bytes are there now; returns the speed over the window, once there is a second of data.
    pub fn sample(&mut self, done: u64) -> Option<u64> {
        self.sample_at(Instant::now(), done)
    }

    fn sample_at(&mut self, now: Instant, done: u64) -> Option<u64> {
        // A smaller count means a new download (or a restart from zero).
        if self.samples.last().is_some_and(|(_, last)| done < *last) {
            self.samples.clear();
        }
        self.samples.push((now, done));
        self.samples.retain(|(t, _)| now.duration_since(*t) <= WINDOW);
        let (first_time, first_done) = *self.samples.first()?;
        let seconds = now.duration_since(first_time).as_secs_f64();
        (seconds >= 1.0).then(|| ((done - first_done) as f64 / seconds) as u64)
    }
}

/// `5.2 MB/s`, `830 kB/s`, `12 B/s`.
pub fn rate_text(bytes_per_second: u64) -> String {
    let value = bytes_per_second as f64;
    if value >= 1e6 {
        format!("{:.1} MB/s", value / 1e6)
    } else if value >= 1e3 {
        format!("{:.0} kB/s", value / 1e3)
    } else {
        format!("{bytes_per_second} B/s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_needs_a_second_and_follows_the_window() {
        let start = Instant::now();
        let mut speed = Speed::default();
        assert_eq!(speed.sample_at(start, 0), None);
        assert_eq!(speed.sample_at(start + Duration::from_millis(500), 500), None);
        assert_eq!(speed.sample_at(start + Duration::from_secs(2), 2000), Some(1000));
        assert_eq!(speed.sample_at(start + Duration::from_secs(9), 10_000), None, "old samples drop");
        assert_eq!(speed.sample_at(start + Duration::from_secs(10), 15_000), Some(5000));
        assert_eq!(speed.sample_at(start + Duration::from_secs(11), 0), None, "a restart clears it");
    }

    #[test]
    fn rates_read_well() {
        assert_eq!(rate_text(5_240_000), "5.2 MB/s");
        assert_eq!(rate_text(830_000), "830 kB/s");
        assert_eq!(rate_text(12), "12 B/s");
    }
}
