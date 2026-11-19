//! The start window: its size comes from the monitor and its scale, so it fits a small screen; eframe remembers the size
//! and position the user leaves it with (`persist_window`). The sizing is pure functions over (monitor pixels, scale).

use eframe::egui;

use crate::theme::layout::{MIN_WINDOW, START_WINDOW};

/// The most of the monitor's logical width and height a window may take. eframe knows the whole monitor, not its work
/// area, so the rest is room for the task bar and the title bar.
pub const MAX_SHARE: [f32; 2] = [0.92, 0.80];
/// A remembered window taller or wider than this share of the monitor is brought back to the start size.
const TOO_BIG: [f32; 2] = [0.98, 0.90];

/// The monitor in logical points.
fn logical(monitor_px: [f32; 2], scale: f32) -> [f32; 2] {
    let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    [monitor_px[0] / scale, monitor_px[1] / scale]
}

/// The window size a first start gets, in points: the usual start size, cut to `MAX_SHARE` of the monitor, never below
/// the smallest size the layout supports (a screen too small for that gets the minimum).
pub fn start_size(monitor_px: [f32; 2], scale: f32) -> [f32; 2] {
    let m = logical(monitor_px, scale);
    let pick = |axis: usize| (START_WINDOW[axis].min(m[axis] * MAX_SHARE[axis]).floor()).max(MIN_WINDOW[axis]);
    [pick(0), pick(1)]
}

/// A remembered size that does not fit the monitor (the bottom or the right edge is off the screen) gives the size to use
/// instead; one that fits gives none.
pub fn fit_remembered(current: [f32; 2], monitor_px: [f32; 2], scale: f32) -> Option<[f32; 2]> {
    let m = logical(monitor_px, scale);
    let too_big = current[0] > m[0] * TOO_BIG[0] || current[1] > m[1] * TOO_BIG[1];
    too_big.then(|| {
        let allowed = start_size(monitor_px, scale);
        [current[0].min(allowed[0].max(MIN_WINDOW[0])), current[1].min(allowed[1].max(MIN_WINDOW[1]))]
    })
}

/// Whether the window sits on a monitor whose top left corner is (0, 0), the one `center_on_screen` can centre on.
pub fn on_origin_monitor(outer: egui::Rect, monitor: egui::Vec2) -> bool {
    egui::Rect::from_min_size(egui::Pos2::ZERO, monitor).contains(outer.center())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    /// Waiting for the monitor's size.
    Measure(u32),
    /// The size is set; centre once the window has taken it.
    Centre(u32),
    Done,
}

/// Sets the size at the first frames with a known monitor: a first start gets `start_size` and is centred, a remembered
/// window that is too big for the monitor is made smaller and centred, anything else is left alone.
pub struct Placer {
    first_run: bool,
    stage: Stage,
}

/// Frames to wait for the monitor's size before giving up (some systems never report it).
const MEASURE_FRAMES: u32 = 30;
/// Frames between the size command and the centring, so the window has taken the new size.
const SETTLE_FRAMES: u32 = 3;

impl Placer {
    pub fn new(first_run: bool) -> Self {
        Self { first_run, stage: Stage::Measure(0) }
    }

