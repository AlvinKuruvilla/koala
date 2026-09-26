"""One measurement process: `koala --bench` replaying a recorded page.

Each call starts a fresh process, so its samples share one warm-up, one
CPU-frequency state, and one allocator history. Differences between
processes are the noise a comparison has to see past, which is why
`compare` runs several per build instead of one long one.
"""

from __future__ import annotations

import json
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from koala_lab.errors import LabError

# The `BenchReport` layout this module reads (`SCHEMA_VERSION` in
# koala-cli/src/bench.rs).
SCHEMA_VERSION = 1

# Matches `just bench`: the koala-ui default window at 2x.
VIEWPORT = (2048, 1536)


@dataclass(frozen=True)
class Counts:
    """How much one process measures."""

    warmup_loads: int
    loads: int
    renders: int


@dataclass(frozen=True)
class Report:
    """What one process measured, plus its raw JSON for the run store."""

    # Per measured load: whole-load time, and time per pipeline stage.
    load_total_us: list[int]
    load_stages_us: list[dict[str, int]]
    # Per measured render: time per render stage.
    render_stages_us: list[dict[str, int]]
    # Heap activity of one load; deterministic for a fixed input.
    alloc_calls: int
    alloc_bytes: int
    render_hash: str
    input_digest: str
    raw: dict[str, Any]


def run(binary: Path, archive: Path, url: str, counts: Counts) -> Report:
    """Measure `url`, served from `archive`, in one `binary` process."""
    width, height = VIEWPORT
    result = subprocess.run(
        [
            str(binary),
            "--replay",
            str(archive),
            "--bench",
            url,
            "--width",
            str(width),
            "--height",
            str(height),
            "--setup-warmup",
            str(counts.warmup_loads),
            "--setup-iterations",
            str(counts.loads),
            "--bench-warmup",
            "1",
            "--bench-iterations",
            str(counts.renders),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        detail = result.stderr.strip().splitlines()[-1:] or ["no output"]
        raise LabError(f"{binary} failed measuring {url}: {detail[0]}")
    return parse(json.loads(result.stdout))


def parse(raw: dict[str, Any]) -> Report:
    """Read a `--bench` report, refusing layouts this module does not know."""
    version = raw.get("schema_version")
    if version != SCHEMA_VERSION:
        raise LabError(
            f"the build wrote report schema {version}, koala-lab reads "
            f"{SCHEMA_VERSION}; builds from before the probe contract "
            "(PR #11) cannot be measured"
        )
    digest = raw.get("input_digest")
    if digest is None:
        raise LabError("the report has no input digest; the page was not replayed")
    return Report(
        load_total_us=[sample["total_us"] for sample in raw["setup_samples"]],
        load_stages_us=[sample["stages_us"] for sample in raw["setup_samples"]],
        render_stages_us=raw["render_samples"],
        alloc_calls=raw["setup_alloc"]["alloc_calls"],
        alloc_bytes=raw["setup_alloc"]["bytes_allocated"],
        render_hash=raw["render_hash"],
        input_digest=digest,
        raw=raw,
    )
