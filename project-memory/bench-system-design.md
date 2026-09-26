---
created: 2026-09-26
area: koala-cli (probe) + new Python orchestrator + dashboard
status: design — not started
last_updated: 2026-09-26
---

# Performance measurement system

A system for answering "did this change make koala faster, slower, or
allocate differently?" with a verdict you can act on, and for keeping
that answer over time. It replaces the collection of one-offs that grew
up around `koala --bench`: `--bench-diff` inside the binary, reports
named `after.json` / `final-a.json` in `tmp/`, and a throwaway shell
script that built three commits and interleaved their runs.

## Why the one-offs failed

Measured on the FlyString work (PR #9), 2026-09-26:

- **The comparer was built from one side of the comparison.**
  `--bench-diff` lives in the `koala` binary, so it can only read reports
  whose schema it knows. Master's reports lacked `setup_size_histogram`,
  so master could not be the baseline; `a5451df` stood in for it.
- **Reports carried no provenance.** 25 JSON files in `tmp/`, none
  recording commit, build flags, host, or input. Which one was the
  baseline lived in someone's head.
- **Single-report comparison was noise.** One base-vs-cascade pair
  showed `js_runtime_init` 6.3% faster; nothing on the branch touches
  the JS runtime. That stage is heavy-tailed (one report: p50 139 us,
  p95 819 us) and `--bench-diff` compared means. Five interleaved
  processes per side, compared on p50, gave ranges tight enough to
  separate an 11% cascade win and to show the tag-name change as nil.
- **Inputs were not pinned.** `koala-ui/res/landing.html` is the UI's
  own page and changes when the UI does. The google snapshot is one
  gitignored file fetched in May.
- **Correctness was checked by hand.** Byte-comparing screenshots
  across builds was a step in the throwaway script.

## Design principle: the output is the product

Using the system should take no thought; reading it should take little.
Concretely:

- A handful of verbs, each with defaults that do the statistically
  right thing. The user names revisions; the tool picks rounds,
  iterations, warmup, corpus, and ordering.
- Every report leads with a verdict in words, then the evidence.
  Changes the data cannot distinguish from noise are summarized in
  one line, not printed as a wall of dimmed numbers to scan.
- When the tool cannot give a trustworthy answer it says why and what
  to do (too few rounds, input changed, render differs), instead of
  printing numbers anyway.

Target output shape:

```text
$ just lab compare master HEAD
master (54e9aad) vs HEAD (b309466, clean) · corpus v3 · 5 rounds · M3 Pro, idle

landing        faster   css_cascade  477 → 426 µs  −11%   (separated, p≈0.008)
               faster   total load   957 → 890 µs   −7%   (separated)
               fewer    setup allocs 17,472 → 15,252  −12.7%  (exact)
google         no detectable change (13 stages); allocs −83 (exact)
wikipedia      no detectable change

render: identical on all 3 pages
```

## Architecture

The part that must run inside the revision under test is separated from
everything that should not:

```text
  built from each revision under test          one copy, from HEAD
 ┌──────────────────────────────┐        ┌───────────────────────────────────────┐
 │ probe: `koala --bench`        │  JSON  │ orchestrator (Python)                  │
 │  replay inputs from archive   │───────▶│  build revs → binary cache keyed by sha│
 │  load/render N times          │        │  record / refresh the corpus           │
 │  raw per-sample timings,      │        │  interleave rounds across revs × pages │
 │  alloc counts, render hash,   │        │  attach provenance (sha, dirty, flags, │
 │  input hash, schema_version   │        │   host, load, timestamp)               │
 └──────────────────────────────┘        │  store run set → analyze → verdict     │
        ▲  small, versioned contract      └───────────────────────────────────────┘
        │                                              │
  corpus: manifest (committed)                         ▼
        + archives (local, gitignored)       run store → compare / history / CI gate
```

- **Probe** — produces measurements and never analyzes them. Only
  things that exist inside the process go here. Raw samples, not
  summaries, so a better statistic later applies to old runs. Its
  contract must stay stable, because every revision being compared has
  to speak it.
- **Orchestrator** — owns everything else. It knows the sha it built,
  the flags it passed, and the host it ran on, so provenance is recorded
  here and the probe is not asked for it.

### Corpus: manifest committed, content recorded locally

Third-party page content is not committed. Two kinds of input:

- **Authored pages** (committed, `bench/corpus/authored/`): our own HTML
  that stresses one subsystem — many custom properties, deep nesting,
  large tables, a frozen copy of the landing page. Ours, small, stable.
- **Recorded pages** (manifest committed, content fetched): the
  manifest lists name, URL, and what the page exercises. `lab corpus
  record` fetches each page and every subresource koala requests, into
  a content-addressed archive under a gitignored directory. The probe
  replays from the archive; the network is never touched during a
  measurement.

Record/replay uses an existing seam. `koala_common::net` routes every
fetch through the `RequestSender` trait (`install_sender`), which
`MappedSender` already uses for the `oom-probe --map` override. Two new
senders:

- `RecordingSender` wraps `DefaultSender` and writes each
  URL → bytes it sees into the archive.
- `ReplaySender` serves from the archive and returns an error on a URL
  it has not recorded. A miss is a result ("HEAD requests a resource
  master did not"), reported by the orchestrator, never a silent
  fallback to the network.

Recording through koala's own fetch path means the archive holds exactly
what koala asks for, not what another browser would.

Consequences:

- An A/B comparison in one session uses one archive for both sides, so
  inputs are identical by construction.
- History across time is split by archive hash. Re-recording a page
  that changed starts a new series rather than silently joining two
  different inputs.
- Archives are per machine. So is timing data, so this costs nothing
  that was not already lost.

### Statistics

- Per process, per stage: p50 of the raw samples. Means are dominated
  by tails (`js_runtime_init`).
- Across processes: the median of the per-process p50s, plus its range.
- Verdict "separated" when every process on one side beat every process
  on the other: a Mann–Whitney U of 0. With 5 per side that is
  p = 2 / C(10,5) ≈ 0.008; with 3 per side only 0.1, so the default is
  5 and fewer than 4 draws a warning.
- Allocation counts and render hashes are deterministic on a fixed
  input. They are compared exactly; a disagreement within one side
  means the runs were not comparable and is an error.

The known weakness: range-separation is the least robust spread; one
bad round can mask a real effect. Raw samples are kept so a bootstrap
or a proper U test can replace it without re-running anything.

## Decisions

1. **Orchestrator in Python.** Most of its work is `git worktree`,
   `cargo build`, and running processes, and the dashboard's loaders
   are already Python. Rejected: a Rust xtask sharing the report types
   with the probe — the shared types would guard against schema drift,
   but a `schema_version` check at the boundary does the same job.
2. **No third-party content in the repo.** Recorded via koala's own
   fetch path and replayed from a local archive (above). Rejected:
   committing snapshots (licensing, repo growth, staleness); a live
   network during measurement (network variance swamps the effects we
   look for).
3. **Revisions older than the probe contract are not supported.** Master
   becomes the earliest valid baseline once phase 1 merges. Old
   numbers worth keeping are re-measured, not converted.

## Phases

1. **Probe contract.** `schema_version`, raw per-stage samples, render
   hash, input hash, `--replay <archive>`. The existing summary fields
   can go once nothing reads them. Done when: a report from HEAD
   carries all of these and a replayed run makes no network request.
2. **Orchestrator core.** `lab corpus record`, `lab compare <rev>
   <rev>` with binary cache, interleaving, provenance, verdict output.
   Retires `--bench-diff` in the binary and `tmp/bench-flystring.sh`;
   `just bench-diff` goes away or becomes an alias. Done when: the PR #9
   comparison reproduces with one command and matches the hand result.
3. **Authored corpus.** Frozen landing page plus pages aimed at the
   cascade, layout, and inline paths.
4. **Run store and history.** Runs kept as `<timestamp>_<sha>`, as
   `just wpt-record` does; a dashboard page for trends per page and
   stage.
5. **CI gate.** Base and PR head built and compared in the same job,
   against the same archive, on deterministic metrics only (allocation
   counts, render hash). No stored baseline needed, so shared-runner
   noise does not matter. Timings stay local.

## Later: fuzzing and other workloads

Fuzzing fits on the same foundation rather than beside it: it needs a
build of a given revision, seed inputs, and somewhere to keep results.
The recorded corpus is a seed corpus; the render hash enables
differential runs (same input, two revisions, different pixels). So the
binary cache, corpus, and run store should not be bench-specific in
naming or layout. No workload abstraction gets built until a second
workload exists.

## Open questions

- **Where the Python lives.** Existing homes are `wptrunner_koala`
  (WPT-specific) and `dashboard/src/data/` (loaders). This is neither,
  so a new package (`tools/koala-lab/`, from the Copier template:
  uv, ruff, mypy strict). Against: one more package to set up, where
  the standing preference is to extend an existing one.
- **Name.** "lab" above is a placeholder chosen to not say "bench".

## Fetch-path audit (2026-09-26)

Replay is only sound if every byte a load reads comes through
`RequestSender`. Checked by grepping the load pipeline at `b309466`:

- **Four fetch sites, all through the sender.** The document
  (`koala-browser/src/lib.rs`, `load_document`), external stylesheets
  (`koala-css/src/lib.rs`), images (`koala-browser/src/image_loader.rs`),
  and external scripts (`koala-browser/src/lib.rs`,
  `fetch_script_source`). Each calls `net::fetch_text` / `fetch_bytes`,
  which dispatch to the active sender.
- **No other network access.** No `reqwest` use outside
  `koala-common/src/{net,hosts}.rs`. Nothing fetches for CSS `@import`,
  `@font-face`, or CSS `url()` images, because none are implemented.
  Nothing fetches from JS: no `fetch` / `XMLHttpRequest` globals, and
  koala-js depends on `boa_engine` only, not `boa_runtime` (which has
  the `reqwest-blocking` fetcher).
- **No threads in the load path.** No `thread::spawn`, scoped threads,
  or rayon in koala-browser, koala-css, koala-js, koala-common, or
  koala-cli. The sender is thread-local, so this matters: a loader
  moved onto a worker would silently get `DefaultSender`. (koala-ui's
  per-tab loader threads are the exception; the probe does not use them.)
- **Filesystem reads outside the sender:** system fonts in
  `renderer.rs` (`load_font_from_paths`) and the WPT hosts file. Fonts
  are a machine input, not a page input: the render hash is comparable
  on one machine, including a CI job comparing base and head, but not
  across machines.

Consequences for the design:

- **Replay must load the page by its original URL.** A saved copy of a
  page gets a `file:` base URL, so its references resolve against the
  local directory, not the site. The probe runs `koala --bench
  https://www.google.com/ --replay <archive>`, and relative subresources
  resolve exactly as they did when recorded.
- **The existing `.bench-cache/` numbers measure a stripped page.**
  `just bench https://google.com` loads `.bench-cache/google_com.html`
  as a file, so its root-relative stylesheet and images resolve to
  `file:///xjs/_/ss/...` and `file:///images/...` and fail, as they
  would in a browser; only the HTML and inline scripts are measured.
  The google figures in PR #9 are for that page, not for google as a
  browser sees it.
- **The audit is a grep and will go stale.** `@import`, `@font-face`,
  JS `fetch`, or a loader thread each reopen it. Enforce the first half
  mechanically: a workspace `clippy.toml` `disallowed-types` entry for
  `reqwest::blocking::Client`, allowed only in `koala-common::net` and
  `hosts`. The threading half stays a note at `ACTIVE_SENDER`.
