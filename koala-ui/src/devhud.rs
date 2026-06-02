//! Developer-HUD sampler: process heap + CPU as a rolling time series.
//!
//! The overlay's data comes from two process-wide sources:
//! `koala_common`'s counting global allocator (registered in `main`)
//! for heap bytes / allocation calls, and `getrusage(RUSAGE_SELF)` for
//! CPU time. A timer on the Rust side calls [`HudSampler::sample`] on a
//! fixed cadence; each call derives rates from the *actual* elapsed
//! wall time (so cadence jitter doesn't distort them) and keeps a ring
//! buffer of live-heap samples, which it renders into SVG path strings
//! for the window's heaptrack-style area chart.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::time::Instant;

use koala_common::alloc_count;

/// Live-heap samples retained for the chart. At the 250 ms sample
/// cadence the timer uses, this is a ~60 s rolling window.
const HISTORY: usize = 240;

const BYTES_PER_MB: f64 = 1024.0 * 1024.0;

/// Coordinate space the chart paths are drawn in; the `Path` element
/// scales this viewbox to its on-screen size, so the units are
/// arbitrary — 1000 just gives smooth sub-pixel resolution.
const VIEW: f64 = 1000.0;

/// Headroom above the all-time peak so a curve sitting *at* peak leaves
/// a margin at the top of the chart instead of gluing to the edge.
const Y_HEADROOM: f64 = 1.15;

/// One computed HUD frame, ready to push into the Slint window.
pub(crate) struct HudFrame {
    pub live_mb: f64,
    pub peak_mb: f64,
    pub alloc_rate_mb_s: f64,
    pub alloc_calls_per_s: f64,
    pub cpu_pct: f64,
    /// Top of the chart's Y axis in MB (peak + headroom) — drives the
    /// scale label.
    pub axis_max_mb: f64,
    /// SVG path commands for the filled area under the heap curve.
    pub heap_area: String,
    /// SVG path commands for the heap curve's top line (stroked).
    pub heap_line: String,
}

/// Holds the previous snapshot so each [`sample`](Self::sample) can turn
/// the monotonic allocator/CPU counters into per-interval rates, plus
/// the live-heap ring buffer the chart is drawn from.
pub(crate) struct HudSampler {
    last_wall: Instant,
    last_total_allocated: usize,
    last_alloc_calls: usize,
    last_cpu_us: u64,
    history: VecDeque<f64>,
}

impl HudSampler {
    pub(crate) fn new() -> Self {
        let snap = alloc_count::snapshot();
        Self {
            last_wall: Instant::now(),
            last_total_allocated: snap.total_allocated,
            last_alloc_calls: snap.alloc_calls,
            last_cpu_us: process_cpu_us(),
            history: VecDeque::with_capacity(HISTORY),
        }
    }

    /// Sample the counters now, advance the deltas, and return a frame.
    pub(crate) fn sample(&mut self) -> HudFrame {
        let snap = alloc_count::snapshot();
        let now = Instant::now();
        // Clamp the interval away from zero so the first tick (or a
        // degenerate same-instant call) can't divide by zero.
        let dt = now.duration_since(self.last_wall).as_secs_f64().max(1e-6);

        let live_mb = snap.live as f64 / BYTES_PER_MB;
        let peak_mb = snap.peak as f64 / BYTES_PER_MB;
        let alloc_rate_mb_s =
            snap.total_allocated.saturating_sub(self.last_total_allocated) as f64 / BYTES_PER_MB / dt;
        let alloc_calls_per_s =
            snap.alloc_calls.saturating_sub(self.last_alloc_calls) as f64 / dt;

        let cpu_us = process_cpu_us();
        let cpu_pct = cpu_us.saturating_sub(self.last_cpu_us) as f64 / (dt * 1_000_000.0) * 100.0;

        self.last_wall = now;
        self.last_total_allocated = snap.total_allocated;
        self.last_alloc_calls = snap.alloc_calls;
        self.last_cpu_us = cpu_us;

        if self.history.len() == HISTORY {
            let _ = self.history.pop_front();
        }
        self.history.push_back(live_mb);

        // Scale the Y axis to the all-time peak (not the window's local
        // max) so a steady footprint reads as a flat line at its true
        // height rather than pegging to the top — the bug heaptrack
        // avoids by using an absolute axis.
        let axis_max_mb = (peak_mb * Y_HEADROOM).max(1.0);
        let (heap_area, heap_line) = self.build_paths(axis_max_mb);

        HudFrame {
            live_mb,
            peak_mb,
            alloc_rate_mb_s,
            alloc_calls_per_s,
            cpu_pct,
            axis_max_mb,
            heap_area,
            heap_line,
        }
    }

