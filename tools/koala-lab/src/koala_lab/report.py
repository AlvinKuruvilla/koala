"""The verdict of a comparison run, and the evidence under it.

Two headline metrics per page are tested: whole-load time and whole-render
time. Stage timings (cascade, layout, ...) are shown only under a headline
that changed, as a breakdown, and are not tested: fifteen more tests per
page would push Holm's threshold below what any practical number of
rounds can reach.
"""

from __future__ import annotations

import statistics
from collections.abc import Callable
from dataclasses import dataclass

from koala_lab.compare import Run
from koala_lab.corpus import Page
from koala_lab.probe import Report
from koala_lab.stats import ALPHA, holm, p_value, smallest_p

# Stages shown under a headline that changed, largest change first.
BREAKDOWN_STAGES = 3


@dataclass(frozen=True)
class Test:
    """One headline metric on one page."""

    page: str
    metric: str
    a: list[float]  # one value per process: that process's p50, in µs
    b: list[float]
    p: float

    @property
    def change(self) -> float:
        """Relative change of the medians, b against a."""
        base = statistics.median(self.a)
        return statistics.median(self.b) / base - 1 if base else 0.0


def _load_p50s(reports: list[Report]) -> list[float]:
    return [statistics.median(r.load_total_us) for r in reports]


def _render_p50s(reports: list[Report]) -> list[float]:
    return [
        statistics.median(s.get("render_total", 0) for s in r.render_stages_us)
        for r in reports
    ]


HEADLINES: dict[str, Callable[[list[Report]], list[float]]] = {
    "load": _load_p50s,
    "render": _render_p50s,
}


def tests(run: Run) -> list[Test]:
    out = []
    for page in run.pages:
        side_a, side_b = run.reports[page.name]
        for metric, extract in HEADLINES.items():
            a, b = extract(side_a), extract(side_b)
            out.append(Test(page.name, metric, a, b, p_value(a, b)))
    return out


# A report line and the style a terminal shows it in (`rich` style
# names; `None` for plain). The text alone is the report; styles only
# help the eye.
Line = tuple[str, str | None]


def render(run: Run) -> str:
    """The report as plain text."""
    return "\n".join(text for text, _ in lines(run))


def lines(run: Run) -> list[Line]:
    """The full report: warnings, then one block per page."""
    results = tests(run)
    significant = holm([t.p for t in results])
    rounds = len(run.reports[run.pages[0].name][0])

    out: list[Line] = [
        (f"a  {run.a.describe()}", "bold"),
        (f"b  {run.b.describe()}", "bold"),
        (
            f"{rounds} rounds, {len(run.pages)} pages; raw reports in {run.directory}",
            "dim",
        ),
        ("", None),
    ]

    warnings = _warnings(run, rounds, len(results))
    if warnings:
        out += [(warning, "bold yellow") for warning in warnings]
        out.append(("", None))

    width = max(len(page.name) for page in run.pages)
    for page in run.pages:
        side_a, side_b = run.reports[page.name]
        for i, metric in enumerate(HEADLINES):
            test, changed = next(
                (t, s)
                for t, s in zip(results, significant, strict=True)
                if t.page == page.name and t.metric == metric
            )
            name = page.name if i == 0 else ""
            out.append(
                (f"{name:<{width}}  {_headline(test, changed)}", _style(test, changed))
            )
            if changed:
                out += [
                    (f"{'':<{width}}      {line}", None)
                    for line in _breakdown(metric, side_a, side_b)
                ]
        allocs = _allocations(side_a, side_b)
        if allocs:
            out.append((f"{'':<{width}}  {allocs}", "yellow"))
    return out


def _style(test: Test, changed: bool) -> str:
    if not changed:
        return "dim"
    return "green" if test.change < 0 else "red"


def _headline(test: Test, changed: bool) -> str:
    a, b = statistics.median(test.a), statistics.median(test.b)
    values = f"{_time(a)} -> {_time(b)}  {test.change:+.1%}"
    if not changed:
        return f"{test.metric:<6}  no detectable change   {values}"
    verdict = "faster" if b < a else "slower"
    return f"{test.metric:<6}  {verdict:<21}  {values}  (p={test.p:.4f})"


def _breakdown(metric: str, side_a: list[Report], side_b: list[Report]) -> list[str]:
    """The stages whose medians moved most, for a headline that changed.
    Descriptive only; see the module documentation."""

    def per_stage(reports: list[Report]) -> dict[str, float]:
        samples = (
            [r.load_stages_us for r in reports]
            if metric == "load"
            else [r.render_stages_us for r in reports]
        )
        stages = {name for proc in samples for sample in proc for name in sample}
        return {
            name: statistics.median(
                statistics.median(sample.get(name, 0) for sample in proc)
                for proc in samples
            )
            for name in stages
            if name != "render_total"
        }

    a, b = per_stage(side_a), per_stage(side_b)
    moved = sorted(a.keys() & b.keys(), key=lambda s: abs(b[s] - a[s]), reverse=True)
    return [
        f"{stage:<20} {_time(a[stage])} -> {_time(b[stage])}"
        for stage in moved[:BREAKDOWN_STAGES]
    ]


def _allocations(side_a: list[Report], side_b: list[Report]) -> str | None:
    """A line when allocations per load changed, else `None`.

    Counts are exact on pages whose scripts behave identically every run.
    Scripts that read the clock can take different paths from run to run
    and move the count slightly, so a change counts only when the two
    builds' ranges do not overlap."""
    a = [r.alloc_calls for r in side_a]
    b = [r.alloc_calls for r in side_b]
    if max(a) >= min(b) and max(b) >= min(a):
        return None
    delta = statistics.median(b) - statistics.median(a)
    jitter = max(max(a) - min(a), max(b) - min(b))
    varies = f", varies by up to {jitter:,} between runs" if jitter else ""
    return (
        f"allocs  {statistics.median(a):,.0f} -> {statistics.median(b):,.0f} "
        f"per load ({delta:+,.0f}{varies})"
    )


def _warnings(run: Run, rounds: int, test_count: int) -> list[str]:
    warnings = []
    if smallest_p(rounds) > ALPHA / test_count:
        needed = next(
            n for n in range(rounds, 100) if smallest_p(n) <= ALPHA / test_count
        )
        warnings.append(
            f"! {rounds} rounds cannot show any change across {test_count} tests "
            f"(use --rounds {needed} or more)"
        )
    for page in run.pages:
        warnings += _page_warnings(page, *run.reports[page.name])
    return warnings


def _page_warnings(page: Page, side_a: list[Report], side_b: list[Report]) -> list[str]:
    warnings = []
    digests_a = {r.input_digest for r in side_a}
    digests_b = {r.input_digest for r in side_b}
    if digests_a != digests_b or len(digests_a) > 1:
        warnings.append(
            f"! {page.name}: the builds consumed different inputs (a build "
            "requests a resource the recording lacks, or skips one); its timings "
            "compare different work"
        )
    hashes_a = {r.render_hash for r in side_a}
    hashes_b = {r.render_hash for r in side_b}
    if hashes_a != hashes_b:
        warnings.append(f"! {page.name}: b draws a different frame from a")
    elif len(hashes_a) > 1:
        warnings.append(f"! {page.name}: frames differ between runs of the same build")
    return warnings


def _time(us: float) -> str:
    if us >= 1_000_000:
        return f"{us / 1_000_000:.2f} s"
    if us >= 1_000:
        return f"{us / 1_000:.2f} ms"
    return f"{us:.0f} µs"
