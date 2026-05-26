//! Process-thread-local DOM handle access for JS globals.
//!
//! [§ 4.5 Interface Document](https://dom.spec.whatwg.org/#interface-document)
//!
//! Boa's `NativeFunction::from_copy_closure` requires the closure
//! environment to be `Copy + 'static`, which `Rc<RefCell<DomTree>>`
//! cannot satisfy. `from_closure_with_captures` does support
//! captures, but they must implement `boa_gc::Trace`, which the
//! standard `Rc` and `RefCell` don't.
//!
//! Rather than wrapping the handle in a custom `Trace` shim, we
//! park it in a thread-local for the duration of each script
//! execution. Every DOM-touching closure (`getElementById`,
//! `getAttribute`, …) is then a plain `Fn + Copy` that reads the
//! thread-local. The trade-off is that only one [`JsRuntime`] may
//! execute scripts on a given thread at a time — fine for koala's
//! single-document-per-process model and easily extensible later
//! via Boa's host-data slots if it ever needs to change.
//!
//! [`JsRuntime`]: crate::JsRuntime

use std::cell::RefCell;
use std::rc::Rc;

use koala_dom::DomTree;

/// Shared handle to a DOM tree. Cloning is cheap (`Rc` bump). Held
/// by [`JsRuntime`](crate::JsRuntime) for the lifetime of the
/// runtime and exposed to JS-callable closures via a thread-local
/// installed by [`guard`] around each script execution.
pub type DomHandle = Rc<RefCell<DomTree>>;

/// Per-thread script-execution context: the current DOM handle plus
/// a dirty flag that gets flipped on by any mutation method
/// (`setAttribute`, `appendChild`, `textContent` setter, …). The
/// flag is captured on [`DomGuard`] install/teardown so each
/// script invocation has its own dirty window — see
/// [`DomGuard::dirty_seen`].
struct DomContext {
    handle: DomHandle,
    dirty: bool,
}

thread_local! {
    static CURRENT: RefCell<Option<DomContext>> = const { RefCell::new(None) };
}

/// Install `handle` as the current DOM for the calling thread, with
/// a fresh dirty=false flag, and return a [`DomGuard`] that
/// restores the previous binding on drop. Wrap script execution
/// like:
///
/// ```ignore
/// let guard = dom_handle::guard(self.dom.clone());
/// let result = self.context.eval(Source::from_bytes(source));
/// let did_mutate = guard.dirty_seen();
/// drop(guard);
/// ```
///
/// Nested guards stack correctly: the inner guard sees its own
/// fresh dirty window; dropping it restores the outer window's
/// previous dirty state, so the outer caller's dirty-tracking is
/// not affected by a nested script's mutations (which is the right
/// thing — those mutations belong to the inner script's
/// re-layout decision, not the outer one's).
#[must_use = "the guard restores the previous DOM on drop; bind it to `_guard`"]
pub(crate) fn guard(handle: DomHandle) -> DomGuard {
    let previous = CURRENT.with(|cell| {
        cell.borrow_mut()
            .replace(DomContext { handle, dirty: false })
    });
    DomGuard { previous }
}

/// RAII guard returned by [`guard`] that restores the previous DOM
/// context on drop. Use [`Self::dirty_seen`] before dropping if you
/// want to know whether any mutation closure flipped the dirty flag
/// during the guard's window.
pub(crate) struct DomGuard {
    previous: Option<DomContext>,
}

impl DomGuard {
    /// True if any mutation method ran [`mark_dirty`] since this
    /// guard was installed. Reads the current thread-local context
    /// rather than the captured `previous` field — by construction
    /// the guard's *own* context is what's installed right now.
    #[allow(clippy::unused_self)] // method belongs to DomGuard by design (clears at scope exit)
    pub(crate) fn dirty_seen(&self) -> bool {
        CURRENT.with(|cell| cell.borrow().as_ref().is_some_and(|c| c.dirty))
    }
}

impl Drop for DomGuard {
    fn drop(&mut self) {
        let prev = self.previous.take();
        CURRENT.with(|cell| {
            *cell.borrow_mut() = prev;
        });
    }
}

