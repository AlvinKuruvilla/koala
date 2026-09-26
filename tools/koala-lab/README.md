# koala-lab

Measures koala builds against each other on a fixed set of real pages and
reports what changed. Run it through `just lab` from anywhere in the
repository.

```sh
just lab corpus record           # once per machine: record the pages
just lab compare                 # working tree vs where this branch left master
just lab compare master my-branch
just lab show                    # re-print the latest result
```

## Reading the result

```text
a  where this branch left master (4a51047)
b  working tree (4a51047, with uncommitted changes)
7 rounds, 4 pages; raw reports in .koala-lab/runs/...

hacker-news  load    slower                 6.03 ms -> 8.24 ms  +36.5%  (p=0.0006)
                 css_cascade          3.01 ms -> 5.10 ms
             render  no detectable change   4.58 ms -> 4.45 ms  -2.8%
```

- **faster** / **slower**: a real change. Across a whole run, the chance
  of reporting any change that is not real is at most 5%.
- **no detectable change**: the builds could not be told apart; the
  percentage says how close they were anyway.
- Indented lines under a change: the stages that moved most. Shown for
  orientation, not tested.
- Lines starting `!` come first and mean the timings compare different
  work: the builds fetched different resources or drew different frames,
  or too few rounds ran to show any change.

## How it works

- **Corpus**: `corpus.toml` lists the pages. Their content is never
  committed; `corpus record` loads each page once through
  `koala --record` and keeps every response in
  `.koala-lab/archives/`. Measurements replay those archives
  (`koala --replay`), so every run sees identical input and none touches
  the network.
- **Builds**: each revision is built in a worktree under `.koala-lab/`
  and the binary is cached by commit; `.` builds the working tree as it
  is.
- **Rounds**: each round runs one fresh `koala --bench` process per page
  per build, alternating which build goes first. Per page, one timed load
  sizes how many loads each process measures.
- **Verdict**: each process contributes its p50. Load and render time per
  page are compared with an exact Mann-Whitney U test, Holm-corrected
  across the run.

The design and the reasoning behind it are in
`project-memory/bench-system-design.md`.

## Limits

- Commits from before PR #11 cannot be measured: they do not write the
  report koala-lab reads.
- Timings compare only on the same machine.
- Re-recording a page changes its input; runs before and after are not
  comparable.
- A change of 2 ms on a 350-850 ms page is below what 7 rounds can
  resolve.

## Development

```sh
just py-check    # ruff, ruff format --check, mypy --strict, pytest
```
