"""Measuring two builds of koala against each other on the corpus.

Each round runs one measurement process per page per build, and the order
of the two builds alternates between rounds, so a machine that drifts
(thermals, background load) over the run pushes both sides equally. Every
process's report is kept, with the run's provenance, under
`<state>/runs/<time>_<a>_<b>/`, so a result can be re-analysed without
re-measuring.
"""

from __future__ import annotations

import json
import os
import platform
from collections.abc import Callable
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from koala_lab import corpus, probe
from koala_lab.build import Build
from koala_lab.corpus import Page
from koala_lab.errors import LabError
from koala_lab.probe import Counts, Report
from koala_lab.repo import state_dir

# How long each process spends per page. Sized from one timed load so a
# 1 ms page and a 1 s page both get enough samples without a slow page
# taking minutes. Warm-up exists to ramp the CPU clock and fill caches
# before measuring; measurement gets three times as long.
WARMUP_SECONDS = 1.0
MEASURE_SECONDS = 3.0
MIN_WARMUP_LOADS = 2
MIN_LOADS = 5
# Caps the fast pages; beyond this the per-process p50 no longer moves.
MAX_LOADS = 300
RENDERS = 10


@dataclass(frozen=True)
class Run:
    """Every report from one comparison, grouped by page and build."""

    a: Build
    b: Build
    pages: list[Page]
    counts: dict[str, Counts]
    # page name -> (reports from `a`, reports from `b`), one per round.
    reports: dict[str, tuple[list[Report], list[Report]]]
    directory: Path


def counts_for(first_load_us: int) -> Counts:
    """Loads and renders per process for a page whose first load took
    `first_load_us`."""
    seconds = max(first_load_us, 1) / 1_000_000
    return Counts(
        warmup_loads=min(
            MAX_LOADS, max(MIN_WARMUP_LOADS, round(WARMUP_SECONDS / seconds))
        ),
        loads=min(MAX_LOADS, max(MIN_LOADS, round(MEASURE_SECONDS / seconds))),
        renders=RENDERS,
    )


def steps(pages: list[Page], rounds: int) -> int:
    """How many measurement processes `run` starts: one sizing process per
    page, then one per page per build per round."""
    return len(pages) * (1 + 2 * rounds)


def run(
    root: Path,
    a: Build,
    b: Build,
    pages: list[Page],
    rounds: int,
    on_step: Callable[[str], None] = lambda _: None,
) -> Run:
    """Measure `pages` on both builds for `rounds` rounds, calling
    `on_step` with a description before each of the `steps` processes."""
    archives = {page.name: corpus.archive_path(root, page) for page in pages}
    missing = [name for name, path in archives.items() if not path.exists()]
    if missing:
        raise LabError(
            f"not recorded: {', '.join(missing)} (run: koala-lab corpus record)"
        )

    # One timed load on `a` sizes each page. The first load of a process
    # is its slowest, so this errs toward fewer samples, never too many.
    counts: dict[str, Counts] = {}
    for page in pages:
        on_step(f"sizing {page.name}")
        first = probe.run(a.binary, archives[page.name], page.url, Counts(0, 1, 1))
        counts[page.name] = counts_for(first.load_total_us[0])

    stamp = datetime.now(UTC).strftime("%Y-%m-%dT%H-%M-%SZ")
    directory = state_dir(root) / "runs" / f"{stamp}_{a.sha[:7]}_{b.sha[:7]}"
    directory.mkdir(parents=True)
    _write_meta(directory, a, b, pages, counts, rounds)

    reports: dict[str, tuple[list[Report], list[Report]]] = {
        page.name: ([], []) for page in pages
    }
    for round_index in range(rounds):
        order = [(0, a), (1, b)] if round_index % 2 == 0 else [(1, b), (0, a)]
        for page in pages:
            for side, build in order:
                on_step(f"round {round_index + 1}/{rounds}  {page.name}  {'ab'[side]}")
                report = probe.run(
                    build.binary, archives[page.name], page.url, counts[page.name]
                )
                reports[page.name][side].append(report)
                out = directory / page.name / f"{'ab'[side]}-{round_index}.json"
                out.parent.mkdir(exist_ok=True)
                out.write_text(json.dumps(report.raw))

    return Run(
        a=a, b=b, pages=pages, counts=counts, reports=reports, directory=directory
    )


def _write_meta(
    directory: Path,
    a: Build,
    b: Build,
    pages: list[Page],
    counts: dict[str, Counts],
    rounds: int,
) -> None:
    """Record what was run, on what, so a stored run is interpretable
    without this session."""

    def build(side: Build) -> dict[str, object]:
        return {"rev": side.rev, "sha": side.sha, "dirty": side.dirty}

    meta = {
        "started": datetime.now(UTC).isoformat(),
        "a": build(a),
        "b": build(b),
        "rounds": rounds,
        "viewport": list(probe.VIEWPORT),
        "pages": [
            {
                "name": page.name,
                "url": page.url,
                "warmup_loads": counts[page.name].warmup_loads,
                "loads": counts[page.name].loads,
                "renders": counts[page.name].renders,
            }
            for page in pages
        ],
        "host": {
            "platform": platform.platform(),
            "machine": platform.machine(),
            "cpus": os.cpu_count(),
            "load_average": os.getloadavg(),
        },
    }
    (directory / "meta.json").write_text(json.dumps(meta, indent=2))


def load(directory: Path) -> Run:
    """Read back a stored run, for re-analysis without re-measuring. The
    builds' binaries are not needed and not checked."""
    try:
        meta = json.loads((directory / "meta.json").read_text())
    except (OSError, json.JSONDecodeError) as err:
        raise LabError(f"{directory} is not a koala-lab run: {err}") from err

    def build(side: dict[str, Any]) -> Build:
        return Build(
            rev=side["rev"], sha=side["sha"], dirty=side["dirty"], binary=Path()
        )

    pages = [Page(p["name"], p["url"], exercises="") for p in meta["pages"]]
    counts = {
        p["name"]: Counts(p["warmup_loads"], p["loads"], p["renders"])
        for p in meta["pages"]
    }
    reports: dict[str, tuple[list[Report], list[Report]]] = {}
    for page in pages:
        sides: tuple[list[Report], list[Report]] = ([], [])
        for round_index in range(meta["rounds"]):
            for side, letter in enumerate("ab"):
                path = directory / page.name / f"{letter}-{round_index}.json"
                try:
                    raw = json.loads(path.read_text())
                except OSError as err:
                    raise LabError(
                        f"{directory} is incomplete (the run was interrupted?): {err}"
                    ) from err
                sides[side].append(probe.parse(raw))
        reports[page.name] = sides
    return Run(
        a=build(meta["a"]),
        b=build(meta["b"]),
        pages=pages,
        counts=counts,
        reports=reports,
        directory=directory,
    )


def latest(root: Path) -> Path:
    """The most recent stored run."""
    runs = sorted((state_dir(root) / "runs").glob("*/meta.json"))
    if not runs:
        raise LabError("no stored runs yet (run: koala-lab compare A B)")
    return runs[-1].parent