    pub fn tick(&mut self, ctx: &egui::Context) {
        match self.stage {
            Stage::Done => {}
            Stage::Measure(frames) => {
                let (monitor, scale, inner) = ctx.input(|i| {
                    let v = i.viewport();
                    (v.monitor_size, v.native_pixels_per_point, v.inner_rect.map(|r| r.size()))
                });
                let (Some(monitor), Some(inner)) = (monitor, inner) else {
                    self.stage = if frames >= MEASURE_FRAMES { Stage::Done } else { Stage::Measure(frames + 1) };
                    ctx.request_repaint();
                    return;
                };
                let scale = scale.unwrap_or_else(|| ctx.pixels_per_point());
                let px = [monitor.x * scale, monitor.y * scale];
                let target = if self.first_run {
                    Some(start_size(px, scale))
                } else {
                    fit_remembered([inner.x, inner.y], px, scale)
                };
                match target {
                    Some(size) => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(size[0], size[1])));
                        self.stage = Stage::Centre(SETTLE_FRAMES);
                        ctx.request_repaint();
                    }
                    None => self.stage = Stage::Done,
                }
            }
            Stage::Centre(0) => {
                let outer = ctx.input(|i| (i.viewport().outer_rect, i.viewport().monitor_size));
                if let (Some(outer), Some(monitor)) = outer
                    && on_origin_monitor(outer, monitor)
                    && let Some(command) = egui::ViewportCommand::center_on_screen(ctx)
                {
                    ctx.send_viewport_cmd(command);
                }
                self.stage = Stage::Done;
            }
            Stage::Centre(left) => {
                self.stage = Stage::Centre(left - 1);
                ctx.request_repaint();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A screen of `w` x `h` physical pixels at `percent` scaling.
    fn start(w: f32, h: f32, percent: f32) -> [f32; 2] {
        start_size([w, h], percent / 100.0)
    }

    /// The window and its title bar and task bar (48 and 32 points at most) fit under the screen's logical height.
    fn fits(size: [f32; 2], h: f32, percent: f32) -> bool {
        size[1] + 48.0 + 32.0 <= h / (percent / 100.0)
    }

    #[test]
    fn a_1080p_screen_gets_a_window_that_fits_at_every_common_scale() {
        assert_eq!(start(1920.0, 1080.0, 100.0), START_WINDOW);
        assert_eq!(start(1920.0, 1080.0, 125.0), [1100.0, 691.0]);
        assert_eq!(start(1920.0, 1080.0, 150.0), [1100.0, 576.0]);
        assert_eq!(start(1920.0, 1080.0, 175.0), [1009.0, MIN_WINDOW[1]]);
        for percent in [100.0, 125.0, 150.0, 175.0] {
            let size = start(1920.0, 1080.0, percent);
            assert!(fits(size, 1080.0, percent), "{percent} %: {size:?} is taller than the screen allows");
            assert!(size[0] >= MIN_WINDOW[0] && size[1] >= MIN_WINDOW[1], "{percent} %: {size:?}");
        }
    }

    #[test]
    fn other_screens() {
        assert_eq!(start(1366.0, 768.0, 100.0), [1100.0, 614.0]);
        assert!(fits(start(1366.0, 768.0, 100.0), 768.0, 100.0));
        assert_eq!(start(3840.0, 2160.0, 200.0), START_WINDOW, "4K at 200 % is a 1080p screen");
        assert_eq!(start(2560.0, 1440.0, 100.0), START_WINDOW);
        assert_eq!(start(1280.0, 720.0, 100.0), [1100.0, 576.0]);
        assert_eq!(start(800.0, 600.0, 100.0), MIN_WINDOW, "a screen smaller than the minimum gets the minimum");
    }

    #[test]
    fn a_nonsense_scale_is_taken_as_one() {
        assert_eq!(start_size([1920.0, 1080.0], 0.0), START_WINDOW);
        assert_eq!(start_size([1920.0, 1080.0], f32::NAN), START_WINDOW);
        assert_eq!(start_size([1920.0, 1080.0], -1.0), START_WINDOW);
    }

    #[test]
    fn the_size_never_grows_with_a_smaller_screen() {
        let mut last = [f32::MAX; 2];
        for h in (500..=2160).rev().step_by(20) {
            let size = start_size([h as f32 * 16.0 / 9.0, h as f32], 1.0);
            assert!(size[0] <= last[0] && size[1] <= last[1], "{h}: {size:?} after {last:?}");
            assert!(size[0] >= MIN_WINDOW[0] && size[1] >= MIN_WINDOW[1]);
            last = size;
        }
    }

    #[test]
    fn a_remembered_size_that_fits_is_kept_and_one_that_does_not_is_brought_back() {
        assert_eq!(fit_remembered([1115.0, 737.0], [1920.0, 1080.0], 1.0), None);
        assert_eq!(fit_remembered([1115.0, 737.0], [1920.0, 1080.0], 1.25), None, "864 high: 737 is under 90 %");
        // The Windows test PC: 1115x737 on 1920x1080 at 150 % is 1106 physical pixels tall.
        assert_eq!(fit_remembered([1115.0, 737.0], [1920.0, 1080.0], 1.5), Some([1100.0, 576.0]));
        assert_eq!(fit_remembered([1115.0, 737.0], [1920.0, 1080.0], 1.75), Some([1009.0, MIN_WINDOW[1]]));
        assert_eq!(fit_remembered([900.0, 560.0], [1920.0, 1080.0], 1.5), None, "a smaller remembered size stays");
        let shrunk = fit_remembered([1500.0, 560.0], [1920.0, 1080.0], 1.5).unwrap();
        assert_eq!(shrunk[1], 560.0, "only the side that is too big changes");
        assert_eq!(shrunk[0], 1100.0);
    }

    #[test]
    fn centring_only_on_the_monitor_at_the_origin() {
        let monitor = egui::vec2(1920.0, 1080.0);
        let on = egui::Rect::from_min_size(egui::pos2(100.0, 80.0), egui::vec2(1100.0, 700.0));
        let off = egui::Rect::from_min_size(egui::pos2(2100.0, 80.0), egui::vec2(1100.0, 700.0));
        assert!(on_origin_monitor(on, monitor));
        assert!(!on_origin_monitor(off, monitor), "a second monitor to the right is not at the origin");
    }
}
