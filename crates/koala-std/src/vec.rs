//! A contiguous growable array type with heap-allocated contents.
//!
//! This module provides [`Vec<T>`], `koala-std`'s hand-rolled
//! counterpart to `std::vec::Vec<T>`. It is built on the private
//! `RawVec<T>` helper which owns the allocation and tracks capacity;
//! `Vec<T>` adds the element count on top.

use core::fmt;
use core::hash::{Hash, Hasher};
use core::ops::{Deref, DerefMut};
use core::ptr;
use core::slice;

use crate::raw_vec::RawVec;

/// A contiguous growable array type, written `Vec<T>` and pronounced
/// "vector".
///
/// `koala-std`'s hand-rolled counterpart to `std::vec::Vec<T>`. Owns a
/// heap allocation (managed internally by `RawVec<T>`) plus a `len`
/// field tracking how many elements of that allocation are currently
/// initialized.
///
/// # Invariant
///
/// `self.len <= self.buf.capacity()`, and slots `[0, self.len)`
/// contain initialized values of type `T` while slots
/// `[self.len, self.buf.capacity())` are uninitialized memory. Every
/// method must re-establish both halves of this invariant before
/// returning.
pub struct Vec<T> {
    buf: RawVec<T>,
    len: usize,
}

impl<T> Vec<T> {
    /// Constructs a new, empty `Vec<T>`.
    ///
    /// The vector will not allocate until elements are pushed onto it.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::vec::Vec;
    /// let mut v: Vec<i32> = Vec::new();
    /// assert_eq!(v.len(), 0);
    /// assert!(v.is_empty());
    /// v.push(42);
    /// assert_eq!(v.pop(), Some(42));
    /// ```
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buf: RawVec::new(),
            len: 0,
        }
    }

    /// Constructs a new, empty `Vec<T>` with at least the specified
    /// capacity pre-allocated.
    ///
    /// The vector can hold `capacity` elements without reallocating.
    /// If `capacity` is zero, no allocation is performed. For
    /// zero-sized types, no allocation is ever performed regardless
    /// of the requested capacity; [`capacity`](Self::capacity) will
    /// continue to report `usize::MAX`.
    ///
    /// # Panics
    ///
    /// Panics on capacity overflow — if the requested capacity
    /// times `size_of::<T>()` would exceed `isize::MAX` bytes, or
    /// if the global allocator fails to satisfy the allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::vec::Vec;
    /// let mut v: Vec<i32> = Vec::with_capacity(10);
    /// assert_eq!(v.len(), 0);
    /// assert!(v.capacity() >= 10);
    ///
    /// // Ten pushes without a single reallocation:
    /// for i in 0..10 {
    ///     v.push(i);
    /// }
    /// assert!(v.capacity() >= 10);
    /// ```
    ///
    /// # Time complexity
    ///
    /// *O*(1) — a single call into the allocator, no per-element
    /// work.
    #[inline]
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: RawVec::with_capacity(capacity),
            len: 0,
        }
    }

    /// Returns the number of elements in the vector.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::vec::Vec;
    /// let mut v: Vec<i32> = Vec::new();
    /// assert_eq!(v.len(), 0);
    /// v.push(1);
    /// v.push(2);
    /// assert_eq!(v.len(), 2);
    /// ```
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the vector contains no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::vec::Vec;
    /// let mut v: Vec<i32> = Vec::new();
    /// assert!(v.is_empty());
    /// v.push(1);
    /// assert!(!v.is_empty());
    /// ```
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the total number of elements the vector can hold without
    /// reallocating.
    ///
    /// For zero-sized types, the capacity is always `usize::MAX` —
    /// ZSTs take no memory regardless of count, so no allocation is
    /// ever performed and the capacity is effectively infinite.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::vec::Vec;
    /// let v: Vec<i32> = Vec::new();
    /// assert_eq!(v.capacity(), 0);
    ///
    /// // ZSTs have infinite logical capacity:
    /// let v: Vec<()> = Vec::new();
    /// assert_eq!(v.capacity(), usize::MAX);
    /// ```
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[inline]
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.buf.capacity()
    }

    /// Returns a shared slice over the vector's initialized elements.
    ///
    /// This is the inherent method that backs the [`Deref`] impl —
    /// `&*vec` and `vec.as_slice()` produce the same `&[T]`. Calling
    /// it explicitly is occasionally useful when a future `Vec`-
    /// specific method would otherwise shadow a slice method of the
    /// same name.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::vec::Vec;
    /// let mut v = Vec::new();
    /// v.push(1);
    /// v.push(2);
    /// v.push(3);
    /// let s: &[i32] = v.as_slice();
    /// assert_eq!(s, &[1, 2, 3]);
    /// ```
    ///
    /// # Time complexity
    ///
    /// *O*(1). Returns a reference to the existing allocation with
    /// no per-element work.
    #[inline]
    #[must_use]
    pub const fn as_slice(&self) -> &[T] {
        // SAFETY: slots `[0, self.len)` contain initialized values of
        // `T` by the struct invariant, and `self.buf.ptr()` is
        // aligned to `align_of::<T>()` (including the ZST /
        // zero-length case where it is `NonNull::dangling()`, which
        // is explicitly documented as a valid source for slices that
        // read zero bytes — either because `self.len == 0` or
        // because `T` is a ZST). We return a shared reference, so no
        // mutation happens through it.
        unsafe { slice::from_raw_parts(self.buf.ptr().as_ptr(), self.len) }
    }

    /// Returns an exclusive slice over the vector's initialized
    /// elements.
    ///
    /// This is the inherent method that backs the [`DerefMut`] impl.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::vec::Vec;
    /// let mut v = Vec::new();
    /// v.push(1);
    /// v.push(2);
    /// v.push(3);
    /// let s: &mut [i32] = v.as_mut_slice();
    /// s[0] = 10;
    /// assert_eq!(v.as_slice(), &[10, 2, 3]);
    /// ```
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[inline]
    #[must_use]
    pub const fn as_mut_slice(&mut self) -> &mut [T] {
        // SAFETY: identical to `as_slice`, plus we have exclusive
        // access through `&mut self`, so the returned `&mut [T]`
        // does not alias any other reference.
        unsafe { slice::from_raw_parts_mut(self.buf.ptr().as_ptr(), self.len) }
    }

    /// Appends an element to the back of the vector.
    ///
    /// If the current length equals the capacity, the backing
    /// allocation is grown via `RawVec`'s doubling strategy before the
    /// new element is written.
    ///
    /// # Panics
    ///
    /// Panics on capacity overflow, which happens if the new allocation
    /// would exceed `isize::MAX` bytes. Allocation failure aborts the
    /// process via `handle_alloc_error`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::vec::Vec;
    /// let mut v = Vec::new();
    /// v.push(1);
    /// v.push(2);
    /// v.push(3);
    /// assert_eq!(v.len(), 3);
    /// assert_eq!(v.pop(), Some(3));
    /// assert_eq!(v.pop(), Some(2));
    /// ```
    ///
    /// # Time complexity
    ///
    /// *O*(1) amortized, *O*(*n*) worst case when the allocation grows,
    /// where *n* is the current length. The grow cost amortizes away
    /// because each reallocation doubles the capacity, so the total
    /// cost across *n* pushes is still *O*(*n*).
    #[inline]
    pub fn push(&mut self, value: T) {
        if self.len == self.buf.capacity() {
            self.buf.grow();
        }
        // SAFETY: after the conditional `grow`, `self.len <
        // self.buf.capacity()`, so `ptr.add(self.len)` is in-bounds
        // and points at uninitialized memory that is safe to write
        // `value` into. `ptr::write` does not drop the old (uninit)
        // contents, which is correct because there are none.
        unsafe {
            ptr::write(self.buf.ptr().as_ptr().add(self.len), value);
        }
        self.len += 1;
    }

    /// Appends an element if there is spare capacity, otherwise
    /// returns the element back to the caller as `Err(value)`.
    ///
    /// This method **never allocates**, never grows the backing
    /// storage, and never panics from allocation — it simply
    /// declines to push if there is no room. Useful for hot paths
    /// where the caller has pre-reserved capacity and wants to
    /// prove that no allocation will happen on a given push.
    ///
    /// This is `koala-std`'s one deliberate API deviation from
    /// `std::vec::Vec`. `std` has this method as
    /// `Vec::push_within_capacity` behind the unstable
    /// `vec_push_within_capacity` feature gate; `koala-std` ships
    /// it stable because the stability question is about `std`'s
    /// external-audience commitments, not the method's design.
    ///
    /// # Errors
    ///
    /// Returns `Err(value)` — handing the original value back to
    /// the caller — if `self.len() == self.capacity()` and there
    /// is no room for the new element. No allocation is attempted
    /// in the error case; the vector's state is completely
    /// unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::vec::Vec;
    /// let mut v: Vec<i32> = Vec::with_capacity(2);
    ///
    /// assert_eq!(v.push_within_capacity(1), Ok(()));
    /// assert_eq!(v.push_within_capacity(2), Ok(()));
    /// // Capacity is now exhausted; the third push is refused:
    /// assert_eq!(v.push_within_capacity(3), Err(3));
    /// assert_eq!(v.len(), 2);
    /// ```
    ///
    /// # Time complexity
    ///
    /// *O*(1), always. No grow path means no amortization caveat.
    #[inline]
    pub const fn push_within_capacity(&mut self, value: T) -> Result<(), T> {
        if self.len == self.buf.capacity() {
            return Err(value);
        }
        // SAFETY: `self.len < self.buf.capacity()` by the branch
        // above, so `ptr.add(self.len)` is in-bounds and points at
        // uninitialized memory. Same reasoning as `push`.
        unsafe {
            ptr::write(self.buf.ptr().as_ptr().add(self.len), value);
        }
        self.len += 1;
        Ok(())
    }

    /// Reserves capacity for at least `additional` more elements to
    /// be inserted into the vector, using the amortized doubling
    /// strategy.
    ///
    /// After calling `reserve`, `capacity() >= len() + additional`.
    /// A subsequent `push` is guaranteed not to reallocate until
    /// `additional` more elements have been pushed.
    ///
    /// Prefer this over [`reserve_exact`](Self::reserve_exact)
    /// unless you specifically want to avoid over-allocation —
    /// the amortization matters for any vector that will continue
    /// growing after the `reserve` call.
    ///
    /// # Panics
    ///
    /// Panics if `len() + additional` overflows `usize` or if the
    /// resulting byte size exceeds `isize::MAX`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::vec::Vec;
    /// let mut v: Vec<i32> = Vec::new();
    /// v.push(1);
    /// v.reserve(10);
    /// assert!(v.capacity() >= 11);
    /// ```
    ///
    /// # Time complexity
    ///
    /// *O*(*n*) worst case where *n* is the current length, due to
    /// the reallocation copy. *O*(1) when the existing capacity
    /// already satisfies the request.
    pub fn reserve(&mut self, additional: usize) {
        let needed = self.len.checked_add(additional).unwrap_or_else(|| {
            // Match RawVec's capacity_overflow behavior with an
            // equally-informative message. The unwrap_or_else
            // closure is `#[cold]`-equivalent by being rarely
            // exercised; we don't need a separate marker function
            // because `RawVec::reserve` never sees the overflow.
            panic!("koala_std::Vec::reserve: len + additional overflows usize")
        });
        self.buf.reserve(needed);
    }

    /// Reserves capacity for at least `additional` more elements,
    /// **without** the doubling amortization of
    /// [`reserve`](Self::reserve). The resulting capacity is
    /// exactly `len() + additional` (or the existing capacity, if
    /// already larger).
    ///
    /// Prefer `reserve` for most use cases. `reserve_exact` is
    /// useful when you know the final size precisely and want to
    /// avoid the over-allocation, but a pattern of repeated
    /// `reserve_exact` calls on a growing vector degrades to
    /// *O*(*n*) per push.
    ///
    /// # Panics
    ///
    /// Panics if `len() + additional` overflows `usize` or if the
    /// resulting byte size exceeds `isize::MAX`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::vec::Vec;
    /// let mut v: Vec<i32> = Vec::new();
    /// v.push(1);
    /// v.reserve_exact(10);
    /// assert_eq!(v.capacity(), 11);
    /// ```
    ///
    /// # Time complexity
    ///
    /// *O*(*n*) worst case. **Not** amortized — that is the whole
    /// point of the method.
    pub fn reserve_exact(&mut self, additional: usize) {
        let needed = self.len.checked_add(additional).unwrap_or_else(|| {
            panic!("koala_std::Vec::reserve_exact: len + additional overflows usize")
        });
        self.buf.reserve_exact(needed);
    }

    /// Shrinks the capacity of the vector to match the current
    /// length, releasing any unused backing storage.
    ///
    /// For a fully-empty vector, this deallocates the backing
    /// completely and returns the vector to its [`new`](Self::new)
    /// state. For zero-sized types, this is a no-op — the ZST
    /// sentinel capacity of `usize::MAX` cannot be shrunk.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::vec::Vec;
    /// let mut v: Vec<i32> = Vec::with_capacity(100);
    /// v.push(1);
    /// v.push(2);
    /// v.push(3);
    /// assert!(v.capacity() >= 100);
    ///
    /// v.shrink_to_fit();
    /// assert_eq!(v.capacity(), 3);
    /// ```
    ///
    /// # Time complexity
    ///
    /// *O*(*n*) worst case, where *n* is the current length, due
    /// to the potential reallocation copy. *O*(1) when the
    /// allocator can shrink in place.
    pub fn shrink_to_fit(&mut self) {
        self.buf.shrink_to(self.len);
    }

    /// Removes the last element from the vector and returns it, or
    /// `None` if the vector is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::vec::Vec;
    /// let mut v: Vec<i32> = Vec::new();
    /// assert_eq!(v.pop(), None); // pop on empty returns None
    /// v.push(10);
    /// v.push(20);
    /// assert_eq!(v.pop(), Some(20)); // LIFO order
    /// assert_eq!(v.pop(), Some(10));
    /// assert_eq!(v.pop(), None);
    /// ```
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    pub const fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        // SAFETY: `self.len` was just decremented to the index of the
        // last initialized slot, which contains a valid `T`.
        // `ptr::read` moves the value out of the slot; after this call
        // that slot is logically uninitialized again, which is
        // correct because the length decrement already excluded it
        // from the initialized range.
        Some(unsafe { ptr::read(self.buf.ptr().as_ptr().add(self.len)) })
    }
}

