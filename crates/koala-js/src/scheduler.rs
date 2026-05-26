//! Per-thread timer scheduler for `setTimeout` / `setInterval` and
//! their cancel counterparts.
//!
//! [§ 8.6 Timers](https://html.spec.whatwg.org/multipage/timers-and-user-prompts.html#timers)
//!
//! The scheduler holds the "when is each timer due?" bookkeeping in
//! plain Rust state, and identifies each pending timer by a `u32`
//! id. The actual JS callbacks live in a hidden `__koala_timers__`
//! global `Array` on Boa's global object — that array is what keeps
//! the callbacks reachable from Boa's GC roots, so we don't need
//! `JsFunction`s to outlive a `Context` or to participate in
//! tracing from plain Rust state.
//!
//! One-shot timers carry `repeat = None`; intervals carry
//! `repeat = Some(period)`. The pump re-schedules an interval with
//! the same id after each firing, so [`cancel`] applies uniformly
//! to both kinds — `clearTimeout(intervalId)` and
//! `clearInterval(timeoutId)` both work, matching the spec's
//! shared id pool.
//!
//! Like [`crate::dom_handle`], the scheduler is exposed to
//! JS-callable closures (`setTimeout` etc.) via a thread-local that
//! [`JsRuntime`] installs around `execute` / `pump_until_idle`.
//!
//! [`JsRuntime`]: crate::JsRuntime

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// Stable identifier returned by `setTimeout` and consumed by
/// `clearTimeout`. Also doubles as the index into the
/// `__koala_timers__` JS-side callback array.
pub type TimerId = u32;

/// Bookkeeping for one pending timer.
///
/// `repeat = None` is a one-shot (`setTimeout`); `repeat = Some(d)`
/// is an interval that the pump re-arms at `now + d` after each
/// firing.
#[derive(Debug)]
struct PendingTimer {
    id: TimerId,
    repeat: Option<Duration>,
}

/// Per-thread timer state. Cancelled timers are dropped lazily on
/// pop, so `cancel()` is O(log n) (just records the id in a
/// hash-set) rather than scanning the queue.
///
/// Ids are assigned by the caller (it's the index into the
/// JS-side `__koala_timers__` array, offset by +1 so id `0` stays
/// usable as a "no timer" sentinel for `clearTimeout(undefined)`).
#[derive(Default)]
struct Scheduler {
    /// Timers keyed by their absolute due time. `Vec` per slot
    /// handles the (rare) case of two timers due at the same
    /// instant.
    pending: BTreeMap<Instant, Vec<PendingTimer>>,
    /// Cancellations applied lazily on pop. Using a `Vec`
    /// (small-N hash-free) since the cancellation rate in practice
    /// is low and lookups during pop hit at most a handful of ids.
    cancelled: Vec<TimerId>,
}

thread_local! {
    static SCHEDULER: RefCell<Option<Scheduler>> = const { RefCell::new(None) };
}

/// Install an empty scheduler for the calling thread, returning a
/// [`SchedulerGuard`] that tears it down on drop. Mirrors
/// [`crate::dom_handle::guard`].
#[must_use = "the guard tears down the scheduler on drop; bind to `_guard`"]
pub(crate) fn guard() -> SchedulerGuard {
    let previous = SCHEDULER.with(|cell| cell.borrow_mut().replace(Scheduler::default()));
    SchedulerGuard { previous }
}

pub(crate) struct SchedulerGuard {
    previous: Option<Scheduler>,
}

impl Drop for SchedulerGuard {
    fn drop(&mut self) {
        let prev = self.previous.take();
        SCHEDULER.with(|cell| {
            *cell.borrow_mut() = prev;
        });
    }
}

