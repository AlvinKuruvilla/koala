"""Aggregate every wptreport.json under ``dashboard/runs/`` into a
single JSON document for the dashboard pages.

This is an Observable Framework "data loader" — when a page does
``FileAttachment("data/runs.json").json()``, the framework runs this
script at build time, captures stdout, and serves the result as
``runs.json``. See https://observablehq.com/framework/loaders.

Output schema:

    {
      "runs": [
        {
          "id": "2026-05-26T16-18-40_3b60711",
          "timestamp": "2026-05-26T16:18:40Z",
          "revision": "3b607118...",
          "product": "koala",
          "totals": {"PASS": 7, "FAIL": 31, "CRASH": 0, ...},
          "pass_rate": 0.184,
          "areas": [
            {"area": "css", "PASS": 7, "FAIL": 31, "total": 38, "pass_rate": 0.184},
            ...
          ],
          "results": [
            {"test": "/css/.../foo.html", "status": "PASS",
             "area": "css", "duration_ms": 65, "message": null},
            ...
          ]
        },
        ...
      ]
    }

Runs are sorted oldest-first so dashboards can take ``runs[-1]`` for
the latest run and walk forward for history charts.
"""

from __future__ import annotations

import json
import sys
from datetime import datetime, timezone
from pathlib import Path


def runs_dir() -> Path:
    """The dashboard/runs/ directory, sibling of dashboard/src/.

    The data loader's working directory is opaque (Observable
    Framework's discretion), so resolve the path relative to this
    file rather than CWD.
    """
    return Path(__file__).resolve().parents[2] / "runs"


def parse_timestamp_from_filename(path: Path) -> str | None:
    """Extract the ISO timestamp from filenames of the form
    ``2026-05-26T16-18-40_<sha>.json``. Returns None if the filename
    doesn't match (e.g. ``.gitkeep``)."""
    name = path.stem
    parts = name.split("_", 1)
    ts_raw = parts[0]
    # Reverse the dash-for-colon swap we did when writing the file.
    try:
        date_part, time_part = ts_raw.split("T", 1)
        time_part = time_part.replace("-", ":")
        return f"{date_part}T{time_part}Z"
    except ValueError:
        return None


def area_of(test_path: str) -> str:
    """Top-level WPT area for a test path, e.g. ``/css/CSS2/visudet/foo.html``
    becomes ``"css"``. Tests that don't have a leading-slash path fall
    back to ``"other"``."""
    if not test_path.startswith("/"):
        return "other"
    parts = test_path[1:].split("/", 1)
    return parts[0] if parts and parts[0] else "other"


def summarize_run(report: dict, path: Path) -> dict:
    """Reduce one wptreport.json into the dashboard's run shape."""
    timestamp = parse_timestamp_from_filename(path)
    if timestamp is None and "time_start" in report:
        timestamp = datetime.fromtimestamp(
            report["time_start"] / 1000, tz=timezone.utc
        ).isoformat().replace("+00:00", "Z")

    results = report.get("results", [])
    totals: dict[str, int] = {}
    areas: dict[str, dict[str, int]] = {}
    flat_results = []

    for r in results:
        status = r.get("status", "UNKNOWN")
        test = r.get("test", "")
        area = area_of(test)

        totals[status] = totals.get(status, 0) + 1
        area_bucket = areas.setdefault(area, {})
        area_bucket[status] = area_bucket.get(status, 0) + 1
        area_bucket["total"] = area_bucket.get("total", 0) + 1

        flat_results.append({
            "test": test,
            "status": status,
            "area": area,
            "duration_ms": r.get("duration"),
            "message": r.get("message"),
        })

    total = sum(totals.values())
    passed = totals.get("PASS", 0) + totals.get("OK", 0)
    pass_rate = passed / total if total else 0.0

    area_list = []
    for area, counts in sorted(areas.items()):
        area_total = counts["total"]
        area_passed = counts.get("PASS", 0) + counts.get("OK", 0)
        area_list.append({
            "area": area,
            "total": area_total,
            "PASS": counts.get("PASS", 0),
            "FAIL": counts.get("FAIL", 0),
            "CRASH": counts.get("CRASH", 0),
            "TIMEOUT": counts.get("TIMEOUT", 0),
            "ERROR": counts.get("ERROR", 0),
            "pass_rate": area_passed / area_total if area_total else 0.0,
        })

    run_info = report.get("run_info", {})

    return {
        "id": path.stem,
        "timestamp": timestamp,
        "revision": run_info.get("revision"),
        "product": run_info.get("product", "koala"),
        "totals": totals,
        "total": total,
        "pass_rate": pass_rate,
        "areas": area_list,
        "results": flat_results,
    }


def main() -> None:
    runs = []
    base = runs_dir()
    if base.exists():
        for path in sorted(base.glob("*.json")):
            try:
                with path.open() as fh:
                    report = json.load(fh)
            except (OSError, json.JSONDecodeError) as exc:
                print(f"# skipping {path.name}: {exc}", file=sys.stderr)
                continue
            runs.append(summarize_run(report, path))

    json.dump({"runs": runs}, sys.stdout, indent=2)


if __name__ == "__main__":
    main()