impl<T> Drop for Vec<T> {
    fn drop(&mut self) {
        // Drops each initialized element in forward order. The
        // backing allocation is released by `RawVec`'s own `Drop`
        // after this method returns, so we only handle the elements.
        //
        // Panic safety: `ptr::drop_in_place` on a `*mut [T]` uses
        // compiler-generated drop glue that handles single-element
        // panics internally — if one element's destructor panics,
        // the unwinding machinery still drops the remaining elements
        // via an implicit scope guard. A second panic during that
        // cleanup triggers the standard Rust double-panic abort,
        // which is unfixable by any user code and not specific to
        // this impl. This matches `std::vec::Vec`'s Drop exactly;
        // no manual scope guard is required here.
        //
        // SAFETY: slots `[0, self.len)` contain initialized values of
        // `T` by the struct's invariant. `slice_from_raw_parts_mut`
        // builds a raw `*mut [T]` (not a reference) pointing at them,
        // which is exactly what `drop_in_place` wants — using the
        // reference-returning `from_raw_parts_mut` here would
        // materialize a `&mut [T]` that imposes stricter validity
        // rules than we actually need. `drop_in_place` then runs
        // each element's destructor via the compiler-generated drop
        // glue. We have exclusive access through `&mut self`, so no
        // aliasing concerns.
        unsafe {
            ptr::drop_in_place(ptr::slice_from_raw_parts_mut(
                self.buf.ptr().as_ptr(),
                self.len,
            ));
        }
    }
}
/// `Vec<T>` dereferences to `[T]` — auto-deref coercion makes every
/// slice method available on a `Vec<T>` for free.
///
/// Concretely, this unlocks `iter`, `get`, `first`, `last`,
/// `binary_search`, `contains`, `windows`, `chunks`, indexing,
/// formatting helpers, and everything else in `impl [T]`. That's
/// why `Vec<T>`'s inherent impl stays small — everything that
/// applies to an arbitrary slice lives on `[T]` and is borrowed
/// through this coercion.
///
/// # Examples
///
/// ```
/// # use koala_std::vec::Vec;
/// let mut v = Vec::new();
/// v.push(3);
/// v.push(1);
/// v.push(2);
///
/// // All of these come from `impl [T]`, not `impl Vec<T>`:
/// assert_eq!(v.first(), Some(&3));
/// assert_eq!(v.last(), Some(&2));
/// let sum: i32 = v.iter().sum();
/// assert_eq!(sum, 6);
/// ```
impl<T> Deref for Vec<T> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

