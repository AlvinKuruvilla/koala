// A worker: a named thread with a channel in each direction, owned by a
// handle on the GUI thread.
//
// `Worker::spawn` starts the thread with a stack large enough for Boa
// (see `WORKER_STACK_BYTES`) and hands its body an `Inbox` to receive from
// and a `Sender` to reply on. The GUI thread keeps the `Worker` and talks
// to it with `send` and `try_recv`, neither of which blocks. Dropping the
// `Worker` closes the inbox, which ends the body's receive loop.
//
// The body is ordinary code with its own loop. `Inbox::recv_batch` gives
// a body that coalesces input everything that has queued up since it last
// looked, and `catch_panic` turns a panic in engine code into an error
// message the body can report instead of a dead thread.
//
// # Trade-offs
//
// Each body still writes its own receive loop. The alternative is a trait
// (`fn handle(&mut self, batch) -> Option<Out>`) and one generic loop that
// drives every worker. That was rejected while the two workers disagree on
// exactly what such a loop would fix: the loader handles requests one at a
// time, the render worker in batches, and each recovers from a panic
// differently. Revisit as workers are added (a DevTools server, a JS
// thread): if their loops settle into a few fixed shapes, the trait would
// make each new worker a single `handle` method.

use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

/// Stack size for every worker: 16 MiB, the size Boa gives its own CLI
/// for the same reason (boa-dev/boa#5537).
///
/// Boa parses JavaScript by recursive descent with no depth limit, about
/// fifteen frames per level of expression nesting. Measured 2026-09-26
/// on google.com's 1.1 MB script bundle, whose deepest nesting is only 19
/// brackets: a debug build needs 2-4 MiB (frames up to 20 KB each), a
/// release build 256-512 KiB. Rust's default for spawned threads is
/// 2 MiB, so debug builds overflowed and aborted the whole process.
///
/// 16 MiB leaves 4x headroom over debug google. The size only reserves
/// address space; memory is committed as the stack is actually used.
///
/// NOTE: this raises the threshold, it does not remove it. A script nested
/// deeply enough still overflows any stack and aborts the process, because
/// Boa's parser has no recursion guard. The real fix is one in the parser
/// (JavaScriptCore's turns this into a `RangeError`).
const WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;

/// The GUI thread's handle on a worker thread that takes `In` messages and
/// replies with `Out`.
pub(crate) struct Worker<In, Out> {
    inbox: Sender<In>,
    outbox: Receiver<Out>,
    // A sender on the worker's own outbox, for `post_as_worker`.
    outbox_sender: Sender<Out>,
    _thread: JoinHandle<()>,
}

impl<In: Send + 'static, Out: Send + 'static> Worker<In, Out> {
    /// Start a thread named `name` running `body`.
    ///
    /// # Panics
    ///
    /// If the operating system cannot create the thread.
    pub(crate) fn spawn(
        name: &str,
        body: impl FnOnce(Inbox<In>, Sender<Out>) + Send + 'static,
    ) -> Self {
        let (inbox, inbox_receiver) = mpsc::channel();
        let (outbox_sender, outbox) = mpsc::channel();
        let replies = outbox_sender.clone();
        let thread = thread::Builder::new()
            .name(name.to_owned())
            .stack_size(WORKER_STACK_BYTES)
            .spawn(move || body(Inbox(inbox_receiver), replies))
            .unwrap_or_else(|e| panic!("cannot spawn the {name} thread: {e}"));
        Self {
            inbox,
            outbox,
            outbox_sender,
            _thread: thread,
        }
    }

    /// Queue `message` for the worker. Returns `false` if the worker has
    /// stopped.
    pub(crate) fn send(&self, message: In) -> bool {
        self.inbox.send(message).is_ok()
    }

    /// The oldest reply not yet taken, if any. Does not block.
    pub(crate) fn try_recv(&self) -> Option<Out> {
        self.outbox.try_recv().ok()
    }

