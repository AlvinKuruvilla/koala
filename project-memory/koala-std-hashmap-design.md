---
created: 2026-04-13
area: koala-std::collections
status: open — design locked, implementation not started
---

# koala-std `HashMap<K, V>` design

This document is the source of truth for `koala_std::collections::HashMap`
(milestone 1, revised). It consolidates the research survey, the seven
locked design decisions, the codebase usage scan, the implementation
checklist, and the future-state notes for v2 and beyond. Future sessions
should start here rather than re-deriving anything from conversation
history.

## Scope and non-goals

### In scope for v1

- A hand-rolled `HashMap<K, V, H = FxHasher>` with Robin Hood hashing,
  inline probe-length tracking, backshift deletion, a cached `u32` hash
  fragment per bucket, and a default 70% load factor.
- `HashSet<T>` as a free wrapper over `HashMap<T, ()>`.
- A `FxHasher` implementation (~30 lines) exposed as the default hasher
  and re-exported for external use.
- Full std-parity API surface for `HashMap` and `HashSet`.
- Differential quickcheck harness against `std::collections::HashMap`,
  explicit ZST tests, explicit drop-ordering tests, `miri` clean in CI.

### Explicitly not in scope for v1

- **SwissTable / SIMD control-byte design.** Architecture-specific,
  complex, and `hashbrown` already fills that role. Revisit only if
  profiling says the simpler Robin Hood design is the bottleneck.
- **F14 chunk-with-overflow-counters design.** Elegant for its own
  structure but doesn't map to Robin Hood's flat probe-length layout.
- **Cuckoo hashing** and other multi-hash-function schemes. Complexity
  exceeds our learning budget.
- **Elastic / funnel hashing** (Farach-Colton / Krapivin / Kuszmaul
  2024). Deferred to **milestone 2 or later** as a parallel sibling
  type `ElasticHashMap<K, V>`, not as a replacement for the Robin Hood
  v1 baseline.
- **SoA storage layout** + SIMD probe-length scanning. Deferred to v2
  as an optimization pass, gated on profiling.
- **Conditional cached-hash** (AK's `may_have_slow_equality_check`
  trait-based selection). Defaults to unconditional `u32` cache for
  simplicity; revisit if profiling shows the overhead hurts small-key
  maps.
- **AK-style additional methods** (`find(hash, predicate)`, `ensure`,
  `take`, `set_from`, `remove_all_matching`). The codebase scan
  (below) found zero Koala demand for any of them. Add only when a
  concrete use case surfaces.
- **Custom `Allocator` trait parameterization.** `koala-std` hardcodes
  the global allocator in v1. The arena-backed collection story lives
  in milestone 3 as `ArenaVec<T>`, not as a `HashMap<K, V, A>` retrofit.

## Seven locked design decisions

Locked 2026-04-13 after the design conversation. Each decision includes
the rationale and the rejected alternatives so future sessions see why.

### 1. Algorithm — Robin Hood hashing with inline probe-length tracking

**Decision**: open addressing with Robin Hood displacement. Each bucket
carries an implicit or explicit probe length. On insert, if the element
being inserted has a longer probe length than the element in the current
slot, they swap (the richer element yields to the poorer one). This
evens out probe-length variance so that no single lookup is
disproportionately expensive.

**Inline probe-length encoding** (borrowed directly from
`AK/HashTable.h`): the bucket's state byte doubles as a probe-length
value. In AK's scheme, `0` means empty, `1..254` encodes
`probe_length = value - 1`, and `255` means "probe length exceeds 253,
recompute from hash." This packs state and distance into a single byte
per bucket at the cost of a rare recompute for very long probe chains.

**Rejected alternatives**:

- **Plain linear probing + tombstones**: simpler (~400 lines) but
  unbounded probe variance means individual lookups can be arbitrarily
  slow when the table is full, and tombstones accumulate and need
  periodic cleanup. Robin Hood's probe-length invariant is worth the
  additional ~200 lines of code.
- **SwissTable (hashbrown-style, SIMD + control bytes)**: current state
  of the art for throughput but architecturally complex (~1500+ lines
  with SIMD fallbacks) and `hashbrown` already exists. Writing a worse
  version of a solved problem is not a good use of the learning
  budget.
