//! The clock a page's scripts see: real time, except that idle gaps
//! before the next timer are skipped instead of slept through.
//!
//! During a load nothing can wake a page except its own timers: fetches
//! are synchronous, and there are no workers, network callbacks, or input
//! events. Sleeping until the next timer is due and jumping the clock to
//! that moment are therefore indistinguishable from inside the page, and
//! the jump costs no wall time. Timer due times and `Date.now()` both read
//! this clock, so a page that measures elapsed time around a timer sees
//! the delay it asked for.
//!
//! Between jumps the clock runs at real speed, so a script that busy-waits
//! on `Date.now()` still sees time pass.
//!
//! Headless Chrome offers the same idea as `--virtual-time-budget`.
//!
//! NOTE: skipping is only invisible while timers are the page's sole
//! wake-up source. Once koala has async fetch, workers, or user input,
//! a real event could be due inside a skipped gap, and the clock must stop
//! skipping while any such work is pending.

use std::cell::Cell;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use boa_engine::context::time::JsInstant;

/// Real elapsed time plus every gap skipped so far.
pub(crate) struct VirtualClock {
    origin: Instant,
    /// Wall-clock time at `origin`, in ms since the Unix epoch; the base
    /// for `Date.now()`.
    wall_origin_ms: i64,
    skipped: Cell<Duration>,
}

impl VirtualClock {
    pub(crate) fn new() -> Self {
        let since_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("the system clock is after 1970");
        Self {
            origin: Instant::now(),
            wall_origin_ms: i64::try_from(since_epoch.as_millis())
                .expect("milliseconds since 1970 fit in i64 for 292 million years"),
            skipped: Cell::new(Duration::ZERO),
        }
    }

    /// Time since the clock was created, skipped gaps included.
    pub(crate) fn elapsed(&self) -> Duration {
        self.origin.elapsed() + self.skipped.get()
    }

    /// Move the clock forward to `target` without waiting. A `target`
    /// already in the past leaves the clock alone; it never runs backward.
    pub(crate) fn skip_to(&self, target: Duration) {
        if let Some(gap) = target.checked_sub(self.elapsed()) {
            self.skipped.set(self.skipped.get() + gap);
        }
    }
}

impl boa_engine::context::Clock for VirtualClock {
    fn now(&self) -> JsInstant {
        let elapsed = self.elapsed();
        JsInstant::new(elapsed.as_secs(), elapsed.subsec_nanos())
    }

    fn system_time_millis(&self) -> i64 {
        let elapsed_ms = i64::try_from(self.elapsed().as_millis())
            .expect("milliseconds since the runtime started fit in i64");
        self.wall_origin_ms + elapsed_ms
    }
}