/// Mutable counterpart to the [`Deref`] impl. Unlocks the `_mut`
/// slice methods (`iter_mut`, `get_mut`, `sort`, `reverse`, etc.)
/// and mutable indexing.
impl<T> DerefMut for Vec<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

/// Creates an empty `Vec<T>`. Equivalent to [`Vec::new`] — no
/// allocation, no elements.
///
/// # Examples
///
/// ```
/// # use koala_std::vec::Vec;
/// let v: Vec<i32> = Vec::default();
/// assert!(v.is_empty());
/// assert_eq!(v.capacity(), 0);
/// ```
impl<T> Default for Vec<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

/// Formats the vector as `[elem0, elem1, ...]` by delegating to the
/// slice's [`Debug`](fmt::Debug) impl.
///
/// # Examples
///
/// ```
/// # use koala_std::vec::Vec;
/// let mut v = Vec::new();
/// v.push(1);
/// v.push(2);
/// v.push(3);
/// assert_eq!(format!("{:?}", v), "[1, 2, 3]");
/// ```
impl<T: fmt::Debug> fmt::Debug for Vec<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_slice(), f)
    }
}

/// Two `Vec<T>` compare equal when they have the same length and
/// all corresponding elements compare equal. Delegates to the slice
/// `PartialEq` impl, which short-circuits on length mismatch.
///
/// # Examples
///
/// ```
/// # use koala_std::vec::Vec;
/// let mut a = Vec::new();
/// a.push(1);
/// a.push(2);
///
/// let mut b = Vec::new();
/// b.push(1);
/// b.push(2);
///
/// assert_eq!(a, b);
///
/// b.push(3);
/// assert_ne!(a, b);
/// ```
impl<T: PartialEq> PartialEq for Vec<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

