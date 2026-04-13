//! Private backing-allocation helper shared by `Vec<T>` and (eventually)
//! other collection types in this crate.
//!
//! `RawVec<T>` owns a heap allocation and tracks its capacity in elements,
//! but intentionally knows nothing about element *initialization* or
//! element *count* — those are the concerns of the wrapping collection
//! type. This separation mirrors `std::alloc::RawVec<T>`. The reason the
//! split exists is deduplication: the allocation logic (grow, realloc,
//! overflow handling, ZST quirks, `Drop` that deallocates) is identical
//! across several collection types, and factoring it out lets each
//! collection focus on its own ownership invariants.
//!
//! # Zero-sized types
//!
//! When `size_of::<T>() == 0` the backing storage is effectively
//! infinite: the collection can logically hold up to `usize::MAX`
//! elements, there is nothing to allocate (every element takes zero
//! bytes), and `ptr` is a [`NonNull::dangling`] pointer that must
//! never be dereferenced. `grow` becomes a no-op in this case and
//! `Drop` must not call `dealloc` — attempting to free a dangling
//! pointer is undefined behavior that `miri` will flag immediately.
//!
//! # Panic-on-failure policy
//!
//! Allocation failures call [`handle_alloc_error`], which aborts the
//! process. Capacity overflow (when the requested byte size exceeds
//! `isize::MAX`) panics with a clear message. There is no fallible
//! variant of `grow` — matching `std::Vec`'s infallible API is the
//! deliberate design choice for milestone 1.

use core::marker::PhantomData;
use core::ptr::NonNull;

use alloc::alloc::{Layout, alloc, dealloc, handle_alloc_error, realloc};

/// The backing allocation for a `Vec<T>`-like collection.
///
/// Owns `cap * size_of::<T>()` bytes of heap storage, properly aligned
/// for `T`, or nothing at all when either `cap == 0` or `T` is a
/// zero-sized type. See the module-level documentation for invariants.
///
/// `redundant_pub_crate` fires here because the containing module is
/// private, but we keep `pub(crate)` as an intentional visibility
/// contract: `Vec<T>` in the sibling `vec` module accesses these
/// items, and the visibility is genuinely required for that.
#[allow(clippy::redundant_pub_crate)]
pub(crate) struct RawVec<T> {
    ptr: NonNull<T>,
    cap: usize,
    _marker: PhantomData<T>,
}

#[allow(clippy::redundant_pub_crate)]
impl<T> RawVec<T> {
    /// Minimum capacity on first grow.
    ///
    /// Matches the floor `std::Vec` uses. Without it, the first three
    /// pushes into an empty Vec would each trigger a reallocation
    /// (1 → 2 → 4); with it, a single allocation reaches 4 directly.
    /// The cost is one unused slot for Vecs that end up holding fewer
    /// than four elements; the benefit is O(1) amortized behavior
    /// that actually *feels* O(1) for short-lived small Vecs.
    const MIN_NON_ZERO_CAP: usize = 4;

    /// Construct an empty `RawVec<T>` that has not allocated anything.
    ///
    /// For zero-sized `T`, the capacity is set to `usize::MAX` because
    /// a ZST collection can logically hold the entire address space's
    /// worth of elements without ever allocating.
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    pub(crate) const fn new() -> Self {
        let cap = if size_of::<T>() == 0 {
            usize::MAX
        } else {
            0
        };
        Self {
            ptr: NonNull::dangling(),
            cap,
            _marker: PhantomData,
        }
    }

