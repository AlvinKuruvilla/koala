---
created: 2026-09-26
area: koala-css (selector matching, cascade) + koala-dom
status: step 0 done (no-allocation class check); steps 1-3 planned
---

# Selector matching: plan

The cascade tests every rule against every element (`compute_node_styles`
in `crates/koala-css/src/cascade/mod.rs`). A `sample` profile of repeated
wikipedia loads (2026-09-26, 7,217 samples) put about half of all samples
in `ElementData::classes`, which built a `HashSet` of the element's
classes on every class-selector test. Style copying (custom properties)
was about 2%.

## Step 0 (done): check class membership without allocating

`ElementData::has_class` scans the attribute in place. Measured with
`just lab compare` against feebb01 (7 rounds): wikipedia load 350 -> 183 ms
(-47.8%), allocations per load 3.65 M -> 0.69 M; hacker news -19.4%;
google -3.9%; frames and inputs identical. Also fixed matching to split on
ASCII whitespace, not only spaces.

## What real engines do

Read from source (servo/stylo `bb8a1c1`, chromium `08da355`):

- **Class list parsed once** on attribute change: Blink
  `Element::ClassAttributeChanged` into `SpaceSplitString`
  (a `Vector<AtomicString, 4>`, shared between elements with the same
  string); Stylo `AttrValue::TokenList` of atoms.
- **Rules bucketed** by the rightmost compound's most selective simple
  selector. Stylo `SelectorMap` (`style/selector_map.rs`): root > id >
  class > attribute > local name > rare pseudo-classes > namespace >
  universal. Blink `RuleSet::FindBestBucketAndAdd` (`css/rule_set.cc`):
  similar, id > class > attribute > tag > universal after special
  pseudo buckets. An element tests only the buckets for its id, each
  class, its tag, and universal.
- **Cascade order** restored by a stored source position: Stylo sorts by
  `(layer, specificity, scope proximity, source_order)`; Blink's
  `MatchedRule` sort key is layer, specificity, proximity, then position.
- **Ancestor Bloom filter** rejects combinator selectors early: Stylo
  `StyleBloom` (`style/bloom.rs`), Blink `SelectorFilter`
  (`css/selector_filter.h`, whose header reports discarding 60-70% of
  rules on real pages as of 2022).

## Steps, each measured with `just lab compare`

1. **Parse classes once.** `ElementData` keeps `classes: Vec<FlyString>`,
   rebuilt when `class` is set, changed, or removed. `attrs` stops being a
   public field; writes go through `set_attribute` / `remove_attribute`
   (four koala-js sites plus the parser and test fixtures).
2. **Rule index.** Buckets for id, class, tag, universal keyed on the
   subject compound; each `ParsedRule` stores its source position; matched
   rules sort by `(origin, specificity, position)`. Today's code relies on
   a stable sort over source order, which stops holding once candidates
   come from several buckets.
3. **Ancestor Bloom filter**, if the profile after 2 still shows
   combinator matching.

Deferred: Blink's shared class-list cache (saves memory, not matching
time); attribute and root buckets (add when attribute selectors show up in
a profile).

Separately: `post_js_relayout` re-runs the whole cascade after scripts
mutate the DOM (wikipedia pays the cascade twice). Restyling only what
changed is its own project.