    /// Render the history into (filled-area, top-line) SVG command
    /// strings in the `VIEW`×`VIEW` viewbox. `axis_max` is the MB value
    /// mapped to the top of the chart. Returns empty strings until
    /// there are at least two points to connect.
    fn build_paths(&self, axis_max: f64) -> (String, String) {
        let n = self.history.len();
        if n < 2 {
            return (String::new(), String::new());
        }
        let denom = (n - 1) as f64;
        let mut line = String::with_capacity(n * 16);
        for (i, &v) in self.history.iter().enumerate() {
            let x = i as f64 / denom * VIEW;
            // Y grows downward in the viewbox, so invert: full bar = y 0.
            let y = (1.0 - (v / axis_max).clamp(0.0, 1.0)) * VIEW;
            let cmd = if i == 0 { 'M' } else { 'L' };
            // `write!` to a String is infallible.
            let _ = write!(line, "{cmd} {x:.1} {y:.1} ");
        }
        // The area is the line closed down to the baseline and back.
        let area = format!("{line}L {VIEW:.1} {VIEW:.1} L 0 {VIEW:.1} Z");
        (area, line)
    }
}

/// Total process CPU time (user + system) in microseconds, summed
/// across all threads, via `getrusage(RUSAGE_SELF)`. Because it counts
/// every thread, the derived percentage can exceed 100% when the render
/// and loader workers are busy — the honest reading for a multi-threaded
/// browser.
fn process_cpu_us() -> u64 {
    // SAFETY: `rusage` is plain old data; `getrusage` only writes into
    // the struct we pass and cannot fail for `RUSAGE_SELF` with a valid
    // pointer, so the ignored return is sound.
    let usage = unsafe {
        let mut u: libc::rusage = std::mem::zeroed();
        let _ = libc::getrusage(libc::RUSAGE_SELF, &mut u);
        u
    };
    let micros = |t: libc::timeval| t.tv_sec as u64 * 1_000_000 + t.tv_usec as u64;
    micros(usage.ru_utime) + micros(usage.ru_stime)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_wellformed_svg() {
        let mut sampler = HudSampler::new();
        sampler.history.extend([10.0, 20.0, 15.0, 40.0]);
        let (area, line) = sampler.build_paths(50.0);

        // Top line: one moveto, then one lineto per remaining point.
        assert!(line.starts_with("M "), "line must start with moveto: {line}");
        assert_eq!(line.matches('L').count(), 3, "n-1 linetos for n points: {line}");

        // Area: the same curve, dropped to the baseline and closed.
        assert!(area.starts_with("M "), "area must start with moveto: {area}");
        assert!(area.contains("L 1000.0 1000.0"), "area drops to baseline: {area}");
        assert!(area.trim_end().ends_with('Z'), "area must close: {area}");
    }

    #[test]
    fn point_below_axis_max_maps_into_range() {
        let mut sampler = HudSampler::new();
        // Two points: one at the axis max (y≈0, top) and one at zero
        // (y≈1000, baseline). Confirms the inversion and clamp.
        sampler.history.extend([50.0, 0.0]);
        let (_, line) = sampler.build_paths(50.0);
        assert!(line.contains("M 0.0 0.0"), "axis-max maps to the top: {line}");
        assert!(line.contains("L 1000.0 1000.0"), "zero maps to the baseline: {line}");
    }

    #[test]
    fn too_few_points_yield_empty_paths() {
        let mut sampler = HudSampler::new();
        sampler.history.push_back(42.0);
        let (area, line) = sampler.build_paths(50.0);
        assert!(area.is_empty() && line.is_empty(), "need >= 2 points to draw");
    }
}