    /// Construct a `RawVec<T>` with at least the requested capacity.
    ///
    /// For zero-sized `T` this never allocates — the logical capacity
    /// is already `usize::MAX` regardless of `requested`. For `T` with
    /// nonzero size and `requested == 0`, this also does not allocate
    /// and returns the same value as `new()`.
    ///
    /// # Time complexity
    ///
    /// *O*(1) — a single call into the allocator, no per-element work.
    /// The allocator itself may internally do variable work to satisfy
    /// the request, but that cost is outside this function's accounting.
    #[allow(dead_code)] // will be consumed by Vec::with_capacity in v1.1
    pub(crate) fn with_capacity(requested: usize) -> Self {
        if size_of::<T>() == 0 || requested == 0 {
            return Self::new();
        }

        let Ok(layout) = Layout::array::<T>(requested) else {
            capacity_overflow();
        };

        // SAFETY: `layout` has non-zero size because `T` is non-ZST and
        // `requested > 0`, both checked above. `alloc` may return null
        // on allocation failure; we handle that via `handle_alloc_error`
        // on the next line.
        let raw_ptr = unsafe { alloc(layout) };
        let Some(ptr) = NonNull::new(raw_ptr.cast::<T>()) else {
            handle_alloc_error(layout);
        };

        Self {
            ptr,
            cap: requested,
            _marker: PhantomData,
        }
    }

    /// Returns the raw backing pointer.
    ///
    /// The pointer is valid for reads and writes of
    /// `cap * size_of::<T>()` bytes, aligned to `align_of::<T>()`. It
    /// must never be dereferenced past the initialized portion of the
    /// collection — `RawVec` does not know where that boundary is,
    /// which is why element counting lives in the wrapper.
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    pub(crate) const fn ptr(&self) -> NonNull<T> {
        self.ptr
    }

    /// Returns the current capacity in elements.
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    pub(crate) const fn capacity(&self) -> usize {
        self.cap
    }

    /// Grow the backing allocation using the amortized doubling
    /// strategy with a floor of [`Self::MIN_NON_ZERO_CAP`].
    ///
    /// Must not be called when `T` is a zero-sized type: ZSTs already
    /// have `cap == usize::MAX` and there is no growth to perform.
    /// The debug assertion below catches accidental misuse; in release
    /// builds the function will still produce nonsense capacities for
    /// ZSTs, so callers are responsible for checking.
    ///
    /// # Time complexity
    ///
    /// *O*(*n*) worst case, where *n* is the current capacity — a
    /// standard `realloc` that has to move the allocation copies every
    /// existing element. *O*(1) best case when the allocator extends
    /// the allocation in place (which `realloc` may do if the
    /// following address range is free). Amortized *O*(1) per element
    /// across a sequence of `grow` calls because the capacity doubles
    /// each time.
    pub(crate) fn grow(&mut self) {
        debug_assert!(
            size_of::<T>() != 0,
            "RawVec::grow() called with a zero-sized T; capacity is already usize::MAX"
        );

        // Compute the new capacity first, independently of any
        // allocation. If computing the new capacity would overflow, we
        // panic before touching the allocator at all.
        let new_cap = if self.cap == 0 {
            Self::MIN_NON_ZERO_CAP
        } else {
            // `cap * 2` can overflow when `cap > usize::MAX / 2`. We
            // catch that here rather than relying on `Layout::array`
            // to surface it, because the error message is clearer.
            // The `match` form reads more naturally than the clippy-
            // nursery `map_or_else(capacity_overflow, identity)`
            // suggestion, and the two compile identically.
            #[allow(clippy::option_if_let_else)]
            match self.cap.checked_mul(2) {
                Some(n) => n,
                None => capacity_overflow(),
            }
        };

        // `Layout::array` enforces that the total byte size is
        // ≤ `isize::MAX`, so this doubles as the "too-many-bytes"
        // overflow check.
        let Ok(new_layout) = Layout::array::<T>(new_cap) else {
            capacity_overflow();
        };

        let new_ptr = if self.cap == 0 {
            // SAFETY: `new_layout` has non-zero size because `T` is
            // non-ZST (checked at function entry) and `new_cap` is
            // at least `MIN_NON_ZERO_CAP` (≥ 1).
            unsafe { alloc(new_layout) }
        } else {
            // The old layout is whatever `Layout::array` produced for
            // the current `self.cap`. Recomputing it is deterministic:
            // the same type with the same element count always yields
            // the same layout, and that layout was already accepted by
            // the allocator on the previous allocation, so the
            // recomputation cannot fail here.
            let old_layout = Layout::array::<T>(self.cap)
                .expect("a layout that was valid on allocation is still valid on grow");

            // SAFETY: `self.ptr` was allocated by the global allocator
            // with exactly `old_layout`, and is still live because we
            // have exclusive access through `&mut self`. `new_layout`
            // has the same alignment as `old_layout` (both come from
            // `Layout::array::<T>`) and has non-zero size because `T`
            // is non-ZST. `realloc` either returns a new pointer and
            // invalidates `self.ptr`, or returns null leaving
            // `self.ptr` intact — both cases are handled below.
            unsafe {
                realloc(
                    self.ptr.as_ptr().cast::<u8>(),
                    old_layout,
                    new_layout.size(),
                )
            }
        };

        let Some(new_ptr) = NonNull::new(new_ptr.cast::<T>()) else {
            handle_alloc_error(new_layout);
        };

        self.ptr = new_ptr;
        self.cap = new_cap;
    }
}

