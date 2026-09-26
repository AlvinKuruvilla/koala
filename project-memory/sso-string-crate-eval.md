# SSO string crate evaluation + FlyString redesign

Decision input for the FlyString → SSO pivot. The histogram on
`perf/alloc-histogram` (commit `a5451df`) showed **~65% of landing's
setup allocations / ~43% of google's are ≤24 bytes** — the inline range
of every SSO type below. That is the lever; tag-name interning was 0.3%.

This doc does two things the way the last session deferred them:
1. Compares the candidate SSO crates (facts verified June 2026).
2. **Evaluates Ladybird's `FlyString` design end-to-end** and lands a
   concrete redesign: a koala-owned `FlyString` *wrapper* over an SSO
   base, not direct use of an SSO crate.

## Part 1 — the crate comparison

| Crate | Latest | Inline | Total | Mutable | Clone (heap) | `no_std` | Maintenance |
|-------|--------|--------|-------|---------|--------------|----------|-------------|
| `compact_str` | 0.9.1 | 24 B | 24 B | yes | **O(n) deep copy** | yes | active |
| `smol_str` | rust-lang org | 23 B | 24 B | no | O(1) Arc | partial | moved to rust-lang; old repo archived Nov 2025 |
| **`ecow` `EcoString`** | 0.2.6 | **15 B** | **16 B** | yes (COW) | **O(1) atomic refcount** | **yes** | active (Typst) |
| ~~`smartstring`~~ | 1.0.1 (2022) | 23 B | 24 B | yes | O(n) | flaky | **ARCHIVED 2026-05-03 — out** |

Sizes are 64-bit LE. koala's strings are cloned far more than mutated
(the cascade copies `ComputedStyle` across the node tree), so **O(1)
clone + small footprint** beat mutability — which argues against
`compact_str`'s O(n) clone despite it being the best pure-`String`
drop-in.

**`ecow::EcoString` — validated facts (docs.rs 0.2.6 / Typst README):**
- 16 B total, **15 B inline** (64-bit LE); spills to a refcounted
  `EcoVec<u8>` at ≥16 bytes. Smallest footprint of the four.
- **O(1) clone** via *atomic* refcount; clone-on-write — spilled strings
  share the allocation until a mutation forces a copy. So it is
  **`Send + Sync`** (matters: per-tab load workers).
- `no_std` supported (crates.io tagged; clean dep tree).
- Trait surface: `Deref<Target=str>`, `Clone`, `Hash`, `Eq`, `Ord`,
  `Send`, `Sync`; `From<&str> / String / &String / char / Cow<str>`;
  mutation via `push`, `push_str`, `make_mut(&mut self) -> &mut str`.
- **Gap that matters for us:** no public inline-vs-heap discriminant and
  no `ptr_eq`. `as_ptr()` is only available via `Deref<str>`. So we can't
  replicate Ladybird's single-word equality directly — see Part 2.

## Part 2 — how Ladybird actually does FlyString (reference read)

Read against `tmp/ladybird-ref/AK/{FlyString,StringBase,StringData}.{h,cpp}`.
Three findings overturn the "two separate tiers" framing from last
session:

**(1) Interning is encapsulated in construction — there is no `intern()`
verb.** `FlyString::from_utf8(StringView)`, `FlyString(String const&)`,
`operator=(String const&)`, and the `""_fly_string` literal are the only
ways to make one. The caller never thinks about the table. This is the
ergonomic the user remembered, and it is correct.

**(2) Short strings are never interned — they are SSO-inline.**
`from_utf8`:
```
empty                              -> empty FlyString
len <= MAX_SHORT_STRING_BYTE_COUNT -> plain short String, inline, NOT in table
else (found in table)              -> share the canonical StringData*
else                               -> create + register in table
```
`MAX_SHORT_STRING_BYTE_COUNT = sizeof(ptr) - 1 = 7` bytes. This is
**exactly why koala's tag-name interning gave 0.3%**: koala interns short
strings into a heap `Arc<str>` *and* a table slot — strictly worse than
inlining them for free and skipping the table. The current
`FlyString = Arc<str>` is the wrong substrate.

**(3) `FlyString` and SSO are ONE unified type, not two bolted tiers.**
`FlyString` wraps `Detail::StringBase` — the *same* SSO storage that
backs `String`. `StringBase` is a `union { ShortString; StringData* }`
discriminated by the pointer's LSB (`SHORT_STRING_FLAG`). `FlyString`
adds only the global dedup table for the long case. Equality is a single
word: short → bitwise compare of the inline union; both-fly → pointer
compare; `may_have_slow_equality_check() == false`. Hash is the
*content* hash cached on `StringData` (so equal short and equal long
strings hash consistently). The table holds **non-owning** pointers and
`StringData::~StringData` removes itself (`did_destroy_fly_string_data`)
— self-GC, no leak. (Ours leaks: strong `Arc` in the `HashSet`.)