/// `Vec<T>` inherits `Eq` whenever `T: Eq` — the relation is
/// reflexive, symmetric, and transitive by the slice's `Eq` impl.
impl<T: Eq> Eq for Vec<T> {}

/// Hashes the vector by delegating to the slice's [`Hash`] impl,
/// which feeds the length into the hasher first and then each
/// element in order. The length prefix is what distinguishes
/// `[1, 2]` concatenated with `[3]` from `[1, 2, 3]` — without it,
/// `Hash` would collide on nested structures.
impl<T: Hash> Hash for Vec<T> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hash::hash(self.as_slice(), state);
    }
}

/// Clones the vector by pre-allocating a backing of the same
/// capacity as the source's length and element-wise cloning into
/// it.
///
/// # Panic safety
///
/// If an element's `Clone` impl panics part-way through the loop,
/// the partially-built vector is dropped during unwinding. Each
/// successful clone is committed via `push`, which increments the
/// running `len` before the next clone is attempted — so the new
/// vector's `len` always reflects the number of elements actually
/// initialized, and `Vec::drop` will correctly run `drop_in_place`
/// on exactly those elements. No manual scope guard is needed;
/// continuously maintaining the invariant is the lighter and more
/// common form of panic safety for this kind of loop.
///
/// # Examples
///
/// ```
/// # use koala_std::vec::Vec;
/// let mut v = Vec::new();
/// v.push(1);
/// v.push(2);
/// v.push(3);
///
/// let w = v.clone();
/// assert_eq!(v, w);
/// ```
///
/// # Time complexity
///
/// *O*(*n*) where *n* is the source vector's length, plus the
/// cost of one allocation. Because we pre-allocate exactly
/// `self.len()` slots, the `push` loop never triggers a `grow`.
impl<T: Clone> Clone for Vec<T> {
    fn clone(&self) -> Self {
        let mut new = Self {
            buf: RawVec::with_capacity(self.len),
            len: 0,
        };
        for item in self {
            new.push(item.clone());
        }
        new
    }
}

