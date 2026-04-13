---
created: 2026-04-13
area: koala-std::vec
status: open — milestone 1 / v1.0 Vec in progress
---

# koala-std `Vec<T>` design notes

This document captures the design discussion that shaped `Vec<T>` in
milestone 1 of `koala-std` — specifically the "easy first, then
smarter" philosophy, where `std::Vec` is actually weak, which gaps
are worth pursuing in later milestones, and which explicitly are
not. See `koala-std-roadmap.md` for the overall crate direction.

## The dumb-first philosophy

The `koala-std` `Vec<T>` implementation progresses through three
deliberate stages:

### v1.0 — Correct minimal

Match `std::Vec`'s API exactly. Aim for correctness parity, not
innovation. The value of this exercise is internalizing `std`'s
choices by rebuilding them from the allocator up, not deviating
from them on the first pass.

Scope:

- `Vec<T>` struct: `{ buf: RawVec<T>, len: usize }`
- `new()`, `with_capacity(n)`, `len()`, `capacity()`, `is_empty()`
- `push(T)`, `pop() -> Option<T>`
- `Drop` with a panic-safe scope guard that drops remaining
  elements even if an element destructor unwinds
- `Deref<Target = [T]>` / `DerefMut<Target = [T]>` — gives access
  to every slice method for free (`iter`, `get`, `first`/`last`,
  `sort`, `binary_search`, formatting helpers, etc.)
- Trait impls: `Debug`, `PartialEq`, `Eq`, `Hash`, `Clone`,
  `Default`, `FromIterator`, `IntoIterator for &Vec`,
  `IntoIterator for &mut Vec`

Whenever there's a choice between "match `std` exactly" and "try
something different," default to **match `std` exactly**. Save
deviation for later milestones with a concrete reason. Every
deviation is a test the differential `quickcheck` harness can't
cover because there's no `std` counterpart to compare against.

### v1.1 — API parity

Everything that's "just more method impls" and doesn't require
rethinking the type:

- `IntoIter<T>` consuming iterator (with its own `Drop` that drops
  remaining elements and deallocs backing)
- `Drain<'_, T>` lifetime-scoped partial consume
- `insert(i, T)` / `remove(i)` using `ptr::copy` for overlapping
  shifts (not `copy_nonoverlapping`)
- `extend`, `extend_from_slice`
- `reserve` / `reserve_exact` / `shrink_to_fit`
- `truncate` / `clear` / `retain` / `dedup`
- `Clone` with element-wise clone + drop-on-panic scope guard

### v1.2 — Performance niceties

- `Copy` specialization for `extend_from_slice` / `clone` using
  `copy_nonoverlapping` instead of element-wise loops
