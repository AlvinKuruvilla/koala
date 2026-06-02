# koala-std FlyString — design

Interned, cheaply-comparable strings for Koala (milestone 2's flagship).
The motivating analysis lives in the 2026-06-01 session discussion; the
short version: strings are the engine's dominant allocation, drawn from
tiny repeated vocabularies (tag names, attribute names, CSS idents, class
names), and selector matching compares them on a hot path. Interning
stores each distinct string once and makes equality an O(1) pointer
compare — the canonical browser primitive (Gecko `nsAtom`, Blink
`AtomicString`, Servo `Atom`, Ladybird `FlyString`).

## Locked decisions

1. **Representation: `FlyString(Arc<str>)`.**
   - `Arc`, not `Rc`: the engine moves interned strings across threads
     (per-tab worker → main `RenderJob`), so `FlyString` must be
     `Send + Sync`. Same constraint that forced `RawTable: Send + Sync`.
   - 8 bytes; `Clone` is one atomic increment.
   - **Safe code** — no raw pointers, no `unsafe`. The learning here is
     designing a correct interning abstraction and upholding its
     invariant, not memory-unsafety. (Contrast the hash table.)

2. **The interning invariant: equal content ⟺ same `Arc`.**
   Equality and hashing both rely on it:
   - `PartialEq` is `Arc::ptr_eq` — O(1).
   - `Hash` hashes the pointer (`Arc::as_ptr as usize`) — O(1), so
     `FlyString`-keyed maps are fast.
   The invariant holds only among `FlyString`s produced by the **same
   `Interner`**. In production that is the one process-global interner
   (a later phase); within these tests it is one local `Interner`.
   `FlyString` therefore has **no public constructor from `&str`/`Arc`** —
   it can only be born through `Interner::intern`, or the invariant
   breaks. A `debug_assert` in `eq` (pointers differ ⇒ contents differ)
   is a cheap tripwire against a stray cross-interner comparison.

3. **The interner: `Interner { table: HashSet<Arc<str>> }`.**
   `intern(&mut self, &str) -> FlyString`:
   - `table.get(s)` → if present, clone that canonical `Arc` into a
     `FlyString`;
   - else `Arc::from(s)`, insert a clone into the table, return it.
   The "retrieve the stored copy equal to a query" step is exactly what
   `std::collections::HashSet::get` exists for — and koala-std's HashSet
   doesn't have it yet, so this milestone adds it (see prerequisites).

4. **Placement (resolved, but only Phase 1+ touches it).** koala-std is
   `#![no_std]` and defers `Mutex`/`LazyLock` to std, but a *global*
   interner needs a global lock. Split by what needs the OS:
   `FlyString` + `Interner` live in **koala-std** (only need
   `alloc::sync::Arc`); the global `static LazyLock<Mutex<Interner>>` +
   the `intern(&str) -> FlyString` entry point live in **koala-common**
   (std). Phase 0 builds only the koala-std half, so the global question
   stays parked.

## Prerequisites (koala-std additions this needs)

- **`HashMap::get_key_value<Q>(&self, &Q) -> Option<(&K, &V)>`** — mirrors
  `get` but returns the stored key too (uses the same `find_index`).
- **`HashSet::get<Q>(&self, &Q) -> Option<&T>`** — delegates to
  `get_key_value` and returns the key. The std-aligned interning method.

## FlyString API (Phase 0)

- `Interner::new()`, `Interner::with_capacity(n)`, `Interner::len`,
  `Interner::is_empty`, `Interner::intern(&mut self, &str) -> FlyString`.
- `FlyString`: `as_str(&self) -> &str`, `Deref<Target = str>`, `Clone`,
  `PartialEq`/`Eq` (pointer), `Hash` (pointer), `Debug`, `Display`.
  Construction is crate-private (only `Interner` builds one).
- Deliberately **not** in Phase 0: `Ord` (content order, add when a
  consumer needs sorted/BTree use), `PartialEq<str>` ergonomic compares,
  the global `intern()`, and any engine adoption.

## Phasing (engine adoption is later, and measured)

- **Phase 0 — koala-std (this slice):** the prerequisites + `FlyString` +
  `Interner` + tests. No global, no engine.
- **Phase 1 — koala-common:** global `static Mutex<Interner>` + `intern()`.
- **Phase 2 — DOM `tag_name`** → FlyString; measure with `bench-diff`.
- **Phase 3 — attribute *names* + class tokens** (values stay `String`).
- **Phase 4 — CSS idents/property names + selector side** → unlocks
  O(1) pointer-compare selector matching.

## Testing posture (Phase 0)

No differential-vs-std harness (std has no `FlyString`); test the
contract directly:
- interning invariant: two `intern` calls with equal content yield
  pointer-equal `FlyString`s; distinct content yields distinct.
- `Send + Sync` (compile-time assertion + move/share across threads).
- hash agreement: equal `FlyString`s hash equal; usable as a map key.
- `Deref`/`as_str` round-trips the bytes.
- interner dedup: interning the same string N times grows the table by 1.

No miri-critical `unsafe` (all safe), so miri is a nice-to-have here, not
the gate it was for the hash table.