impl<T> Drop for RawVec<T> {
    fn drop(&mut self) {
        // Two cases where there is nothing to deallocate:
        //
        // 1. `T` is a zero-sized type — we never allocated in the first
        //    place; `self.ptr` is `NonNull::dangling()`. Calling
        //    `dealloc` on a dangling pointer is undefined behavior.
        //
        // 2. `self.cap == 0` — we never allocated. Same reasoning.
        if size_of::<T>() == 0 || self.cap == 0 {
            return;
        }

        // The layout must match what was used at allocation time.
        // See the `grow` commentary for why recomputing is safe.
        let layout = Layout::array::<T>(self.cap)
            .expect("a layout that was valid on allocation is still valid on drop");

        // SAFETY: `self.ptr` was allocated with exactly this layout in
        // `with_capacity` or `grow`, and is still live because Drop
        // has exclusive access to `self`. After this call `self.ptr`
        // is invalidated; since `self` is being dropped, that pointer
        // is never observed again.
        unsafe {
            dealloc(self.ptr.as_ptr().cast::<u8>(), layout);
        }
    }
}

// SAFETY: `RawVec<T>` is semantically "owning heap storage for zero or
// more T" — its thread-safety is identical to `Box<[T]>`, which is
// `Send`/`Sync` iff `T` is. These impls are written manually rather
// than derived because `NonNull<T>` is `!Send + !Sync` even when `T` is
// thread-safe, and that suppresses the automatic derivation. The field
// only exists to track an owned allocation; it does not introduce any
// aliasing that would violate `T`'s own `Send`/`Sync` guarantees.
unsafe impl<T: Send> Send for RawVec<T> {}
unsafe impl<T: Sync> Sync for RawVec<T> {}

