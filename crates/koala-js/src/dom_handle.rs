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

thread_local! {
    static CURRENT_DOM: RefCell<Option<DomHandle>> = const { RefCell::new(None) };
}

/// Install `handle` as the current DOM for the calling thread and
/// return a [`DomGuard`] that restores the previous binding when
/// dropped. Wrap script execution like:
///
/// ```ignore
/// let _guard = dom_handle::guard(self.dom.clone());
/// self.context.eval(Source::from_bytes(source))
/// ```
///
/// Nested guards stack correctly: dropping the outer one restores
/// the binding from before the outer guard, not whatever the inner
/// one set.
#[must_use = "the guard restores the previous DOM on drop; bind it to `_guard`"]
pub(crate) fn guard(handle: DomHandle) -> DomGuard {
    let previous = CURRENT_DOM.with(|cell| cell.borrow_mut().replace(handle));
    DomGuard { previous }
}

/// RAII guard returned by [`guard`] that restores the previous DOM
/// binding on drop.
pub(crate) struct DomGuard {
    previous: Option<DomHandle>,
}

impl Drop for DomGuard {
    fn drop(&mut self) {
        let prev = self.previous.take();
        CURRENT_DOM.with(|cell| {
            *cell.borrow_mut() = prev;
        });
    }
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
    CURRENT_DOM.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|handle| f(&handle.borrow()))
    })
}

/// Like [`with_dom`] but exclusive-borrows the tree for mutation.
/// Panics if a read borrow is already outstanding on the same
/// thread — which would indicate a re-entrancy bug in a JS-callable
/// closure that's holding a borrow across a nested script call.
#[allow(dead_code)] // first mutating method lands in the next Phase-2 chunk
pub(crate) fn with_dom_mut<R, F>(f: F) -> Option<R>
where
    F: FnOnce(&mut DomTree) -> R,
{
    CURRENT_DOM.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|handle| f(&mut handle.borrow_mut()))
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
