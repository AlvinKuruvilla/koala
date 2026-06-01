//! Low-level allocation primitives shared by the crate's backing
//! stores.
//!
//! Both `vec::raw_vec::RawVec<T>` and
//! `collections::raw_table::RawTable<K, V>` need the same opening
//! move: ask the allocator for a contiguous, correctly-aligned run
//! of bytes big enough for `n` values of some type, turn the raw
//! byte pointer into a typed [`NonNull`], and abort cleanly if the
//! request either overflows the address space or the allocator
//! refuses it. That sequence is a small amount of `unsafe` whose
//! correctness depends on a handful of preconditions; duplicating
//! it per backing store would mean re-auditing the same `unsafe`
//! in every copy. Centralizing it here means one reviewed `unsafe`
//! block and one place for the SAFETY contract to live.
//!
//! What deliberately does *not* live here is *policy*: whether a
//! given backing store skips allocation for zero-sized types or a
//! zero count, and how it derives the element count it asks for
//! (a `Vec` uses the requested count as-is; a hash table inflates
//! it by load factor and rounds to a power of two). Those choices
//! belong to the caller. This module only answers the narrow
//! question "give me memory for `count` values of `T`," and trusts
//! the caller to have decided that `count` is the right number and
//! greater than zero.

use core::ptr::NonNull;

use alloc::alloc::{Layout, alloc, alloc_zeroed, dealloc, handle_alloc_error};

/// Abort the current operation because a requested capacity cannot
/// be represented.
///
/// Called when the byte size of a requested allocation would exceed
/// what a `Layout` can describe (`isize::MAX` bytes). This is a hard
/// stop rather than a recoverable error: a collection that cannot
/// size its own backing has no sensible state to return to. The
/// message is intentionally generic — it is shared by every backing
/// store in the crate, so it names neither `RawVec` nor `RawTable`.
///
/// Factored out into a `#[cold]` + `#[inline(never)]` function so the
/// overflow check on a backing store's hot path stays a single
/// conditional branch without inline panic machinery. The attributes
/// hint to LLVM that this is the unlikely branch and should not be
/// inlined, keeping the hot path tight.
#[cold]
#[inline(never)]
pub(crate) fn capacity_overflow() -> ! {
    panic!("koala_std: capacity overflow");
}

/// Allocate backing storage for `count` values of `T` and return a
/// non-null, `T`-aligned pointer to the start of the run.
///
/// When `zeroed` is `true` the bytes come from `alloc_zeroed` and
/// every byte is guaranteed `0`; when `false` they come from
/// `alloc` and are uninitialized. The choice is the caller's: a
/// `Vec` does not care (it tracks an initialized length and never
/// reads past it), whereas a hash table relies on a zeroed block
/// already encoding an all-empty table, because its empty-slot
/// marker is the zero byte.
///
/// The returned pointer is valid for reads and writes of
/// `count * size_of::<T>()` bytes. Ownership of the allocation
/// transfers to the caller, which becomes responsible for
/// eventually `dealloc`-ing it with the same `Layout` this function
/// used (`Layout::array::<T>(count)`).
///
/// # Panics
///
/// Panics via [`capacity_overflow`] if `count * size_of::<T>()`
/// overflows `isize::MAX` bytes — i.e. if `Layout::array::<T>(count)`
/// fails.
///
/// # Aborts
///
/// If the allocator cannot satisfy the request it returns null, and
/// this routes through `handle_alloc_error`, which aborts the
/// process. Allocation failure is not a recoverable condition here.
///
/// # Caller obligations
///
/// `count` must be greater than zero and `T` must not be
/// zero-sized. Both would make `Layout::array::<T>(count)` describe
/// a zero-byte allocation, which is a precondition violation for
/// `alloc`/`alloc_zeroed`. Callers that can produce a zero count or
/// a ZST must short-circuit to their no-allocation state (a
/// dangling pointer) before calling this function.
///
/// # Time complexity
///
/// *O*(*n*) in `count` when `zeroed` is `true` (the allocator must
/// zero the run); *O*(1) plus the allocator's own bookkeeping when
/// `false`.
pub(crate) fn alloc_array<T>(count: usize, zeroed: bool) -> NonNull<T> {
    // Translate the element count into a byte layout. `Layout::array`
    // is where capacity overflow is detected: it returns `Err` when
    // `count * size_of::<T>()` would exceed `isize::MAX`.
    let Ok(layout) = Layout::array::<T>(count) else {
        capacity_overflow();
    };

    // SAFETY: the caller guarantees `count > 0` and `T` is non-ZST
    // (see "Caller obligations"), so `layout` describes a non-zero
    // number of bytes — the precondition `alloc`/`alloc_zeroed`
    // require. A null return signals allocation failure, handled via
    // `handle_alloc_error` below
    let raw_ptr = unsafe {
        if zeroed {
            alloc_zeroed(layout)
        } else {
            alloc(layout)
        }
    };
    let Some(ptr) = NonNull::new(raw_ptr.cast::<T>()) else {
        handle_alloc_error(layout);
    };
    ptr
}

/// Free a `count`-element run of `T` previously obtained from
/// [`alloc_array`].
///
/// This is the inverse of [`alloc_array`]: it reconstructs the exact
/// `Layout::array::<T>(count)` that the allocation used and returns the
/// block to the global allocator. Routing both sides of the allocation
/// through this module means the layout is computed by one piece of
/// code, so an allocate/free layout mismatch — which is instant
/// undefined behavior — cannot arise from the two sites drifting apart.
///
/// # Safety
///
/// - `ptr` must have come from [`alloc_array::<T>`] with the *same*
///   `count`, and must not have been freed already.
/// - After this call the allocation is gone: `ptr` is dangling and must
///   never be read, written, or freed again.
///
/// `count` is necessarily the same non-zero value originally passed to
/// `alloc_array` (a zero count or ZST never allocates), so
/// `Layout::array::<T>(count)` cannot fail here — the `expect` documents
/// an unreachable branch rather than a real failure mode.
///
/// # Time complexity
///
/// *O*(1) plus the allocator's own bookkeeping.
pub(crate) unsafe fn dealloc_array<T>(ptr: NonNull<T>, count: usize) {
    let layout =
        Layout::array::<T>(count).expect("layout was valid when alloc_array succeeded");
    // SAFETY: by the contract above, `ptr` came from `alloc_array::<T>`
    // with this same `count` (hence this same layout) and has not been
    // freed. Casting to `*mut u8` matches what the global allocator
    // originally handed out.
    unsafe {
        dealloc(ptr.as_ptr().cast::<u8>(), layout);
    }
}