/// Mark the current DOM context as having been mutated. Called by
/// every DOM-bridge method that changes the tree's observable
/// state (attributes, child lists, text data). The flag is read
/// by [`DomGuard::dirty_seen`] after the JS script returns.
///
/// No-op outside a [`guard`]-protected scope; that path is mainly
/// hit by direct unit tests of helpers, where the caller doesn't
/// care about layout invalidation.
pub(crate) fn mark_dirty() {
    CURRENT.with(|cell| {
        if let Some(ctx) = cell.borrow_mut().as_mut() {
            ctx.dirty = true;
        }
    });
}

/// Run `f` with a borrow of the thread's current DOM. Returns
/// `None` when called outside a [`guard`]-protected scope, which
/// can happen if a closure leaks past the [`JsRuntime`] that
/// installed it (shouldn't, but the API stays safe either way).
///
/// The closure receives `&DomTree`; for mutation see
/// [`with_dom_mut`].
///
/// [`JsRuntime`]: crate::JsRuntime
pub(crate) fn with_dom<R, F>(f: F) -> Option<R>
where
    F: FnOnce(&DomTree) -> R,
{
    CURRENT.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|ctx| f(&ctx.handle.borrow()))
    })
}

/// Like [`with_dom`] but exclusive-borrows the tree for mutation.
/// Panics if a read borrow is already outstanding on the same
/// thread — which would indicate a re-entrancy bug in a JS-callable
/// closure that's holding a borrow across a nested script call.
pub(crate) fn with_dom_mut<R, F>(f: F) -> Option<R>
where
    F: FnOnce(&mut DomTree) -> R,
{
    CURRENT.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|ctx| f(&mut ctx.handle.borrow_mut()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use koala_dom::{DomTree, NodeType};

    #[test]
    fn with_dom_returns_none_without_guard() {
        let read = with_dom(|_| 1);
        assert!(read.is_none());
    }

    #[test]
    fn guard_installs_and_restores() {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let _g = guard(Rc::clone(&tree));
            let count = with_dom(|d| d.iter_all().count()).unwrap();
            assert!(count >= 1, "document root should be visible");
        }
        assert!(with_dom(|_| ()).is_none(), "binding restored on drop");
    }

    #[test]
    fn dirty_flag_isolated_per_guard() {
        let outer = Rc::new(RefCell::new(DomTree::new()));
        let inner = Rc::new(RefCell::new(DomTree::new()));

        let g_outer = guard(Rc::clone(&outer));
        assert!(!g_outer.dirty_seen());
        {
            let g_inner = guard(Rc::clone(&inner));
            mark_dirty();
            assert!(g_inner.dirty_seen(), "inner guard sees its own dirty flip");
        }
        // The inner guard dropped restoring the outer context; the
        // outer guard should not have inherited the inner's dirty.
        assert!(
            !g_outer.dirty_seen(),
            "outer guard's dirty state is unaffected by inner mutations",
        );

        mark_dirty();
        assert!(g_outer.dirty_seen(), "outer flips when its own scope mutates");
    }

    #[test]
    fn nested_guards_restore_outer_handle() {
        // Attach a Text child to inner's root so iter_all sees it
        // — orphan nodes aren't reachable from the document root.
        let outer = Rc::new(RefCell::new(DomTree::new()));
        let inner = {
            let mut t = DomTree::new();
            let root = t.root();
            let txt = t.alloc(NodeType::Text("inner".into()));
            t.append_child(root, txt);
            Rc::new(RefCell::new(t))
        };

        let _g1 = guard(Rc::clone(&outer));
        let outer_nodes = with_dom(|d| d.iter_all().count()).unwrap();
        {
            let _g2 = guard(Rc::clone(&inner));
            let inner_nodes = with_dom(|d| d.iter_all().count()).unwrap();
            assert!(
                inner_nodes > outer_nodes,
                "inner ({inner_nodes}) should outnumber outer ({outer_nodes})",
            );
        }
        let restored = with_dom(|d| d.iter_all().count()).unwrap();
        assert_eq!(restored, outer_nodes, "outer handle restored");
    }
}
