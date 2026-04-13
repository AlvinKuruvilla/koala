//! A contiguous growable array type with heap-allocated contents.
//!
//! This module provides [`Vec<T>`], `koala-std`'s hand-rolled
//! counterpart to `std::vec::Vec<T>`. It is built on the private
//! `RawVec<T>` helper which owns the allocation and tracks capacity;
//! `Vec<T>` adds the element count on top.

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
    // `Default` lands in commit #4 alongside the rest of the trait
    // impls. `new_without_default` is suppressed until then to keep
    // this commit scoped to docs rather than API surface.
    #[allow(clippy::new_without_default)]
    pub const fn new() -> Self {
        Self {
            buf: RawVec::new(),
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