- `FromIterator` with size-hint-aware pre-reservation
- (Note: `realloc`-in-place on grow is already in v1.0 via
  `alloc::alloc::realloc`, so there's no separate step for it)

## Where `std::Vec` is actually weak

Being honest: `std::Vec` is ferociously well-engineered and the
gaps are narrow. But they're real. This section catalogs them so
later milestones can pick them up.

### Real API gaps

1. **`push_within_capacity` is unstable.** `std` has it behind a
   nightly feature gate. It returns `Result<(), T>` — pushes if
   there's room, gives the value back if not. Useful for hot paths
   where you've pre-reserved and want to prove there's no
   allocation. **We can just make it stable.** Trivial, genuinely
   useful. Candidate for v1.0 as the single explicit deviation
   from `std` API parity.

2. **`spare_capacity_mut() -> &mut [MaybeUninit<T>]`** is stable
   but awkward. The flow "write into spare capacity, then commit
   via `set_len`" is clunky and easy to misuse. A closure-based
   `build_with(n, |uninit| ...)` that takes a closure and bumps
   `len` on successful return is cleaner and harder to misuse.
   **Candidate for v1.1.**

3. **No "append-by-move" for non-`Clone` types.**
   `extend_from_slice` requires `T: Clone`. `append(&mut other)`
   moves but requires you to already have a `Vec` on the other
   side. There's no "move a range out of a slice you own"
   primitive. Minor gap.

4. **No ergonomic "would this push trigger a grow" predicate.**
   You can compute `len() == capacity()` manually but there's no
   method. Trivial to add as `would_reallocate_on_push() -> bool`.
   Opinionated — `std` might consider it unnecessary, but for
   learning it's nice to have the question surfaced.

5. **`Vec::from_raw_parts_in`** — allocator-parameterized
   construction — is nightly-only in `std`. When a custom
   allocator trait exists (milestone 3-ish), we can make this
   stable in `koala-std`.

### Real layout / performance weaknesses

1. **Three `usize` fields = 24 bytes on 64-bit.** For workloads
   with millions of mostly-empty `Vec`s (DOM attribute lists, CSS
   property overrides per node), this is significant. **Not
   fixable in `Vec<T>` itself** — it's inherent to the
   `{ ptr, len, cap }` design. The fix is a different type; see
   "the real opportunities" below.

2. **Grow strategy is fixed at 2×.** Some allocators can reuse a
   freed 2× block better with 1.5× growth. `std` is not tunable.
   Almost nobody actually needs this; API bloat for unclear
   benefit. **Skip.**

3. **No SIMD specializations.** `std` relies on LLVM
   auto-vectorization through `copy_nonoverlapping`, which is
   usually fine but not always. Writing explicit SIMD is a rabbit
   hole and usually worse than LLVM's output. **Skip.**

4. **`Drop` of a large `Vec` with non-trivial destructors is
   sequential.** A `Vec` of 1M `String`s drops strings one at a
   time. Parallel drop is research territory. **Skip** (but see
   the "interesting sidebar" below).

### Things `std` does right that we should just copy

Every one of these exists because `std` tried the alternative and
regretted it. Do not deviate:

- Infallible API with `handle_alloc_error` on OOM.
- Doubling growth with a min-floor of 4.
- `NonNull<T>` for niche optimization (`Option<Vec<T>>` is the
  same size as `Vec<T>`).
- `NonNull::dangling()` for zero-capacity state.
- `realloc` preference on grow (may extend in place).
- Separation of `RawVec<T>` from `Vec<T>` (already done — commit
  `19958e9`).
- Panic-safe `Drop` via scope guard.
- `Deref<Target = [T]>` to inherit the slice API.

## Ranked opportunities for `koala-std`

In rough order of "realistic v1.0 inclusion" → "much later":

| Opportunity | Target milestone | Rationale |
|---|---|---|
| `push_within_capacity` (stable) | v1.0 or v1.1 | Trivial, genuinely useful, `std` has it on nightly |
| `build_with(n, closure)` for uninit writes | v1.1 | Cleaner than `spare_capacity_mut()` + `set_len` |
| `would_reallocate_on_push() -> bool` | v1.1 | Useful predicate, trivial |
| `Vec::into_iter_range(a..b)` | v1.2 | Take ownership of a range, keep the rest. Useful for layout-pass bookkeeping. Genuinely new. |
| **`SmallVec<T, const N: usize>`** | milestone 2 | Separate type. Matches browser workloads (CSS selectors, inline runs). Profile-justified. |
| **`ThinVec<T>`** | milestone 2 | Separate type. Matches DOM attribute workloads (empty-common). Profile-justified. Requires storing metadata in allocation header. |
| **`ArenaVec<T>`** | milestone 3 | Ties into `BumpAllocator`. Lifetime-scoped, never reallocates, zero-cost deallocation. Very Koala-specific. |
| Custom `Allocator` trait parameter | milestone 3 | Needed to plug `Vec<T, BumpAllocator>` together. `std`'s trait is unstable; we'd define our own. |

## The real opportunities — not `Vec<T>` itself

The meaningful "improve on `std`" energy should go into types
`std` deliberately does not cover, not into `Vec<T>` itself:

### `SmallVec<T, const N: usize>` (milestone 2)

Inline storage for `N` elements, heap past that. No allocation at
all until you exceed `N`. Used heavily in `servo`, `rustc`, and
other Rust projects where many small collections exist.

Koala use cases (likely, to be profiled):
- CSS selector component lists (usually 1–3 simple selectors)
- Inline formatting context runs of inline fragments
- Table row → cells (often small)
- Attribute lists on a single DOM element

### `ThinVec<T>` (milestone 2)

Single pointer, stores `len` + `cap` at the allocation header.
Empty `ThinVec<T>` is a literal null pointer — 8 bytes on 64-bit
instead of 24. Firefox's `nsTArray` works this way.

Koala use cases (likely, to be profiled):
- DOM attribute lists on elements with no attributes (very common)
- CSS property overrides on nodes inheriting everything (very
  common in deep trees)
- Any per-element list where "zero items" is the dominant case

### `ArenaVec<T>` (milestone 3)

Uses `BumpAllocator` directly. Lifetime-tied to an arena. Never
reallocates (grow is a new bump allocation; old contents are
abandoned in place). Deallocation is zero-cost — the whole arena
is freed at once. Very Koala-specific; the layout engine is the
obvious target.

## Concrete recommendation for v1.0

**Match `std::Vec`'s API exactly, plus exactly one addition**: a
stable `push_within_capacity(&mut self, value: T) -> Result<(), T>`.

Rationale for only one deviation:

- Every deviation is a test the differential `quickcheck` harness
  can't cover, because there's no `std` counterpart to compare
  against.
- The whole point of v1.0 is internalizing `std`'s choices, not
  scattering improvements.
- Saving the real improvements (`SmallVec`, `ThinVec`, `ArenaVec`,
  `build_with`) for later milestones means each one gets proper
  design attention instead of being retrofitted into a crowded
  v1.0.
- `push_within_capacity` is cheap to add (it's literally a length
  check followed by the existing push body) and it's a method
  `std` has explicitly chosen to stabilize eventually — it's
  borrowed, not novel.

## Interesting sidebar — parallel drop

Mentioned for future reference; not on any milestone roadmap.

A `Vec<String>` with 1M entries drops strings sequentially when
the outer `Vec` is dropped. Each `String` does its own dealloc,
one after another. The obvious question: could you parallelize
drops across a thread pool and get an N× speedup?

In practice this is significantly harder than it looks, which is
why `std` does not do it:

1. **Drop order is observable**. Most `Drop` impls are independent
   but not all. If dropping `T` has side effects that racing with
   another `T`'s drop would break, parallelization changes
   semantics. `std` cannot assume drops are commutative.

2. **Thread-pool overhead dominates for small `T`**. Spawning work
   onto a thread pool costs microseconds. Dropping a `String`
   costs tens of nanoseconds. You need elements whose individual
   drop is expensive enough to amortize the scheduling overhead —
   think `Vec<Box<LargeTree>>` where each drop recursively frees
   a large structure.

3. **Requires `Send + Sync` guarantees**. `Drop` runs with
   exclusive `&mut T` access; you'd need the element to be safely
   droppable from a different thread, which is `T: Send`. Not
   every `T` is `Send`, and `Vec` does not currently require it.

4. **Breaks `no_std` compatibility**. Thread pools live in `std`
   (or a dependency). A parallel-drop `Vec` cannot be in a
   `no_std` crate.

5. **Memory model contention**. N threads deallocating
   simultaneously hit the allocator's internal lock (for most
   allocators). The speedup is bounded by allocator concurrency,
   not by thread count.