- **Facebook F14 chunks with overflow counters**: elegant for its own
  chunk design but the overflow-counter trick only works inside a
  fixed-size chunk. Robin Hood's flat table has no natural place to
  hang the counter.
- **Cuckoo hashing**: two hash functions, O(1) worst-case lookup, but
  complicated insert logic with potential rehash loops. Too much
  engineering for v1.
- **Hopscotch hashing**: Robin Hood's closest competitor. Slightly
  different invariants (each element within a fixed "neighborhood" of
  its home position). Not obviously better for our workload; Robin
  Hood is better-known and matches AK.

### 2. Hash function — FxHash

**Decision**: implement FxHash as the default hasher. Single-multiply +
rotate + xor per `u64` chunk, ~30 lines of code. This is the same hash
function rustc uses internally for its non-adversarial data
structures.

**Signature**:

```rust
pub struct FxHasher {
    hash: usize,
}

impl Hasher for FxHasher {
    fn write(&mut self, bytes: &[u8]) { /* ... */ }
    fn finish(&self) -> u64 { self.hash as u64 }
}
```

The constant `FX_SEED = 0x51_7cc1_b727_220a_95` (rustc's pick) and a
rotate-left of 5 per absorb cycle produce acceptable distribution for
non-adversarial keys with minimal work. It's genuinely ~30 lines.

**Rejected alternatives**:

- **SipHash-1-3**: DoS-resistant but slow. Was `std::HashMap`'s default
  before Rust 1.36, and still the choice for programs that take
  untrusted input. `koala-std`'s workload is internal containers owned
  by Koala itself — there is no adversarial input path.
- **foldhash** (hashbrown's current default since 0.15): non-DoS-
  resistant, very good quality, very fast — but ~400 lines of code
  with a sophisticated quality/speed tuning story. The complexity is
  unjustified for our simplicity goal. foldhash is the right choice
  for `std::HashMap` because `std`'s user base includes HTTP servers;
  it is not the right choice for a hand-rolled educational foundation.
- **ahash**: AES-NI accelerated on x86, very fast — but architecture-
  dependent. Same problem as SIMD.
- **SeaHash**: another non-cryptographic option. Slightly slower than
  FxHash without a compelling quality advantage for our workload.
- **Identity hash / raw hash** for integer keys: sometimes proposed for
  maps with `u64` keys. Works until it doesn't. FxHash handles this
  case well enough and avoids the "surprise catastrophic collision
  when your input happens to cluster" failure mode.

**Pluggability**: `HashMap<K, V, H = FxHasher>` accepts any `Hasher`.
Callers who need DoS resistance for some reason can plug in
`std::hash::DefaultHasher` or `siphasher::sip::SipHasher13` without any
koala-std changes.

### 3. Load factor — 70%

**Decision**: grow when `used_buckets / capacity >= 0.7`. Minimum
capacity 8, double on grow.

**Rationale**: 70% is what AK uses, and it's a good Robin Hood
operating point. Robin Hood hashing handles high load factors better
than plain linear probing (variance is bounded), but going above ~80%
starts to bite into average probe length even with Robin Hood.

**Rejected alternatives**:

- **66% (2/3, Python dict)**: lower memory pressure, marginally more
  collisions headroom. Fine choice, just more conservative than we
  need.
- **87.5% (SwissTable)**: only works because SIMD group probing
  amortizes the cost of long probe chains across 16-element SIMD
  groups. Without SIMD, 87% degrades non-SIMD probe performance
  meaningfully.
- **85% (F14 12/14)**: same reasoning — F14's chunk design permits it,
  our flat Robin Hood does not.

The grow threshold is configurable at the type level if we want to
parameterize later, but v1 bakes in 70%.

### 4. Deletion — backshift (no tombstones)

**Decision**: on delete, walk forward in the probe sequence and shift
each displaced element (probe_length > 0) back into the preceding slot,
decrementing each element's probe length by 1. Stop on reaching an
empty slot or a home-position element (probe_length == 0).

**Pseudocode**:

```
delete(i):
    loop:
        next = (i + 1) & mask
        if buckets[next].state == Empty: break
        if buckets[next].probe_length == 0: break  // home; don't move
        buckets[i] = buckets[next]
        buckets[i].probe_length -= 1
        i = next
    buckets[i].state = Empty
```

This is the canonical Robin Hood delete. Every remaining element stays
at its correct probe-length distance, so subsequent lookups benefit.
No tombstones accumulate, no periodic cleanup, no load-factor drift.

**Rejected alternatives**:

- **Tombstones with periodic cleanup** (linear probing classic): works
  but introduces operational complexity — when to clean up, how to
  detect that cleanup is needed, what the lookup sees during cleanup.
  Robin Hood sidesteps all of this.
- **F14-style overflow counters**: requires chunk structure, which we
  don't have. Cannot port directly.
- **Lazy deletion / mark-and-sweep**: same failure mode as tombstones
  but worse.

### 5. Storage layout — Array of Structs (AoS)

**Decision**: each bucket is a contiguous struct
`{ state: u8, hash: u32, entry: MaybeUninit<(K, V)> }`. The buckets
live in a single heap allocation as an array. On a 64-bit target with
`K = u64, V = u64`, each bucket is 1 + 3 (padding) + 4 + 8 + 8 = 24
bytes, which fits nicely in cache lines.

**Rationale**: Robin Hood's hot loop on every probe touches all three
fields of a bucket together — state to check "is this slot live?",
hash fragment to filter before full eq, then the key to confirm match.
AoS keeps these in the same cache line. SoA (parallel arrays) would
split them into three separate allocations, which costs extra cache
misses on the hot path.

SoA's benefit is SIMD scanning of just the state bytes — 16 states in
one SIMD register instead of 16 full cache lines. That benefit is real
for SwissTable/F14 designs where the probe is fundamentally SIMD; it's
mostly wasted for Robin Hood, where the probe is sequential and
touches every field per step.

**Rejected alternatives**:

- **SoA**: deferred to v2 as an optimization candidate if we later add
  SIMD probe-length scanning. Listed in "Future state" below.
- **Hybrid**: separate state bytes, combined hash+entry. Neither fish
  nor fowl.

### 6. Cached hash — unconditional `u32` fragment per bucket

**Decision**: every bucket stores a `u32` fragment of the full hash.
On lookup, the probe loop compares the fragment first. Only if the
fragment matches do we call `K::eq` for a full equality check.

**Memory cost**: 4 bytes per bucket. For a `HashMap<String, V>` with
1000 entries at 70% load factor, that's ~5.7 KB extra — cheap.

**Rationale**: the codebase scan (section below) shows `HashMap<String,
V>` is used in 5+ places in Koala, where `String::eq` is O(key length).
The fragment check skips all false-positive full equality calls in the
common case where two keys collide on `hash & mask` but differ
elsewhere. The win scales with key length, which for DOM attribute
names and CSS identifiers can easily be 10–50 bytes. The 4-byte
overhead buys a large multiplier on those.

**Why a fragment and not the full hash?**: storing the full 64-bit
hash would be 8 bytes per bucket instead of 4, which is 2× the
overhead on small-key maps. The probability of two keys colliding on
both their slot *and* a 32-bit fragment without being truly equal is
~1/2^32 — effectively never in a single program run. The fragment
check is a filter, not a proof.

**Rejected alternatives**:

- **Conditional storage via AK's `may_have_slow_equality_check`
  trait**: genuine savings for `HashMap<u64, V>` and similar maps
  where equality is already cheap, but requires a trait the user has
  to know about and monomorphization branching that's non-trivial to
  get right in Rust. v1 defaults to unconditional for simplicity;
  v1.1 can add conditional if profiling shows the overhead bites.
- **No cached hash at all**: optimizes for `HashMap<u64, V>` at the
  cost of every string-keyed map. The scan shows string-keyed maps
  are common enough to refuse this trade.
- **Full 64-bit cached hash**: 8 bytes per bucket is a 50% metadata
  overhead on small-key maps. Not worth the collision-false-positive
  improvement from 2^32 to 2^64.

### 7. API shape — mirror std, add on demand

**Decision**: v1 mirrors `std::collections::HashMap`'s public API
exactly. Every method `std` has, we have, with the same signature and
semantics. The differential quickcheck harness depends on this.

**Methods added to v1 beyond std-parity**: none. The codebase scan
(below) found zero Koala demand for `find(hash, predicate)`, `ensure`,
`take`, `set_from`, `remove_all_matching`, or any other AK-style
convenience. They get added if and when a concrete use case surfaces,
not speculatively.

**Methods explicitly *not* added**: same list. The principle is the
same one the Vec scan established: the codebase uses `std::HashMap`
idiomatically and doesn't work around missing methods. Respecting
that means not inventing new ones.

**Generic parameters**: `HashMap<K, V, H = FxHasher>`. `K` must
implement `Hash + Eq`. `V` has no trait bounds beyond what's
needed for specific operations (`Clone` for `.clone()`, etc.). `H`
defaults to `FxHasher` but can be any `Hasher`.

## Research survey — what we looked at and what we learned

The full multi-source research survey lives in the conversation
history; this section is the compressed version of what mattered for
the design decisions above.

### Production hash tables

**[`hashbrown`](https://github.com/rust-lang/hashbrown) and
[Abseil SwissTable](https://abseil.io/about/design/swisstables)** —
the state of the art for throughput. 57/7 H1/H2 hash split, 1-byte
control bytes per slot, SIMD group probing (16 slots at a time via
SSE2), ~87.5% load factor, tombstones with gradual cleanup. `std::
HashMap` has used `hashbrown` since Rust 1.36. Recent versions use
`foldhash` as the default hasher. **Considered and rejected for
koala-std**: SIMD is architecture-specific, the code is complex, and
writing a worse port of `hashbrown` is not a good use of learning
time.

**[Ladybird `AK/HashTable`](https://github.com/LadybirdBrowser/ladybird/blob/master/AK/HashTable.h)**
— the model we're copying. Robin Hood hashing with probe length stored
inline in the bucket state byte (values 1..254), `Free = 0`,
`CalculateLength = 255` sentinel for very long probes. 70% load factor,
minimum capacity 8, double on grow. Conditionally caches the hash in
the bucket based on whether `K::eq` is expensive. Supports optional
ordered iteration via a linked-list of buckets (which we're skipping
for v1). **This is the direct reference model for koala-std's v1.**

**[Facebook F14](https://engineering.fb.com/2019/04/25/developer-tools/f14/)**
— Meta's alternative to SwissTable. 14-way chunk probing with SIMD,
~85% load factor, **reference-counted overflow bits** instead of
tombstones. When deleting, decrement the counter on every chunk in the
probe sequence; when a counter hits zero, slots in that chunk return to
fully empty without a cleanup pass. Elegant for F14's chunk design.
**Considered and rejected**: doesn't map to Robin Hood's flat layout.
The *principle* (deletions should not degrade the table over time) is
worth preserving, and Robin Hood with backshift achieves it via a
completely different mechanism.

**[CPython dict](https://www.laurentluce.com/posts/python-dictionary-implementation/)**
— open addressing with perturbation-based probe sequence that mixes
high bits of the hash into the probe step. 2/3 load factor. Compact
dict representation since Python 3.6 separates the sparse hash table
from a dense insertion-ordered entries array. **Considered**: the
perturbation trick is a nice micro-optimization on weak hashes. Not
relevant because FxHash's avalanching is sufficient for our workload.
The compact-dict trick is how you get insertion order preservation
without losing O(1) — worth knowing about if we ever need ordered
iteration.

**Go map**, **Swift Dictionary**, **Java HashMap**, **C++
`std::unordered_map`** — not fetched in detail this round. Go uses
bucket + overflow chains with incremental resize; Swift is probably
open-addressed; Java is separate chaining with tree fallback for
buckets with ≥8 collisions (since Java 8); C++'s `unordered_map` is
node-based separate chaining per the standard's reference-stability
requirements and is considered slow compared to alternatives. None of
these are model candidates for koala-std.

**Clojure `PersistentHashMap`** — HAMT-based persistent data
structure. O(1) clone, O(log n) updates via structural sharing.
Different category; not a model candidate.

### The 2024/2025 theoretical breakthrough

Farach-Colton, Krapivin, and Kuszmaul's
[Optimal Bounds for Open Addressing Without Reordering](https://arxiv.org/abs/2501.02305)
(FOCS 2024) **disproves Yao's 1985 conjecture** that uniform random
hashing is optimal for open addressing without element reordering. The
paper introduces two algorithms that beat the O(1/δ) bound:

- **Elastic hashing** — amortized *O*(1) (~2–3 probes), worst-case
  *O*(log(1/δ)) (~7 probes even at 99% full). Uses a "multi-floor"
  architecture where lookups start at a ground floor and progressively
  ascend to smaller floors on failure, eventually falling back to a
  linear scan.
- **Funnel hashing** — amortized *O*(log(1/δ)) (~7 probes at 99%
  full), worst-case *O*(log²(1/δ)). Monotonically ascends without
  retreat.

**Practical impact**: the bound improvement is asymptotic at very high
load factors (>95%). At typical production load factors (70–90%), the
new algorithms are likely no faster than SwissTable in practice and may
be slower due to constant factors. The authors themselves say in the
Quanta coverage that the results "may not lead to any immediate
applications."

**What this means for koala-std**: we do **not** build elastic hashing
as the v1 algorithm. We *do* flag it as a **v2 sibling type**
(`ElasticHashMap<K, V>`) because (a) it's genuinely novel, (b) it would
be one of the first Rust ports of the algorithm, and (c) it's the kind
of thing this project could be known for as a learning showcase. See
"Future state" below.

## Codebase scan — HashMap usage patterns in Koala

A background scan on 2026-04-13 surveyed all `HashMap` / `HashSet`
usage in `crates/koala-common`, `koala-dom`, `koala-html`, `koala-css`,
`koala-js`, `koala-browser`, plus the binaries. 29 distinct
`HashMap`/`HashSet` declarations across ~10 files.

### Key/value type distribution

| Key → Value | Count | Locations | Notes |
|---|---|---|---|
| `HashMap<NodeId, ComputedStyle>` | 4 | `cascade.rs:123`, `display_list_builder.rs:54`, `layout_box.rs:1004`, `browser/lib.rs:70` | The primary CSS cascade output. NodeId is cheap to hash (u64). Good fit. |
| `HashMap<String, LoadedImage>` | 3 | `browser/lib.rs:86`, `renderer.rs:157`, `browser/lib.rs:231` | Image-by-src cache. String-keyed, benefits directly from the cached-hash optimization. |
| `HashMap<String, String>` (`AttributesMap`) | 2+ | `koala-dom/src/lib.rs:27` (type alias), used throughout cascade and layout | DOM attribute maps. See "type-choice finding" below. |
| `HashMap<String, Vec<ComponentValue>>` | 2 | `style/computed.rs:754`, `substitute.rs` | CSS custom properties (`--var-name`). String-keyed, benefits from cached hash. |
| `HashMap<&'static str, &'static str>` | 1 | `named_character_references.rs:2257` | 2,231-entry static HTML entity table. Perfect HashMap fit. |
| `HashMap<NodeId, (f32, f32)>` | 1 | `browser/lib.rs:232` | Image intrinsic dimensions. Good fit. |
| `HashMap<usize, (f32, f32)>` | 1 | `table.rs:547` | Table row-group bounds. Trivial. |

**Implication for the cached-hash decision**: 5+ HashMaps are
string-keyed. For those maps, every failed probe-fragment comparison
avoids a full `String::eq` call, which is O(key length). The cached-
hash overhead (4 bytes per bucket) is unambiguously worth it for
Koala's actual workload.

### Pattern findings — no new methods justified

The scan looked for four categories of patterns that could justify
additional HashMap methods. All four came back empty:

1. **`.entry().or_insert_with()` + subsequent mutation**: appears
   **once** total (`table.rs:555`, used to track row-group min/max
   bounds). Insufficient for a method.
2. **Bulk filter-and-clone (`iter().filter().collect()`)**: none
   found.
3. **Workarounds for missing std methods**: none found. Every
   HashMap use is idiomatic — `.get()`, `.insert()`, `.contains_key()`,
   `.entry()`, `.collect()`.
4. **Parallel data structures for ordering**: none found. Nobody
   maintains a Vec alongside a HashMap to preserve insertion order.

### AK method candidates — all rejected

| AK method | Koala demand | Verdict |
|---|---|---|
| `find(hash, predicate)` | Zero | No |
| `ensure(key, init_fn)` | One use of `entry().or_insert_with()` | No |
| `take(key)` | Zero (Rust's `.remove()` already returns `Option<V>`) | No |
| `set_from(pairs)` | Zero (`.collect()` is already idiomatic) | No |
| `remove_all_matching(pred)` | Zero (`.retain(!pred)` covers it cosmetically) | No |

**Conclusion**: v1 ships std-parity exactly. No AK additions. If a
concrete Koala use case later surfaces one of these, it gets added as
a targeted change.

### Type-choice finding — not a HashMap issue

The one genuine problem the scan surfaced is orthogonal to HashMap
method design: **`HashMap<String, String>` is the wrong type for DOM
attribute maps.** Most real-world HTML elements have 0–3 attributes;
the hash table's metadata overhead (~30 bytes per bucket on top of the
key/value) plus string hashing cost dominate the actual data. A
`Vec<(String, String)>` or (eventually) `ThinVec<(FlyString,
FlyString)>` would be more cache-friendly for typical DOM usage.

**This is a type-choice issue in koala-dom, not a HashMap issue in
koala-std.** Fix it when the browser-string family (milestone 2) and
`ThinVec` (milestone 3) land, not now. Noted here so future sessions
see the motivation for those milestones includes a concrete current
pain point.

## Implementation checklist

Work items grouped by phase. Check off each box as it lands. Each
phase should end with a green test run and a commit before moving on
to the next.

### Phase 1 — Private allocation + hash function

- [ ] `koala_std::hash::FxHasher` — implement the single-multiply
      hash (~30 lines), expose as `pub struct FxHasher` with a `Hasher`
      trait impl. Include a `FxBuildHasher` for use as `BuildHasher`.
      Doc comment + meaningful doc-test showing two identical inputs
      hash identically.
- [ ] Unit tests for `FxHasher`: determinism, distribution sanity
      check (not cryptographic — just "avalanches reasonably for a
      handful of inputs"), a `write_u64` roundtrip, ZST case
      (`()` should hash to a stable fixed value).

### Phase 2 — `RawTable<K, V>` backing type

- [ ] `mod raw_table;` — private module, same shape as `raw_vec` in
      its relationship to the public type.
- [ ] `struct RawTable<K, V>` with fields: `buckets: NonNull<Bucket<K,
      V>>`, `capacity: usize`, `used: usize`, `_marker: PhantomData<(K,
      V)>`.
- [ ] `struct Bucket<K, V>`: `{ state: u8, hash_fragment: u32, entry:
      MaybeUninit<(K, V)> }`. Layout verified with a static assertion.
- [ ] `RawTable::new()` — dangling allocation, capacity 0.
- [ ] `RawTable::with_capacity(n)` — allocate enough buckets to satisfy
      `n` entries at 70% load factor: `max(8, n * 100 / 70 + 1).next_power_of_two()`.
- [ ] Allocation via `alloc::alloc::{alloc, dealloc, realloc, Layout}`,
      aborting on failure via `handle_alloc_error`. Overflow detection
      via `Layout::array::<Bucket<K, V>>(capacity)`.
- [ ] `RawTable::drop()` — walk every used bucket, call
      `drop_in_place` on the initialized `(K, V)` entries, then
      `dealloc` the backing.
- [ ] Unit tests: construction, capacity math, grow/shrink, ZST
      `RawTable<(), ()>`, drop correctness with a `DropRecorder`.

### Phase 3 — `HashMap<K, V>` public type, basic API

- [ ] `pub struct HashMap<K, V, H = FxHasher>` with fields `{ table:
      RawTable<K, V>, hasher_builder: BuildHasherDefault<H> }`.
- [ ] `fn new() -> Self` — const fn if possible.
- [ ] `fn with_capacity(capacity: usize) -> Self`.
- [ ] `fn len(&self) -> usize`, `fn is_empty(&self) -> bool`,
      `fn capacity(&self) -> usize`.
- [ ] `fn insert(&mut self, key: K, value: V) -> Option<V>` — Robin
      Hood probe, cached `u32` hash fragment filter before `K::eq`,
      displacement on probe-length-poorer-than-incoming, grow via
      `RawTable::reserve` when load factor exceeds 70%.
- [ ] `fn get<Q>(&self, key: &Q) -> Option<&V>` where `K: Borrow<Q>`.
- [ ] `fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>`.
- [ ] `fn contains_key<Q>(&self, key: &Q) -> bool` — delegates to
      `get`.
- [ ] `fn remove<Q>(&mut self, key: &Q) -> Option<V>` — backshift
      deletion per the pseudocode in decision #4.
- [ ] Doc comments for every public method per the conventions in
      `koala-std-vec-design.md` (summary → examples with meaningful
      doc-tests → `# Panics` where applicable → `# Time complexity`).

### Phase 4 — Grow and rehash

- [ ] `fn reserve(&mut self, additional: usize)`.
- [ ] `fn shrink_to_fit(&mut self)`.
- [ ] Internal `fn grow_to(new_capacity)`: allocate new backing,
      re-insert every entry (recomputing home positions), free old
      backing. Panic-safe via a drop guard on the new table in case a
      re-insert panics mid-loop.
- [ ] Tests: force multiple grows via repeated insert, verify contents
      after each grow, verify `capacity()` behaves monotonically
      except under `shrink_to_fit`.

### Phase 5 — Iterator / entry / trait API

- [ ] `struct Iter<'a, K, V>` — borrowed iteration yielding
      `(&'a K, &'a V)`. Walks the bucket array, skipping empty slots.
- [ ] `struct IterMut<'a, K, V>` — same, yielding `(&'a K, &'a mut V)`.
- [ ] `struct IntoIter<K, V>` — consuming iteration; owns the
      `RawTable` and drops remaining entries in its own `Drop`.
- [ ] `fn iter(&self)`, `fn iter_mut(&mut self)`, `fn into_iter(self)`
      accessors.
- [ ] `fn keys(&self)`, `fn values(&self)`, `fn values_mut(&mut self)`.
- [ ] `struct Entry<'a, K, V>` enum with `Occupied` / `Vacant` variants.
- [ ] `fn entry(&mut self, key: K) -> Entry<'_, K, V>` — probe once,
      capture the slot, return a typed entry that can `or_insert`,
      `or_insert_with`, `and_modify`, etc.
- [ ] Trait impls: `Default`, `Debug`, `Clone` (where `K: Clone, V:
      Clone`), `PartialEq`/`Eq`, `Extend<(K, V)>`, `FromIterator<(K,
      V)>`, `IntoIterator` for owned / `&` / `&mut`.

### Phase 6 — `HashSet<T>` wrapper

- [ ] `pub struct HashSet<T, H = FxHasher>` as a thin wrapper around
      `HashMap<T, ()>`.
- [ ] `insert(&mut self, value: T) -> bool`,
      `contains<Q>(&self, value: &Q) -> bool`,
      `remove<Q>(&mut self, value: &Q) -> bool`, etc.
- [ ] Same trait impls as `HashMap`.
- [ ] Set operations: `intersection`, `union`, `difference`,
      `symmetric_difference`, `is_subset`, `is_superset`.

### Phase 7 — Tests, miri, differential harness

- [ ] Unit tests inside `raw_table.rs` for private-impl coverage.
- [ ] `tests/hashmap.rs` integration test file with:
  - [ ] Differential `quickcheck` harness: random `Op` sequence
        (Insert, Remove, Get, ContainsKey, Len, Clear) applied to
        both `koala_std::HashMap<i32, i32>` and `std::collections::
        HashMap<i32, i32>` in lock-step, with snapshot comparison.
  - [ ] Explicit ZST tests: `HashMap<(), ()>`, `HashMap<(), i32>`,
        `HashMap<i32, ()>`.
  - [ ] Explicit drop-ordering tests with `DropRecorder` values.
  - [ ] Explicit tests for `HashMap<String, i32>` that exercise the
        cached-hash optimization path.
- [ ] `miri` clean: `cargo miri test -p koala-std` green (the
        existing CI workflow already runs this).
- [ ] `clippy` clean across all targets.

### Phase 8 — Documentation polish

- [ ] Module-level doc on `koala_std::collections` explaining the
      hash table design at a high level with a link back to this doc.
- [ ] Verify every public method has a meaningful doc-test that asserts
      against the contract (not just syntax).
- [ ] Verify every public method has a `# Time complexity` section in
      the proportional-prose style from `koala-std-vec-design.md`.
- [ ] Update `koala-std-roadmap.md` progress log when the work lands.

## Future state — v2 and beyond

Work that is **explicitly deferred** but worth capturing so future
sessions pick up with the full context.

### v2 optimization pass (same type, no API changes)

- **SoA refactor + SIMD probe-length scanning.** Separate the bucket
  state bytes into their own parallel array so we can SIMD-compare 16
  probe lengths at once during insert's "where to displace" search.
  Gated on profiling showing the current AoS design is the hot path.
  **Potentially novel** — most production Robin Hood designs went to
  SwissTable-style control bytes rather than keeping Robin Hood with
  SIMD metadata scanning.
- **Conditional cached hash via trait specialization.** AK's
  `may_have_slow_equality_check` pattern: a trait the user can opt
  into to disable the `u32` hash fragment for cheap-`eq` key types.
  Gated on profiling showing the 4-byte-per-bucket overhead is
  observable on small-key maps.
- **Full 64-bit cached hash as a feature flag.** For maps where key
  eq is very expensive (`HashMap<LargeStruct, V>`), upgrade from 32
  to 64 bits of cached hash to further reduce false-positive equality
  calls. Low priority.

### v2 parallel type — `ElasticHashMap<K, V>`

Implement Farach-Colton / Krapivin / Kuszmaul's elastic hashing as a
separate sibling type in `koala_std::collections`. **Not a replacement
for the Robin Hood HashMap** — a parallel offering, explicitly marked
as "research-grade, benchmarks pending."

- [ ] Read the [full paper (arXiv 2501.02305)](https://arxiv.org/abs/2501.02305)
      and the [Python reference implementation](https://github.com/sternma/optopenhash).
- [ ] Port the algorithm to Rust. Expected ~800 lines plus careful
      testing of the multi-floor structure.
- [ ] Differential tests against the baseline Robin Hood HashMap.
      (Not against std — same interface, so either works as an
      oracle, but Robin Hood is what we have locally.)
- [ ] Benchmarks comparing the two at a range of load factors (50%,
      70%, 85%, 95%, 99%). The theoretical win is at >95%; confirm
      it shows up empirically.
- [ ] Publish the benchmark results. This might be the first
      systematic comparison of elastic hashing vs Robin Hood in Rust
      at the time we ship it.

This is the "novel research angle" item you flagged as interesting.
It is a v2 item — we build the Robin Hood baseline first, ship it,
use it, *then* layer on the elastic sibling type once there is
something real to compare it against.

### Milestone 3 implications

The HashMap work in milestone 1 feeds directly into milestone 2 and
milestone 3:

- **Milestone 2 `FlyString`** needs a `HashMap` for its global intern
  table. Building HashMap first means `FlyString` doesn't have to
  temporarily use `std::HashMap` and migrate later.
- **Milestone 3 `ArenaVec`** is independent of HashMap but shares the
  `RawVec` allocation primitive pattern.

## References

### Primary sources

- [Optimal Bounds for Open Addressing Without Reordering (arXiv 2501.02305)](https://arxiv.org/abs/2501.02305)
- [Ladybird `AK/HashTable.h` source](https://github.com/LadybirdBrowser/ladybird/blob/master/AK/HashTable.h) — fetched 2026-04-13 via `gh api`
- [hashbrown repository](https://github.com/rust-lang/hashbrown)
- [Abseil SwissTable design](https://abseil.io/about/design/swisstables)
- [Facebook F14 engineering blog](https://engineering.fb.com/2019/04/25/developer-tools/f14/)
- [CPython dictobject.c implementation notes](https://www.laurentluce.com/posts/python-dictionary-implementation/)
- [optopenhash Python reference for elastic hashing](https://github.com/sternma/optopenhash)

### Accessible framing

- [Undergraduate Upends a 40-Year-Old Data Science Conjecture (Quanta Magazine)](https://www.quantamagazine.org/undergraduate-upends-a-40-year-old-data-science-conjecture-20250210/)
- [OPAW: Optimal Bounds for Open Addressing Without Reordering (technical blog)](https://blog.georgovassilis.com/2026/04/04/opaw-optimal-bounds-for-open-addressing-without-reordering/)

### Related koala-std docs

- [`koala-std-roadmap.md`](koala-std-roadmap.md) — overall crate
  roadmap, milestone structure, build-from-scratch philosophy
- [`koala-std-vec-design.md`](koala-std-vec-design.md) — historical
  `Vec<T>` design retrospective, doc-comment conventions, opportunity
  analysis for milestone-3 vector types, CoW and parallel-drop
  sidebars. The doc-comment convention section applies to HashMap too.
