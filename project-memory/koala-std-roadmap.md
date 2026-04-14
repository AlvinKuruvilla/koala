---
created: 2026-04-13
area: koala-std
status: open — milestone 1 in progress (RawVec landed, Vec next)
---

# koala-std roadmap

`koala-std` is Koala's hand-rolled `#![no_std]` + `alloc` foundation
library — a learning-motivated replacement for selected types from
Rust's standard library. This document captures the versioned plan
(what's in each milestone and why), the design principles borrowed
from Ladybird's `AK`, and the categories of `std` work that are
deliberately deferred to `std` rather than rebuilt.

This is the source of truth for koala-std's direction. Future work
should align with it; changes require updating this document first.

## Why koala-std exists

Koala is explicitly an experimental / educational agent-native
browser. Its existing crates — `koala-html`, `koala-css`,
`koala-dom`, `koala-browser` — are all built by reading the relevant
specs and translating them into Rust. `koala-std` extends that
build-from-scratch philosophy into the systems-primitives layer that
Koala otherwise inherits from `std`.

The motivation is explicitly **learning**: understanding how `Vec`,
`HashMap`, and related types actually work by building them against
the abstract machine that `core` and `alloc` provide. This is
different from the motivation for the existing crates, which is
**spec implementation**. Both exercises are valuable; they teach
complementary skills and run in parallel as concurrent threads, not
sequential calendar blocks.

**Success metric**: "I understand how `Vec<T>` works from the
allocator up." Not: "koala-std replaced every `std::Vec` usage in
Koala." Migration is a separate project that may or may not follow.

## Two learning tracks — distinct, concurrent

1. **Spec implementation** — `koala-image` (PNG/JPG decoders), future
   `koala-font` (TrueType/OpenType + glyph rasterization), future
   `koala-shape` (HarfBuzz-equivalent text shaping), and the
   eventual replacement for Boa. Teaches reading a standard and
   translating it faithfully. Skill Koala is already built on.

2. **Rust unsafe + systems primitives** — `koala-std`. Teaches raw
   pointers, memory layout, `Drop` ordering, ZST correctness, hash
   table design, allocator design. New territory for this project.

Doing one of each track in parallel is more valuable than doing two
from the same track, because the skill families don't overlap.
Main browser work (DOM↔JS bridge, rendering improvements, layout
correctness) continues alongside both tracks — nothing about
starting `koala-std` pauses the rendering pipeline.

### Track-1 sequence

The spec-implementation queue, in order of intended start:

1. **`koala-image`** — PNG first (DEFLATE + filter algorithms +
   chunk structure; self-contained with clear test vectors), then
   JPG (Huffman + DCT + chroma subsampling), then simpler formats
   as needed. Replaces the `image` crate's decode path.
2. **`koala-font`** — TrueType / OpenType table parsing and glyph
   rasterization. Replaces `fontdue`. Substantial spec undertaking.
3. **`koala-shape`** — HarfBuzz-equivalent. Complex script shaping,
   OpenType `GSUB`/`GPOS`, ligatures, kerning, bidi interaction.
   Genuinely one of the hardest things in a browser. `rustybuzz` is
   permitted as a temporary crutch but treated as a known migration
   target.

## The layering principle (from Ladybird's AK)

Ladybird's foundation library `AK/` is strictly OS-free. Everything
that needs a syscall — files, sockets, threads, processes — lives
in `LibCore`, not `AK`. We adopt the same split:

```
core       rustc-provided: Copy, Sized, Drop, Iterator, primitive ops
  ↓
alloc      rustc-provided: GlobalAlloc, heap primitives
  ↓
koala-std  this crate: collections, strings, arena allocator
           NO syscalls, NO std:: imports, backed by alloc::alloc::Global
  ↓
koala-os   future crate: File, TcpStream, Thread, real Mutex parking
           appears when the first syscall is concretely needed
  ↓
koala-{browser, css, html, dom, js, ...}
```

Importing from `std::` inside `koala-std` is a hard design violation.
The crate is `#![cfg_attr(not(test), no_std)]` + `extern crate alloc`.
The `cfg_attr` is needed because Rust's built-in `#[test]` harness
pulls in `std`; production consumers never see `std`.

Writing a custom allocator is explicitly out of scope. `koala-std`
uses `alloc::alloc::Global` as its backing allocator. A custom
allocator would be a different project and would blur what `Vec` is
teaching — that project happens at milestone 3 via `BumpAllocator`,
which is an *additional* allocator, not a replacement for `Global`.

## Ladybird AK lessons (confirmed by reading the actual directory)

The `AK/` directory was fetched from the Ladybird GitHub repo on
2026-04-13. Three insights worth stealing:

### 1. AK is OS-free

Zero syscalls in the foundation library. Captured above in the
layering principle.

### 2. "One `String` type" is not enough for a browser

AK has five live string types:

- **`String`** — UTF-8 validated, refcounted, immutable. Cheap
  clones for text content.
- **`FlyString`** — interned/deduped. O(1) equality via pointer
  compare. Used for DOM tag names, attribute names, CSS identifiers.
- **`Utf16String`** — ECMAScript strings are spec'd as UTF-16. JS
  engines don't want to convert on every string operation.
- **`Utf16FlyString`** — UTF-16 interned form.
- **`StringBuilder`** — mutable UTF-8 building via `Vec<u8>`.

Plus view types (`StringView`, `Utf8View`, `Utf16View`, `Utf32View`)
for borrowed access.

Rust's single `String` type is fine for application code but will
hurt in a browser. Milestone 2 builds this family.

### 3. `ErrorOr<T>` ≈ `Result<T, Error>`

Ladybird chose typed return-value errors instead of C++ exceptions.
Rust gives us this for free via `core::result::Result` + a
user-defined `Error` enum. No work needed — one category of AK's
work that we can cross off without writing a line of code.

### Things from AK that Rust gives us free from `core`

- `Span` → `&[T]`
- `Array` → `[T; N]`
- `Tuple` → tuples
- `Optional` → `Option`
- `Variant` → enums
- `Iterator` / `Find` / `AllOf` / `AnyOf` / `Enumerate` → `Iterator`
  trait and its methods
- `Concepts` → traits
- `ErrorOr` → `Result`
- `Noncopyable` → absence of `impl Copy`

Don't rebuild any of these. They already exist in the unavoidable
compiler floor and rebuilding them would be non-interoperable parallel
hierarchies with no benefit.

## Crate conventions (locked 2026-04-13)

- **Location**: `crates/koala-std/`, added to the root
  `Cargo.toml` workspace members, exposed via
  `koala-std = { path = "crates/koala-std" }` in
  `[workspace.dependencies]`.
- **Milestones ≠ SemVer**. The "milestone 1/2/3" labels in this
  document are *internal learning milestones*, not crate SemVer
  releases. The crate starts at `0.1.0` and bumps minor versions as
  milestones land. "Milestone 1 done" does not imply `1.0.0` — the
  crate is nowhere near a stability commitment until much later.
- **Zero runtime dependencies**. `[dependencies]` stays empty.
  Dev-deps are fine (`quickcheck` for the differential harness,
  whatever miri runner wrappers we need). Never add `bytemuck`,
  `smallvec`, etc. "just for milestone 1" — defeats the entire point.
- **Build posture**: `#![cfg_attr(not(test), no_std)]` +
  `extern crate alloc`, backed by `alloc::alloc::Global`. No custom
  allocator. No syscalls. No `std::` imports.
- **Module layout mirrors `std`**: `koala_std::collections::HashMap`,
  `koala_std::collections::HashSet`, and (milestone 2) the
  browser-string family module. Easier differential testing and
  seeing where std actually puts things is itself learning.
- **Public API mirrors `std`** for each type where a `std`
  counterpart exists — same method names, same signatures, same
  semantics. Deviate only with a justified reason documented in
  code. The full analysis lives in `koala-std-vec-design.md`
  (kept under that name as a retrospective on the `Vec<T>`
  experiment even though `Vec<T>` itself has been removed).
- **Rust edition 2024**, MSRV = latest stable at crate-creation
  time. Bump forward aggressively — this is an experimental project
  not shipped to external consumers.
- **Testing floor**: `quickcheck` differential vs `std` counterparts,
  `miri` in CI (mandatory for unsafe-heavy code), explicit ZST
  tests, explicit drop-ordering tests once anything owns heap data.
- **Workspace lint override**: the workspace denies `unsafe_code`.
  `koala-std` overrides that at the crate root with
  `#![allow(unsafe_code)]` because denying unsafe would make the
  entire crate's purpose impossible. Unsafe is reviewed via miri and
  differential testing, not via lint.

## Milestone 1 — Foundations: hash-based collections

**Question answered:** How does a hash table actually work at the
level of "open addressing, tombstones, load factor, hash function
choice" — the one milestone-1 type whose design is genuinely
algorithmic rather than a thin layer over the allocator.

| Deliverable | Teaches |
|---|---|
| `HashMap<K, V>` | Hash table internals, load factor, tombstones for deletion, hash function choice, resize triggers |
| `HashSet<T>` | Free wrapper over `HashMap<T, ()>` |

The milestone-1 scope was originally much broader — it included
`Vec<T>`, `Box<T>`, and `String`. After a retrospective on
2026-04-13 (see "Considered and rejected" below) those three were
dropped: `Box` and `String` never entered production scope, and
`Vec<T>` was built for learning but deleted from the source tree
after a scan of the Koala codebase confirmed it provided no
production value and the milestone-3 types (`SmallVec`, `ThinVec`,
`ArenaVec`) do not require it as a foundation.

`RawVec<T>` — the private allocation primitive that backed
`Vec<T>` — is kept as `pub(crate)` dead code in the crate because
milestone-3 collection types will use it as a shared building
block. The investment in `RawVec`'s allocation/grow/drop/ZST
handling is reusable even though `Vec<T>` itself was not.

### Locked design decisions for milestone 1

- **`HashMap` algorithm**: open-addressing with linear probing +
  tombstones. Matches Ladybird's `AK/HashTable`. Robin Hood hashing
  and SwissTable are explicit non-goals — both are rewrite
  candidates for a later milestone if interest remains.
- **`HashMap` hash function**: **FxHash** (single-multiply, what
  rustc uses for internal maps). Not DoS-resistant but we don't
  need that for an internal container. SipHash is ~5× slower and
  out of scope. Tradeoff noted in a comment; no further discussion.

### Exit criteria (milestone 1)

- Differential `quickcheck` vs `std::collections::HashMap` passes
  for every public operation.
- `miri` clean in CI (mandatory for unsafe-heavy code).
- Explicit ZST tests for `HashMap<(), V>` and `HashMap<K, ()>` —
  ZSTs are where hand-rolled collections most commonly break.
- Explicit drop-ordering tests using a drop-recorder type.

### Explicitly not in milestone 1

Anything shared (`Rc`), anything interior-mutable (`RefCell`),
anything concurrent (`Arc`, `Mutex`), anything OS-dependent,
multiple string types, arena allocator, extended collections.

## Considered and rejected — milestone-1 types we decided not to build

Locked 2026-04-13 after the post-Vec retrospective. Recording
the reasoning so future sessions don't re-raise these without new
information.

### `Box<T>` — no learning, no production value

A hand-rolled `Box<T>` would be roughly `{ ptr: NonNull<T>,
_marker: PhantomData<T> }` with `alloc` in `new`, `dealloc` in
`Drop`, and `Deref`/`DerefMut`. The allocation/deallocation
pattern is a strict subset of what `RawVec<T>` already exercises,
so there is no new skill to acquire. No milestone-2 or
milestone-3 type depends on having a custom `Box` (they all use
`std::rc::Rc` or `Arc` for shared ownership, or they are
stack-allocated structs that wouldn't use `Box` anyway). There
is no Koala-specific optimization that a custom `Box` would
unlock — `std::Box<T>` is a solved problem and rebuilding it
would be pure completionism.

**Decision**: dropped from the roadmap entirely. `std::Box<T>`
is fine wherever we need owned heap allocation.

### `String` — the browser-string family doesn't need it

A hand-rolled UTF-8-validated `String` is roughly `Vec<u8>` plus
UTF-8 validation plus a few char-oriented methods. The
validation is genuinely useful to write once, but it is not a
project's worth of learning, and it is not something we would
want to do "our way" because WHATWG's UTF-8 decoding algorithm
is rigidly specified. The only real question is whether
milestone 2's browser-grade string types (`FlyString`,
`StringBuilder`, `Utf16String`) need it as a foundation, and
they do not:

- **`FlyString`** needs a refcounted immutable UTF-8 slice.
  `std::rc::Rc<str>` already provides exactly that; what
  `FlyString` adds is the *intern table*, which is a `HashMap`,
  not a `String` extension.
- **`StringBuilder`** is `Vec<u8>` (std's) with UTF-8-aware
  append methods. No custom `String` needed.
- **`Utf16String`** is `Vec<u16>` (std's) with surrogate-pair
  handling. Its element type is `u16`, not `u8`, so a custom
  UTF-8 `String` would not even be relevant.

**Decision**: dropped from the roadmap entirely. The browser-
string family in milestone 2 uses `std`'s `Vec<u8>` and
`Vec<u16>` directly as backing storage.

### `Vec<T>` — built for learning, removed retrospectively

`Vec<T>` was built across tasks #3–#8 (milestone 1 core API plus
v1.1 extensions). The code was correct, well-tested, and
matched `std::vec::Vec` almost exactly. The retrospective found
that:

1. **Production value in the Koala codebase is zero.** A scan of
   `koala-html`, `koala-css`, `koala-dom`, `koala-browser`,
   `koala-js`, and the binaries on 2026-04-13 surfaced no
   recurring `Vec` patterns that would benefit from a custom
   method or type. Every Vec-shaped idiom in the existing code
   is either `std`-idiomatic or a borrow-checker workaround that
   a custom Vec cannot fix.
2. **Milestone-3 types do not depend on it.** `SmallVec<T, N>`
   has its own `[MaybeUninit<T>; N]` inline storage, `ThinVec<T>`
   uses a different `{ptr} + header-stored metadata` layout, and
   `ArenaVec<T>` ties directly into `BumpAllocator`. None of
   them inherit from or share code with `Vec<T>`. The foundation
   they do share is `RawVec<T>`, which is kept.
3. **The learning did happen.** `RawVec<T>`'s allocation logic,
   ZST handling, panic-safety reasoning for `Drop`, and the
   `IntoIter<T>` + `DoubleEndedIterator` patterns were all
   genuine new territory. They are captured in git history and
   in the surviving `raw_vec.rs` source file.

**Decision**: `Vec<T>` is deleted from the source tree. `RawVec<T>`
stays as `pub(crate)` dead code until milestone 3 consumers
arrive. The v1.0 and v1.1 Vec commits are preserved in git
history for reference and as a learning artifact.

## Milestone 2 — Browser-grade string family

**Question answered:** Why do browsers have multiple string types,
and what does each one actually optimize for?

| Deliverable | Teaches |
|---|---|
| `FlyString` (interned strings, `std::rc::Rc`-backed) | Global intern table, O(1) equality via pointer compare, dedup semantics |
| `StringBuilder` | Mutable UTF-8 building on top of milestone 1's `Vec<u8>` |
| `Utf16String` + `Utf16View` | ECMAScript strings are UTF-16; surrogate pair handling; not converting on every op |
| `CowStr<'a>` (≈ `Cow<str>`) | Borrow-or-own pattern |

### Why this slot

Highest-leverage work for Koala itself. DOM attribute names, CSS
identifiers, and element tag names all want interning; a future JS
engine wants `Utf16String` natively. Milestone 2 is the first
milestone where `koala-std` actively improves Koala, not just
teaches a concept.

### Uses `std::rc::Rc`

`FlyString`'s refcounted backing uses `std::rc::Rc` directly — we
chose not to rebuild refcounting (see "deferred to std" below).

### Explicitly not in milestone 2

`ByteString` (legacy raw bytes) — Ladybird only has it for legacy
reasons and is phasing it out. We start clean.

### Exit criteria (milestone 2)

- `FlyString` dedup verified: two identical input strings produce
  identical `Rc` pointers (test via `Rc::ptr_eq`).
- `Utf16String` roundtrips through a surrogate-pair corpus without
  corruption.
- Differential tests against `std::String` where semantics align.

## Milestone 3 — Arena allocator + extended collections

**Question answered:** When does a custom allocator actually help,
and what do the browser-specific vector types teach beyond `std`'s
`Vec` and `HashMap`?

### Required deliverables

| Deliverable | Teaches |
|---|---|
| `BumpAllocator` / `Arena<T>` | Allocator design at its simplest; lifetime tying via `'arena`; when arenas beat `Global` |
| `SmallVec<T, const N: usize>` | Inline storage with heap fallback; union-backed uninit arrays; why "match std's API" doesn't apply to types std doesn't have |
| `ThinVec<T>` | Storing length and capacity at the allocation header so the outer struct is one pointer; the `nsTArray` pattern Firefox uses for empty-common DOM attribute lists |
| `ArenaVec<T>` | Ties into `BumpAllocator` directly; never reallocates; O(1) bulk invalidation via arena reset |

**Why the arena is the headline feature:** every `LayoutBox` in
`koala-css` currently allocates through `Global`. A bump allocator
is both a real optimization target (per
`rasterizer-future-work.md`) and a teachable allocator-design
project without the complexity of a general-purpose allocator. High
marginal value to the main project.

**Why the three vector types live here instead of milestone 1:**
they are where the real "improve on `std`" energy belongs. `std`
does not ship `SmallVec`, `ThinVec`, or `ArenaVec` — these types
cover workloads that `std::Vec<T>`'s single-layout design
deliberately leaves on the table. They are the actual production-
value types in the `koala-std` plan. Each of them is built
directly on `RawVec<T>` (the milestone-1 allocation primitive
that survived the `Vec<T>` removal) rather than on a full `Vec<T>`.

### Demand-driven stretch goals

| Stretch | Gate |
|---|---|
| `VecDeque<T>` | Profile shows Koala has a bounded ring-buffer workload |
| `BinaryHeap<T>` | A scheduling / priority-queue use case appears |
| `BTreeMap<K, V>` + `BTreeSet<T>` | An ordered-iteration use case (e.g., CSS `@import` ordering, CSS cascade layers) |

**Stretch collections require a data-driven justification before
being built.** Confirmed 2026-04-13: we implement them eventually
but not on spec. Each one gets built only when profiling or a
concrete Koala use case identifies a need. Speculative "we might
need it" is not sufficient. This matches the CLAUDE.md rule:
"Profile before micro-optimizing. Data-driven decisions over
intuition."

### Explicit non-goal

`LinkedList<T>` — famous "when would you actually use this" type
with low learning payoff. Skip permanently unless something
genuinely needs it.

## Deferred to Rust's `std` — not rebuilt in `koala-std`

Four categories of `std` work were considered as milestones and
deferred. The reasoning matters because future sessions will be
tempted to re-raise them.

### Refcounting & interior mutability (`Rc`, `Weak`, `Cell`, `RefCell`)

Available in `alloc::rc` and `core::cell` — free in `no_std + alloc`.
Rebuilding them is genuine learning but doesn't unlock anything
Koala needs that `std` doesn't already provide. `koala-std`
milestone 2 uses `std::rc::Rc` directly for `FlyString`'s backing.
Rebuilding remains a possible future side project outside `koala-std`
if the interest returns.

### Concurrency (`Arc`, `Mutex`, `RwLock`, atomics)

Available in `core::sync::atomic` and `alloc::sync::Arc`. `Mutex` /
`RwLock` are in `std::sync` and will come from `koala-os` or `std`
when needed. Atomic memory ordering is a genuinely hard topic but
it's orthogonal to Koala's rendering work — not worth gating
rendering progress on it.

### Formatting (`Display`, `Debug`, `fmt::Write`, `format!`, `write!`)

**Cannot be meaningfully replaced.** These are tied to lang items:
`core::fmt::Arguments` is `#[lang = "format_arguments"]`,
`#[derive(Debug)]` hardcodes the path to `core::fmt::Debug`, and
`write!` / `writeln!` / `format!` macros dispatch to these exact
types. Any "koala-std Display" trait would be a parallel hierarchy
that doesn't interop with `{}` formatting anywhere else in the
ecosystem. All of it is available in `no_std + alloc` already —
`format!` lives in `alloc::fmt` on top of `core::fmt::Arguments`.
Zero rebuild value, full availability → use `std`.

### Non-OS IO traits (`io::Read`, `io::Write`, `io::Seek`, `BufReader`, `BufWriter`, `Cursor`)

These live in `std::io`, NOT in `core` or `alloc`, so they are
genuinely absent from `no_std + alloc`. However: defining the trait
family in `koala-std` with zero concrete implementations is "shape
without substance" — the learning comes from *implementing*
`Read` / `Write` over a file handle or socket, and that means
syscalls, which means `koala-os`. **Decision**: move
`Read` / `Write` / `Seek` / `BufReader` / `BufWriter` / `Cursor` into
the initial cut of `koala-os` whenever it appears, built against
real file/socket impls from the start. `koala-std` never gets an
IO trait family.

## Progress log

### 2026-04-13 — Milestone 1 start

- **Commit 1** (`1874772`): scaffolded `koala-std` with
  `no_std + alloc`, zero runtime deps, workspace wiring, first
  GitHub Actions workflow (`miri.yml`) running
  `cargo miri test -p koala-std` with
  `-Zmiri-strict-provenance`.
- **Commit 2** (`19958e9`): `RawVec<T>` with full ZST handling,
  doubling grow with floor of 4, `realloc`-in-place on grow,
  panic-safe `Drop`, overflow detection at both `checked_mul` and
  `Layout::array` levels. 13 unit tests including explicit ZST
  drop-of-dangling-pointer and capacity-overflow tests.
- **Commits 3–8** (tasks #3–#8, `8050222` through `021cfd2`):
  `Vec<T>` built across eight commits covering the v1.0 minimal
  API, the full trait surface (`Debug` / `PartialEq` / `Eq` /
  `Hash` / `Clone` / `Default` / `FromIterator` / `IntoIterator`),
  a quickcheck differential harness, and the v1.1 extensions
  (`with_capacity`, `reserve` family, `shrink_to_fit`,
  `push_within_capacity`, `truncate` / `clear` / `insert` /
  `remove` / `swap_remove` / `retain` / `dedup`, `extend` /
  `extend_from_slice`, `IntoIter<T>` with `DoubleEndedIterator` /
  `ExactSizeIterator` / `Drop`, and `IntoIterator for Vec<T>`).
  Reached 79 passing tests (13 unit + 33 integration + 33
  doc-tests), `miri` clean, clippy clean. Genuinely matched
  `std::vec::Vec` for every public method shipped.

### 2026-04-13 — Retrospective and `Vec<T>` removal

After task #8 shipped, an explicit retrospective on the crate's
scope found that `Vec<T>` had zero production value in Koala
(see "Considered and rejected — `Vec<T>`" above) and that none
of the milestone-3 types would use it as a foundation. `Box<T>`
and `String` were reassessed at the same time and also dropped
from the roadmap. The decision was to delete `Vec<T>` from the
source tree while keeping `RawVec<T>` as a dead-code primitive
for the milestone-3 consumers. The v1.0 and v1.1 Vec commits
remain in git history as a learning artifact.

- **Next**: milestone 1 revised to `HashMap<K, V>` +
  `HashSet<T>` only. After that, milestone 3's `BumpAllocator`,
  `SmallVec`, `ThinVec`, and `ArenaVec`. Milestone 2's browser-
  string family lands between those, after `HashMap` exists
  (because `FlyString`'s intern table needs it).