## The redesign this implies

**Wrap, don't use ecow directly.** ecow alone gives SSO + O(1) clone but
not (a) cross-construction-site dedup — two `EcoString::from("long-repeat")`
calls make two allocations — nor (b) the O(1)-equality / case-insensitive
HTML/CSS helpers the engine wants. The wrapper is also the **stable seam**
that lets us swap the base (ecow now → hand-rolled koala-std SSO later)
without touching engine call sites.

> **DECISION (2026-06-02):** rather than the koala-common/koala-os split
> below, `FlyString` was placed in **koala-std behind an opt-in `std`
> feature** (`default = []`). The collection core stays `no_std`; the
> `std` feature is the one sanctioned exception, hosting the global
> interner (which needs `std` `Mutex`/`LazyLock`). This also removed the
> reqwest-into-koala-dom problem, since koala-dom already depends on
> koala-std. The orphan-rule analysis below still holds — `FlyString` is
> local to the crate that defines it, so `impl From<&str>` is legal there.

**Layering — this is what unblocks `FlyString::from(...)`.** The orphan
rule blocked `impl From<&str> for FlyString` last session *only because*
`FlyString` lived in koala-std (no_std) while the global table lived in
koala-common (needs `std` `Mutex`/`LazyLock`). Split it Ladybird's way and
the block vanishes:

- **koala-std** owns the **SSO base** (the `StringBase` analog) — the
  milestone-2 string deliverable, hand-rolled eventually. `no_std`.
  No global table here.
- **koala-common** owns **`FlyString`** = `{ base }` + the global dedup
  table (`LazyLock<Mutex<...>>`). Because `FlyString` is now *local to
  koala-common*, `impl From<&str> for FlyString` is orphan-rule-legal,
  and `from()` encapsulates the short-inline-vs-long-intern decision.
  The free `intern()` verb goes away.

There is **no migration cost**: Phase 2 was reverted, so there are zero
production interning call sites today. Build the new wrapper fresh.

**Recommended path** (mirrors the `FxHasher` vs `rustc-hash` oracle and
the [[feedback-vendor-subcrate-over-submodule]] rule):
1. Adopt **`ecow::EcoString`** as the base now; build the `FlyString`
   wrapper over it in koala-common to bank the ≤24 B win immediately.
2. Use ecow as the differential oracle when koala-std hand-rolls its own
   SSO base (milestone 2), then swap it behind the unchanged wrapper.

## What the wrapper should include (from FlyString.h)

- `From<&str>` / `From<String>` / `Default` (empty) — interning
  encapsulated; short → inline base, long → table dedup.
- `as_str` + `Deref<Target=str>`; `len`, `is_empty`.
- **`Hash` by content, `Eq` by fast path.** *Correctness note:* the
  current koala-std `FlyString` hashes by **pointer**, which is unsound
  once short strings are inline (two equal inline strings have different
  addresses but must hash equal). The wrapper must hash by content
  (cache it on the heap node, like `StringData`). Fast `Eq`:
  `self.as_ptr() == other.as_ptr() && self.len() == other.len()` →
  `true` (covers canonical interned long strings, O(1)); else byte
  compare (short strings, ≤15 B, cheap). This recovers Ladybird's
  single-word intent using only ecow's public API.
- **`equals_ignoring_ascii_case`, `to_ascii_lowercase/uppercase`** —
  genuinely useful: HTML tag/attribute names and many CSS idents match
  ASCII-case-insensitively.
- **`is_one_of(...)`** — ergonomic tag/keyword dispatch.
- `number_of_fly_strings()` (diagnostics/tests); self-GC on drop so the
  table doesn't leak transient strings.

## Open questions before building

- **Measure, don't assume.** Re-run bench-diff after adopting in
  koala-dom alone; the histogram is size-keyed (upper bound) and google's
  share is partly Boa internals SSO can't touch. Also worth knowing the
  ≤7 / ≤15 / ≤24 byte split, since ecow's 15 B inline misses e.g.
  `"background-color"` (16 B).
- **Does the SSO base type leak into koala-css/html signatures?** Yes
  wherever DOM stores names — acceptable behind the wrapper; the eventual
  koala-std base swaps in at the same seam.
- **Single-interner invariant** still holds: equal long strings must be
  the *same* ecow allocation for the `as_ptr` fast path, which the table
  guarantees by handing back the canonical instance.

## Sources

- compact_str: <https://docs.rs/compact_str/latest/compact_str/>
- smol_str: <https://github.com/rust-analyzer/smol_str>
- ecow: <https://docs.rs/ecow/latest/ecow/string/struct.EcoString.html>,
  <https://github.com/typst/ecow>
- smartstring (archived): <https://github.com/bodil/smartstring>
- Ladybird reference: `tmp/ladybird-ref/AK/{FlyString,StringBase,StringData}.{h,cpp}`