/// Panic on capacity overflow.
///
/// Factored out into a `#[cold]` + `#[inline(never)]` function so that
/// the overflow check on the hot path is a single conditional branch
/// without inline panic machinery. The attributes hint to LLVM that
/// this is the unlikely branch and should not be inlined, keeping the
/// hot path tight.
#[cold]
#[inline(never)]
fn capacity_overflow() -> ! {
    panic!("koala_std::RawVec: capacity overflow");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_does_not_allocate_for_non_zst() {
        let raw: RawVec<i32> = RawVec::new();
        assert_eq!(raw.capacity(), 0);
    }

    #[test]
    fn new_sets_capacity_to_usize_max_for_unit_type() {
        let raw: RawVec<()> = RawVec::new();
        assert_eq!(raw.capacity(), usize::MAX);
    }

    #[test]
    fn new_sets_capacity_to_usize_max_for_zero_sized_struct() {
        struct EmptyMarker;
        let raw: RawVec<EmptyMarker> = RawVec::new();
        assert_eq!(raw.capacity(), usize::MAX);
    }

    #[test]
    fn with_capacity_zero_does_not_allocate() {
        let raw: RawVec<i32> = RawVec::with_capacity(0);
        assert_eq!(raw.capacity(), 0);
    }

    #[test]
    fn with_capacity_nonzero_allocates_exact_amount() {
        let raw: RawVec<i32> = RawVec::with_capacity(16);
        assert_eq!(raw.capacity(), 16);
    }

    #[test]
    fn with_capacity_ignores_request_for_zst() {
        // Asking for 1000 ZST slots should not allocate, and the
        // reported capacity stays at the ZST sentinel of usize::MAX.
        let raw: RawVec<()> = RawVec::with_capacity(1000);
        assert_eq!(raw.capacity(), usize::MAX);
    }

    #[test]
    fn grow_from_zero_jumps_to_min_non_zero_cap() {
        // First grow must not land on 1 or 2 — it goes straight to the
        // floor of 4 to avoid three reallocations for a four-element
        // push loop.
        let mut raw: RawVec<i32> = RawVec::new();
        raw.grow();
        assert_eq!(raw.capacity(), 4);
    }

    #[test]
    fn grow_doubles_existing_capacity() {
        let mut raw: RawVec<i32> = RawVec::with_capacity(4);
        raw.grow();
        assert_eq!(raw.capacity(), 8);
        raw.grow();
        assert_eq!(raw.capacity(), 16);
        raw.grow();
        assert_eq!(raw.capacity(), 32);
    }

    #[test]
    fn repeated_grow_does_not_leak_under_miri() {
        // This test exists primarily to exercise `grow` + `Drop`
        // interaction under `miri`. A leaked or double-freed allocation
        // will surface here when the CI miri job runs.
        let mut raw: RawVec<u64> = RawVec::new();
        for _ in 0..10 {
            raw.grow();
        }
        // Drop at end of scope — miri verifies it matches the
        // allocator call.
    }

    #[test]
    fn drop_empty_does_not_dealloc_dangling_pointer() {
        // If `Drop` ever called `dealloc` on a cap=0 RawVec, miri would
        // immediately flag a use of `NonNull::dangling()` as UB.
        let _raw: RawVec<i32> = RawVec::new();
    }

    #[test]
    fn drop_zst_does_not_dealloc_dangling_pointer() {
        // Same as above, but for ZSTs — the capacity is usize::MAX but
        // no allocation was ever performed, so dealloc would be UB.
        let _raw: RawVec<()> = RawVec::new();
    }

    #[test]
    #[should_panic(expected = "capacity overflow")]
    fn with_capacity_of_usize_max_panics_on_u64() {
        // `Layout::array::<u64>(usize::MAX)` exceeds `isize::MAX`
        // bytes and returns an error, which we translate into a panic
        // via `capacity_overflow`. This test confirms the error path
        // is wired up rather than silently producing a bogus
        // allocation.
        let _raw: RawVec<u64> = RawVec::with_capacity(usize::MAX);
    }

    #[test]
    fn ptr_for_aligned_zst_respects_type_alignment() {
        // `NonNull::dangling()` produces a pointer whose address is
        // `align_of::<T>()`. Checking non-nullness is redundant —
        // `NonNull` guarantees it at the type level — but the
        // alignment is a real invariant we want to confirm, because a
        // misaligned dangling pointer could break downstream slice
        // construction even though it would never be dereferenced.
        #[repr(align(16))]
        struct Aligned16;

        let raw: RawVec<Aligned16> = RawVec::new();
        let addr = raw.ptr().as_ptr() as usize;
        assert_eq!(
            addr % 16,
            0,
            "NonNull::dangling() must be aligned to align_of::<T>()"
        );
        assert_eq!(raw.capacity(), usize::MAX);
    }
}
