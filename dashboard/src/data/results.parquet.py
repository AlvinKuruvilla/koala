#!/usr/bin/env python3
"""Emit per-test WPT results as a single Parquet file.

Observable Framework runs this script at build time, captures stdout
as bytes, and serves the result at ``data/results.parquet``. Pages
load it via ``FileAttachment("data/results.parquet")`` and query it
client-side with DuckDB-WASM.

We use Parquet (rather than JSON) because dictionary encoding on the
low-cardinality columns (``status``, ``area``, ``run_id``, ``revision``)
compresses 240k+ rows down to a few MB and DuckDB-WASM scans it
without blocking the UI. JSON would re-parse and stay resident in JS
memory.

Schema:

    run_id        VARCHAR  -- filename stem, e.g. 2026-05-26T16-46-44_acf0045
    run_ts        TIMESTAMP
    revision      VARCHAR  -- full koala git revision
    test          VARCHAR
    area          VARCHAR  -- two-level subarea, e.g. "css/CSS2"
    status        VARCHAR  -- PASS / FAIL / CRASH / TIMEOUT / ERROR / SKIP / ...
    duration_ms  INTEGER
    message       VARCHAR  -- nullable

Requires the `duckdb` Python package (>=1.0). Installed via
``just wpt-setup`` (extended to also pull dashboard deps).
"""

from __future__ import annotations

import json
import os
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path

import duckdb


def runs_dir() -> Path:
    """The dashboard/runs/ directory, sibling of dashboard/src/."""
    return Path(__file__).resolve().parents[2] / "runs"


def parse_timestamp(stem: str, fallback_ms: int | None) -> str | None:
    """Pull the ISO timestamp out of a run filename of the form
    ``2026-05-26T16-18-40_<sha>.json``. Falls back to the report's
    own ``time_start`` if the filename doesn't match."""
    name, _, _ = stem.partition("_")
    try:
        date_part, time_part = name.split("T", 1)
        return f"{date_part}T{time_part.replace('-', ':')}Z"
    except ValueError:
        if fallback_ms is None:
            return None
        return (
            datetime.fromtimestamp(fallback_ms / 1000, tz=timezone.utc)
            .isoformat()
            .replace("+00:00", "Z")
        )


def area_of(test_path: str) -> str:
    """Mirror of the bucketing in ``runs.json.py`` — see that file
    for the rationale on the two-level scheme."""
    if not test_path.startswith("/"):
        return "other"
    parts = [p for p in test_path[1:].split("/") if p]
    if not parts:
        return "other"
    dirs = parts[:-1]
    if not dirs:
        return parts[0]
    return "/".join(dirs[:2])


def flatten_runs() -> list[dict[str, object]]:
    """Walk dashboard/runs/*.json and produce one row per test result
    across all runs."""
    rows: list[dict[str, object]] = []
    base = runs_dir()
    if not base.exists():
        return rows

    for path in sorted(base.glob("*.json")):
        try:
            with path.open() as fh:
                report = json.load(fh)
        except (OSError, json.JSONDecodeError) as exc:
            print(f"# skipping {path.name}: {exc}", file=sys.stderr)
            continue

        run_id = path.stem
        run_ts = parse_timestamp(run_id, report.get("time_start"))
        revision = report.get("run_info", {}).get("revision")

        for r in report.get("results", []):
            test = r.get("test", "")
            rows.append({
                "run_id": run_id,
                "run_ts": run_ts,
                "revision": revision,
                "test": test,
                "area": area_of(test),
                "status": r.get("status", "UNKNOWN"),
                "duration_ms": r.get("duration"),
                "message": r.get("message"),
            })

    return rows


def write_parquet(rows: list[dict[str, object]]) -> bytes:
    """Use DuckDB to write a Parquet file from the row list, then
    return its bytes. We go via a JSONL temp file because DuckDB's
    Python bindings don't have a direct list-of-dicts → Parquet path
    that survives without pandas/pyarrow."""
    with tempfile.TemporaryDirectory(prefix="koala-parquet-") as tmpdir:
        jsonl_path = os.path.join(tmpdir, "results.jsonl")
        parquet_path = os.path.join(tmpdir, "results.parquet")

        with open(jsonl_path, "w", encoding="utf-8") as fh:
            for row in rows:
                fh.write(json.dumps(row, default=str) + "\n")

        con = duckdb.connect(":memory:")
        # `read_json_auto` with the newline_delimited format
        # auto-infers the schema, including nullable columns like
        # `message` and `duration_ms`. ZSTD picks up the redundancy
        # in status/area/run_id far better than the default snappy.
        con.execute(
            f"""
            COPY (
                SELECT
                    run_id::VARCHAR        AS run_id,
                    CAST(run_ts AS TIMESTAMP) AS run_ts,
                    revision::VARCHAR      AS revision,
                    test::VARCHAR          AS test,
                    area::VARCHAR          AS area,
                    status::VARCHAR        AS status,
                    duration_ms::INTEGER   AS duration_ms,
                    message::VARCHAR       AS message
                FROM read_json_auto('{jsonl_path}', format='newline_delimited')
            ) TO '{parquet_path}' (FORMAT PARQUET, COMPRESSION ZSTD)
            """
        )

        with open(parquet_path, "rb") as fh:
            return fh.read()


def main() -> None:
    rows = flatten_runs()
    if not rows:
        # Empty parquet still needs the schema so DuckDB-WASM can
        # mount it. Emit a header-only file by giving duckdb a tiny
        # placeholder row that we then DELETE before writing.
        rows = [{
            "run_id": "",
            "run_ts": None,
            "revision": None,
            "test": "",
            "area": "",
            "status": "",
            "duration_ms": None,
            "message": None,
        }]
        blob = write_parquet(rows)
        # We did keep one row above — that's intentional. An empty
        # Parquet with schema-only is fiddly and the placeholder
        # gets filtered out by `WHERE run_id != ''` in the pages.
    else:
        blob = write_parquet(rows)

    sys.stdout.buffer.write(blob)


if __name__ == "__main__":
    main()
