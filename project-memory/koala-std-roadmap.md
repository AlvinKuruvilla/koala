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
- **Module layout mirrors `std`**: `koala_std::vec::Vec`,
  `koala_std::boxed::Box`, `koala_std::string::String`,
  `koala_std::collections::HashMap`, `koala_std::collections::HashSet`.
  Easier differential testing and seeing where std actually puts
  things is itself learning.
- **Public API mirrors `std`** for milestone 1 — same method names,
  same signatures, same semantics. Deviate only with a justified
  reason documented in code. The full analysis of where to deviate
  lives in `koala-std-vec-design.md`.
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

## Milestone 1 — Foundations: owned heap storage

**Question answered:** How does owned, unshared, heap-backed storage
actually work in Rust?

| Deliverable | Teaches |
|---|---|
| `Vec<T>` | Raw pointers, `GlobalAlloc`/`Layout`, `Drop` ordering, `Iterator`/`IntoIterator`, `Deref`/`DerefMut`, zero-sized types |
| `Box<T>` | `Unique` ownership; nearly free after `Vec`'s allocator work |
| `String` (UTF-8 validated) | Invariant preservation, byte-vs-char distinction, `Vec<u8>` layering |
| `HashMap<K, V>` | Hash table internals, load factor, tombstones for deletion, hash function choice |
| `HashSet<T>` | Free wrapper over `HashMap<T, ()>` |

### Locked design decisions for milestone 1

- **`HashMap` algorithm**: open-addressing with linear probing +
  tombstones. Matches Ladybird's `AK/HashTable`. Robin Hood hashing
  and SwissTable are explicit non-goals — both are rewrite
  candidates for a later milestone if interest remains.
- **`HashMap` hash function**: **FxHash** (single-multiply, what
  rustc uses for internal maps). Not DoS-resistant but we don't
  need that for an internal container. SipHash is ~5× slower and
  out of scope. Tradeoff noted in a comment; no further discussion.
- **`RawVec<T>` split**: `Vec<T>` is built on a private `RawVec<T>`
  helper that owns the `ptr + cap + marker`, mirroring std's
  architecture. `RawVec` will be reused by `String` (milestone 1)
  and potentially `VecDeque` (milestone 3 if justified).

### Exit criteria (milestone 1)

- Differential `quickcheck` vs `std` counterparts passes for every
  public operation on every type.
- `miri` clean in CI (mandatory for unsafe-heavy code).
- Explicit ZST tests for `Vec<()>`, `HashMap<(), ()>`, etc. — ZSTs
  are where hand-rolled collections most commonly break.
- Explicit drop-ordering tests using a drop-recorder type.

### Explicitly not in milestone 1

Anything shared (`Rc`), anything interior-mutable (`RefCell`),
anything concurrent (`Arc`, `Mutex`), anything OS-dependent,
multiple string types, arena allocator, extended collections.

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
and what do less common collections teach beyond `Vec` and
`HashMap`?

### Required deliverable

| Deliverable | Teaches |
|---|---|
| `BumpAllocator` / `Arena<T>` | Allocator design at its simplest; lifetime tying via `'arena`; when arenas beat `Global` |

**Why arena is the headline feature:** every `LayoutBox` in
`koala-css` currently allocates through `Global`. A bump allocator
is both a real optimization target (per
`rasterizer-future-work.md`) and a teachable allocator-design
project without the complexity of a general-purpose allocator. High
marginal value to the main project.

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
- Next: `Vec<T>` wrapper with `push`/`pop`/`Drop`/`Deref<[T]>`. See
  `koala-std-vec-design.md` for the design decisions.
