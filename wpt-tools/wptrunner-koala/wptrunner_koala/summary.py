"""Post-run summary for a koala wptreport JSON.

Reads the file produced by ``wpt run --log-wptreport=...`` and
prints a digest covering:

- Wall-clock duration of the run
- Test-level status counts (one row per test file: OK / ERROR /
  TIMEOUT / CRASH / ...)
- Subtest-level status counts (PASS / FAIL / TIMEOUT / ...)
- Overall subtest pass rate
- Most common failure patterns (numeric ``expected (X) v1 but got
  (Y) v2`` clauses are normalised so a family of tests that all
  fail with the same skeleton on different properties group
  together)
- The ten slowest tests by reported duration — surfaces hangs,
  near-timeouts, and any test whose pump didn't stop early

Runs as a module so it picks up the same import path / venv the
wptrunner plugin already lives in:

    .venv-wpt/bin/python -m wptrunner_koala.summary /tmp/koala-wpt.json

Lives alongside the executor / browser plugin rather than in a
standalone script so all the koala-specific wpt tooling stays in
one package.
"""

from __future__ import annotations

import json
import re
import sys
from collections import Counter
from pathlib import Path


# Replace the type+value clauses inside common ``assert_*``
# messages so a family of tests that all fail with the same
# skeleton (e.g. ``expected (number) 0 but got (undefined)
# undefined``) on different properties counts as one failure
# pattern instead of N independent ones.
_VALUE_PATTERN = re.compile(
    r"expected\s+\([^)]+\)\s+\S+|but got\s+\([^)]+\)\s+\S+"
)


def normalise_message(msg: str) -> str:
    """Group assertion messages by their invariant structure."""
    return _VALUE_PATTERN.sub(
        lambda m: m.group(0).split(maxsplit=1)[0] + " <…>", msg
    ).strip()


def _pct(n: int, total: int) -> str:
    return f"{(100 * n / total):.1f}%" if total else "n/a"


def summarise(data: dict, source: str = "") -> int:
    """Print the summary for a parsed wptreport. Returns an exit
    code (``0`` on success, ``1`` when the report is unusable)."""
    results = data.get("results", [])
    if not results:
        print("no tests in report")
        return 0

    wall_ms = (data.get("time_end") or 0) - (data.get("time_start") or 0)

    test_status: Counter[str] = Counter()
    subtest_status: Counter[str] = Counter()
    failing: Counter[str] = Counter()
    total_subs = 0

    for r in results:
        test_status[r.get("status", "?")] += 1
        for sub in r.get("subtests", []):
            status = sub.get("status", "?")
            subtest_status[status] += 1
            total_subs += 1
            if status != "PASS":
                msg = (sub.get("message") or "").strip()
                if msg:
                    failing[normalise_message(msg)] += 1

    n_tests = len(results)
    line = "=" * 64
    header = f"WPT run summary ({source})" if source else "WPT run summary"

    print(line)
    print(header)
    print(line)
    print(f"Wall time:      {wall_ms / 1000:7.1f} s")
    print(f"Tests run:      {n_tests:>7}")

    print()
    print("Test status (one per file):")
    for status, count in test_status.most_common():
        print(f"  {status:<22}{count:>6}   {_pct(count, n_tests)}")

    if total_subs:
        print()
        print(f"Subtests:                  {total_subs}")
        for status, count in subtest_status.most_common():
            print(f"  {status:<22}{count:>6}   {_pct(count, total_subs)}")
        passes = subtest_status.get("PASS", 0)
        print()
        print(
            f"Subtest pass rate: {_pct(passes, total_subs)}  "
            f"({passes} / {total_subs})"
        )

    if failing:
        print()
        print("Top failing assertion patterns:")
        for msg, count in failing.most_common(10):
            preview = msg if len(msg) <= 80 else msg[:77] + "..."
            print(f"  {count:>4}  {preview}")

    print()
    print("Slowest tests:")
    slowest = sorted(
        results,
        key=lambda r: r.get("duration", 0) or 0,
        reverse=True,
    )[:10]
    for r in slowest:
        dur = r.get("duration", 0) or 0
        status = r.get("status", "?")
        name = r.get("test", "?")
        print(f"  {dur:>7} ms  [{status:<8}] {name}")

    return 0


def main(argv: list[str] | None = None) -> int:
    args = sys.argv if argv is None else argv
    if len(args) != 2:
        print(f"usage: {args[0]} <wptreport.json>", file=sys.stderr)
        return 2

    path = Path(args[1])
    if not path.exists():
        print(f"report not found: {path}", file=sys.stderr)
        return 1

    try:
        data = json.loads(path.read_text())
    except json.JSONDecodeError as e:
        print(f"could not parse {path}: {e}", file=sys.stderr)
        return 1

    return summarise(data, source=str(path))


if __name__ == "__main__":
    sys.exit(main())