/// Collects elements from an iterator into a new `Vec<T>`,
/// pre-allocating based on the iterator's `size_hint` lower bound.
///
/// If the iterator yields more elements than the lower bound
/// promised, `push` will grow the backing allocation as normal —
/// the pre-allocation is a hint, not a contract. The lower bound
/// (rather than the upper bound) is used because an iterator may
/// report an unbounded or dishonest upper bound that would make
/// `with_capacity` over-commit or OOM.
///
/// # Examples
///
/// ```
/// # use koala_std::vec::Vec;
/// let v: Vec<i32> = (1..=3).collect();
/// assert_eq!(v.as_slice(), &[1, 2, 3]);
/// ```
///
/// # Time complexity
///
/// *O*(*n*) where *n* is the number of elements yielded, plus the
/// cost of any grows when the iterator exceeds the pre-allocated
/// capacity.
impl<T> FromIterator<T> for Vec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let (lower, _) = iter.size_hint();
        let mut v = Self {
            buf: RawVec::with_capacity(lower),
            len: 0,
        };
        for item in iter {
            v.push(item);
        }
        v
    }
}

/// Allows `for x in &vec` — yields shared references to each
/// element in order, delegating to the slice iterator.
///
/// # Examples
///
/// ```
/// # use koala_std::vec::Vec;
/// let mut v = Vec::new();
/// v.push(1);
/// v.push(2);
/// v.push(3);
///
/// let mut sum = 0;
/// for x in &v {
///     sum += x;
/// }
/// assert_eq!(sum, 6);
/// ```
impl<'a, T> IntoIterator for &'a Vec<T> {
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Allows `for x in &mut vec` — yields exclusive references to each
/// element in order, delegating to the slice iterator.
///
/// # Examples
///
/// ```
/// # use koala_std::vec::Vec;
/// let mut v = Vec::new();
/// v.push(1);
/// v.push(2);
/// v.push(3);
///
/// for x in &mut v {
///     *x *= 10;
/// }
/// assert_eq!(v.as_slice(), &[10, 20, 30]);
/// ```
impl<'a, T> IntoIterator for &'a mut Vec<T> {
    type Item = &'a mut T;
    type IntoIter = slice::IterMut<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