    /// Queue `reply` as though the worker had sent it, so the GUI thread
    /// can feed a result it produced itself through the same path as the
    /// worker's.
    pub(crate) fn post_as_worker(&self, reply: Out) {
        // The receiver is `self.outbox`, alive as long as `self` is.
        let _ = self.outbox_sender.send(reply);
    }
}

/// A worker's end of its inbox.
pub(crate) struct Inbox<In>(Receiver<In>);

impl<In> Inbox<In> {
    /// Wait for the next message. `None` once the `Worker` is dropped,
    /// which is the body's cue to return.
    pub(crate) fn recv(&self) -> Option<In> {
        self.0.recv().ok()
    }

    /// Wait for the next message, then take every other message already
    /// queued behind it, oldest first. `None` once the `Worker` is dropped.
    pub(crate) fn recv_batch(&self) -> Option<impl Iterator<Item = In> + '_> {
        let first = self.0.recv().ok()?;
        Some(std::iter::once(first).chain(self.0.try_iter()))
    }
}

/// Run `f`, turning a panic into its message.
///
/// The closure is asserted unwind-safe: after a panic the caller must
/// throw away whatever `f` was mutating rather than read it again, since
/// it may be half-updated. Both workers do (the loader drops the
/// document, the render worker drops the page).
pub(crate) fn catch_panic<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    catch_unwind(AssertUnwindSafe(f)).map_err(|payload| panic_message(&payload))
}

/// Best-effort extraction of a human-readable message from a panic
/// payload. `catch_unwind` hands us a `Box<dyn Any>`, so we try the
/// two concrete types the standard library uses before falling back
/// to a placeholder.
fn panic_message(payload: &Box<dyn Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = payload.downcast_ref::<&'static str>() {
        (*s).to_owned()
    } else {
        "engine panicked (no message)".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use super::{Worker, catch_panic};

    const WAIT: Duration = Duration::from_secs(5);

    /// Each reply is the size of the batch the body received. A body that
    /// is held until three messages are queued sees them as one batch.
    #[test]
    fn recv_batch_takes_everything_queued() {
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let worker = Worker::<u32, usize>::spawn("test-batch", move |inbox, replies| {
            release_rx.recv().expect("released by the test");
            while let Some(batch) = inbox.recv_batch() {
                let _ = replies.send(batch.count());
            }
        });
        for message in 0..3 {
            assert!(worker.send(message));
        }
        release_tx.send(()).expect("worker waiting");

        let deadline = std::time::Instant::now() + WAIT;
        let reply = loop {
            if let Some(reply) = worker.try_recv() {
                break reply;
            }
            assert!(std::time::Instant::now() < deadline, "no reply");
            std::thread::yield_now();
        };
        assert_eq!(reply, 3);
    }

    #[test]
    fn catch_panic_returns_the_message() {
        assert_eq!(catch_panic(|| 7), Ok(7));
        assert_eq!(catch_panic(|| -> u8 { panic!("broken page") }), Err("broken page".to_owned()));
        let index = 9;
        assert_eq!(
            catch_panic(|| -> u8 { panic!("index {index}") }),
            Err("index 9".to_owned())
        );
    }

    #[test]
    fn post_as_worker_arrives_like_a_reply() {
        let worker = Worker::<(), &str>::spawn("test-post", |_inbox, _replies| {});
        worker.post_as_worker("synthetic");
        assert_eq!(worker.try_recv(), Some("synthetic"));
    }

    /// Dropping the handle closes the inbox, so a body's receive loop ends
    /// rather than leaving the thread blocked forever.
    #[test]
    fn dropping_the_worker_ends_its_loop() {
        let (done_tx, done_rx) = mpsc::channel();
        let worker = Worker::<(), ()>::spawn("test-drop", move |inbox, _replies| {
            while inbox.recv().is_some() {}
            let _ = done_tx.send(());
        });
        drop(worker);
        assert!(done_rx.recv_timeout(WAIT).is_ok(), "loop did not end");
    }
}
