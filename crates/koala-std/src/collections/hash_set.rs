//! A hash set built as a thin wrapper over [`HashMap`].
//!
//! `HashSet<T>` stores its elements as the keys of a `HashMap<T, ()>`.
//! The unit value is a zero-sized type, so the set adds no per-element
//! storage over the map's keys — every method here forwards to the
//! inner map, and the only real work is translating the map's
//! `Option<V>` returns into the set's `bool` returns.
//!
//! Iterator types live in this module (`hash_set::Iter`,
//! `hash_set::IntoIter`) rather than being re-exported at the
//! `collections` root, because the root already exposes
//! `HashMap`'s `Iter` / `IntoIter`. This mirrors `std`, where the two
//! families are distinguished by module path
//! (`std::collections::hash_set::Iter`). Revisit if you prefer a
//! different naming scheme — see question 4 in the handoff.

use core::borrow::Borrow;
use core::fmt;
use core::hash::{BuildHasher, Hash};

use crate::collections::hash_map::{self, HashMap};
use crate::hash::FxBuildHasher;

/// A hash set, implemented as a `HashMap` whose values are `()`.
///
/// Like [`HashMap`], it defaults to [`FxBuildHasher`] and offers
/// average *O*(1) insert / lookup / removal.
pub struct HashSet<T, S = FxBuildHasher> {
    map: HashMap<T, (), S>,
}

impl<T> HashSet<T, FxBuildHasher> {
    /// Creates an empty `HashSet` with the default [`FxBuildHasher`].
    ///
    /// Allocates nothing until the first insertion.
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[must_use]
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Creates an empty `HashSet` with room for at least `capacity`
    /// elements before the first resize.
    ///
    /// # Time complexity
    ///
    /// *O*(*capacity*) for the initial allocation.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            map: HashMap::with_capacity(capacity),
        }
    }
}

impl<T> Default for HashSet<T, FxBuildHasher> {
    fn default() -> Self {
        Self::new()
    }
}

/// Accessors and constructors that need no bound on `T` or `S` because
/// they never hash or compare an element.
impl<T, S> HashSet<T, S> {
    /// Creates an empty `HashSet` that will use `hasher`.
    pub fn with_hasher(hasher: S) -> Self {
        Self {
            map: HashMap::with_hasher(hasher),
        }
    }

    /// Creates an empty `HashSet` with the given capacity and hasher.
    pub fn with_capacity_and_hasher(capacity: usize, hasher: S) -> Self {
        Self {
            map: HashMap::with_capacity_and_hasher(capacity, hasher),
        }
    }

    /// Returns the number of elements in the set.
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns `true` if the set contains no elements.
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Returns the number of elements the set can hold without resizing.
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.map.capacity()
    }

    /// Removes all elements, keeping the allocated capacity.
    ///
    /// # Time complexity
    ///
    /// *O*(*capacity*).
    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// An iterator visiting all elements in arbitrary order, yielding
    /// `&T`.
    ///
    /// # Time complexity
    ///
    /// *O*(*capacity*) to walk the backing array.
    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            inner: self.map.keys(),
        }
    }
}

/// Operations that hash or compare elements, so they require
/// `T: Hash + Eq` and a `BuildHasher`.
impl<T, S> HashSet<T, S>
where
    T: Hash + Eq,
    S: BuildHasher,
{
    /// Adds `value` to the set. Returns `true` if it was newly inserted,
    /// `false` if the set already contained it.
    ///
    /// Unlike the inner map's `insert`, the existing element is *not*
    /// replaced when already present (the value is `()`, so this is a
    /// no-op anyway, but the semantic matters for `T`s with identity).
    ///
    /// # Time complexity
    ///
    /// Average *O*(1), amortized over resizes.
    pub fn insert(&mut self, value: T) -> bool {
        self.map.insert(value, ()).is_none()
    }

    /// Returns `true` if the set contains an element equal to `value`.
    ///
    /// Accepts any borrowed form of `T` (e.g. `&str` for a
    /// `HashSet<String>`), mirroring [`HashMap::get`].
    ///
    /// # Time complexity
    ///
    /// Average *O*(1).
    pub fn contains<Q>(&self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.map.contains_key(value)
    }

    /// Returns a reference to the element equal to `value`, or `None`.
    ///
    /// Unlike [`contains`](Self::contains), this hands back the set's
    /// *stored* element — the canonical copy — which is what string
    /// interning needs: look up a `&str`, and if it's already in the
    /// set, clone the stored `Arc<str>` rather than allocate a new one.
    ///
    /// # Time complexity
    ///
    /// Average *O*(1).
    pub fn get<Q>(&self, value: &Q) -> Option<&T>
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        // Keep the `Option` and project to the key — `None` (absent value)
        // must stay `None`, not panic.
        self.map.get_key_value(value).map(|(k, _)| k)
    }

    /// Removes `value` from the set. Returns `true` if it was present.
    ///
    /// # Time complexity
    ///
    /// Average *O*(1).
    pub fn remove<Q>(&mut self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.map.remove(value).is_some()
    }

    /// Reserves capacity for at least `additional` more elements.
    pub fn reserve(&mut self, additional: usize) {
        self.map.reserve(additional);
    }
}

/// Borrowing iterator over a set's elements, yielding `&T`. Wraps the
/// inner map's `Keys`.
pub struct Iter<'a, T> {
    inner: hash_map::Keys<'a, T, ()>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        // `Keys` already yields `&T`, so this is a straight forward.
        self.inner.next()
    }
}

/// Owning iterator over a set's elements, yielding `T`. Wraps the inner
/// map's `IntoIter` and drops the `()` value of each pair.
pub struct IntoIter<T> {
    inner: hash_map::IntoIter<T, ()>,
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(k, ())| k)
    }
}

impl<T, S> IntoIterator for HashSet<T, S> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            inner: self.map.into_iter(),
        }
    }
}

impl<'a, T, S> IntoIterator for &'a HashSet<T, S> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T: Clone, S: Clone> Clone for HashSet<T, S> {
    fn clone(&self) -> Self {
        Self {
            map: self.map.clone(),
        }
    }
}

impl<T: fmt::Debug, S> fmt::Debug for HashSet<T, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

impl<T: Eq + Hash, S: BuildHasher> PartialEq for HashSet<T, S> {
    fn eq(&self, other: &Self) -> bool {
        // Set equality, order-independent: same size and every element of
        // `self` is present in `other`. Equal lengths make the one-way
        // containment check sufficient.
        self.len() == other.len() && self.iter().all(|value| other.contains(value))
    }
}

impl<T: Eq + Hash, S: BuildHasher> Eq for HashSet<T, S> {}

impl<T: Eq + Hash, S: BuildHasher> Extend<T> for HashSet<T, S> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for value in iter {
            let _ = self.insert(value);
        }
    }
}

impl<T: Eq + Hash, S: BuildHasher + Default> FromIterator<T> for HashSet<T, S> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut set = Self::with_hasher(S::default());
        set.extend(iter);
        set
    }
}