### When it might actually pay off

- `Vec<T>` where `T` owns a large tree or arena that takes
  significant CPU to walk during drop.
- Shutdown paths where the program is exiting anyway and you're
  willing to tolerate parallelism overhead to finish faster.
- Specific crates like `rayon`'s parallel collections, which opt
  into parallel drop via user-visible API (you ask for it
  explicitly rather than it being automatic).

### Where this lives in `koala-std`

Not anywhere. It's a `koala-os` concern at the earliest — it
needs threading, which is a syscall concern, which is the `os`
layer. And even there, the right abstraction is probably "a
parallel-drop adapter around any `Vec<T>`" rather than baking it
into `Vec<T>` itself. For now it's a curiosity, not a roadmap
item.

If we ever genuinely need it — e.g., the layout pass drops
millions of `LayoutBox`es between renders and that drop time is
observable — the right starting point is probably:

1. Profile to confirm drop time is actually the bottleneck (not
   allocation, not tree walking).
2. Check whether switching to an arena allocator (milestone 3's
   `BumpAllocator`) makes the problem disappear entirely —
   arenas have O(1) drop regardless of element count.

In most realistic Koala scenarios, the arena is a better answer
than parallel drop. The arena gives you the speedup for free
without needing threads, memory model reasoning, or user-visible
API changes.