/// Register a timer to fire at `Instant::now() + delay` with the
/// caller-supplied `id`. The id is the JS-visible value returned
/// from `setTimeout` / `setInterval`; the caller is responsible
/// for keeping it in sync with whatever storage holds the JS
/// callback (in koala's case, the index+1 into the
/// `__koala_timers__` array).
///
/// `repeat = None` makes this a one-shot. `repeat = Some(period)`
/// makes it an interval — after the pump fires the callback it
/// calls [`schedule`] again with the same id, `period`, and
/// `repeat`, keeping the slot live across firings.
pub(crate) fn schedule(id: TimerId, delay: Duration, repeat: Option<Duration>) {
    SCHEDULER.with(|cell| {
        let mut guard = cell.borrow_mut();
        let Some(sched) = guard.as_mut() else { return };
        let due = Instant::now() + delay;
        sched
            .pending
            .entry(due)
            .or_default()
            .push(PendingTimer { id, repeat });
    });
}

/// Mark a timer as cancelled. The next `pop_due_now` call will
/// drop it before invoking its callback.
pub(crate) fn cancel(id: TimerId) {
    SCHEDULER.with(|cell| {
        if let Some(sched) = cell.borrow_mut().as_mut() {
            sched.cancelled.push(id);
        }
    });
}

/// Earliest `Instant` at which any pending timer is due, or `None`
/// when the queue is empty. Used by the pump loop to decide
/// whether to sleep.
pub(crate) fn next_due_time() -> Option<Instant> {
    SCHEDULER.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|s| s.pending.keys().next().copied())
    })
}

/// Pop every timer whose due time is `<= now()`. Filters out
/// cancelled ids. Returns the surviving `(TimerId, repeat)` pairs
/// in tree (i.e. chronological) order — callers iterate, invoke
/// each callback, and re-call [`schedule`] for any pair whose
/// `repeat` is `Some` to keep the interval running.
pub(crate) fn pop_due_now() -> Vec<(TimerId, Option<Duration>)> {
    SCHEDULER.with(|cell| {
        let mut guard = cell.borrow_mut();
        let Some(sched) = guard.as_mut() else { return Vec::new() };
        let now = Instant::now();

        let mut due_keys: Vec<Instant> = Vec::new();
        for key in sched.pending.keys() {
            if *key <= now {
                due_keys.push(*key);
            } else {
                break;
            }
        }

        let mut out = Vec::new();
        for key in due_keys {
            if let Some(bucket) = sched.pending.remove(&key) {
                for timer in bucket {
                    if !sched.cancelled.contains(&timer.id) {
                        out.push((timer.id, timer.repeat));
                    }
                }
            }
        }
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_orders_by_due_time() {
        let _g = guard();
        schedule(1, Duration::from_millis(50), None);
        schedule(2, Duration::from_millis(10), None);
        schedule(3, Duration::from_millis(30), None);
        let now = Instant::now();
        let next = next_due_time().unwrap();
        assert!(next >= now);
        assert!(next - now < Duration::from_millis(50));
    }

    #[test]
    fn cancel_skips_callback_on_pop() {
        let _g = guard();
        schedule(7, Duration::from_millis(0), None);
        cancel(7);
        std::thread::sleep(Duration::from_millis(1));
        let popped = pop_due_now();
        assert!(popped.is_empty(), "cancelled timer should not pop");
    }

    #[test]
    fn pop_due_now_only_returns_passed_due_times() {
        let _g = guard();
        schedule(1, Duration::from_millis(0), None);
        schedule(2, Duration::from_secs(60), None);
        std::thread::sleep(Duration::from_millis(1));
        let popped = pop_due_now();
        assert_eq!(popped, vec![(1, None)], "only the +0ms timer should be due");
        assert!(next_due_time().is_some(), "+60s timer still pending");
    }

    #[test]
    fn pop_due_now_reports_interval_repeat_period() {
        let _g = guard();
        let period = Duration::from_millis(25);
        schedule(9, Duration::from_millis(0), Some(period));
        std::thread::sleep(Duration::from_millis(1));
        let popped = pop_due_now();
        assert_eq!(
            popped,
            vec![(9, Some(period))],
            "interval should pop with its repeat period so the caller can re-arm it",
        );
    }
}
