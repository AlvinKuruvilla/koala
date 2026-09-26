"""Sizing measurement processes, and turning runs into verdicts."""

from __future__ import annotations

from pathlib import Path

from koala_lab.build import Build
from koala_lab.compare import MAX_LOADS, MIN_LOADS, Run, counts_for
from koala_lab.corpus import Page
from koala_lab.probe import Counts, Report
from koala_lab.report import render


def test_slow_page_gets_the_minimum_loads() -> None:
    counts = counts_for(first_load_us=2_000_000)
    assert counts.loads == MIN_LOADS


def test_fast_page_is_capped() -> None:
    assert counts_for(first_load_us=10).loads == MAX_LOADS


def test_mid_page_fills_the_budget() -> None:
    # 3 s of measurement at 50 ms per load.
    assert counts_for(first_load_us=50_000).loads == 60


def report(load_us: int, render_hash: str = "frame", allocs: int = 1000) -> Report:
    return Report(
        load_total_us=[load_us] * 5,
        load_stages_us=[{"css_cascade": load_us // 2}] * 5,
        render_stages_us=[{"render_total": 500}] * 5,
        alloc_calls=allocs,
        alloc_bytes=0,
        render_hash=render_hash,
        input_digest="input",
        raw={},
    )


def make_run(side_a: list[Report], side_b: list[Report]) -> Run:
    build = Build(rev="HEAD", sha="0123456789abcdef", dirty=False, binary=Path())
    page = Page("example", "https://example.com/", "")
    return Run(
        a=build,
        b=build,
        pages=[page],
        counts={"example": Counts(2, 5, 5)},
        reports={"example": (side_a, side_b)},
        directory=Path("run"),
    )


def test_clear_slowdown_is_reported_with_its_stage() -> None:
    a = [report(1000 + i) for i in range(7)]
    b = [report(2000 + i) for i in range(7)]
    text = render(make_run(a, b))
    assert "load    slower" in text
    assert "css_cascade" in text
    assert "render  no detectable change" in text


def test_identical_builds_report_no_change_and_no_warnings() -> None:
    a = [report(1000 + i) for i in range(7)]
    text = render(make_run(a, list(a)))
    assert "slower" not in text
    assert "faster" not in text
    assert "!" not in text


def test_different_frames_are_flagged() -> None:
    a = [report(1000) for _ in range(7)]
    b = [report(1000, render_hash="other") for _ in range(7)]
    assert "b draws a different frame from a" in render(make_run(a, b))


def test_allocation_change_outside_both_ranges_is_shown() -> None:
    a = [report(1000, allocs=1000) for _ in range(7)]
    b = [report(1000, allocs=900) for _ in range(7)]
    assert "allocs  1,000 -> 900 per load (-100)" in render(make_run(a, b))


def test_too_few_rounds_says_how_many_are_needed() -> None:
    a = [report(1000 + i) for i in range(3)]
    b = [report(2000 + i) for i in range(3)]
    assert "use --rounds 5 or more" in render(make_run(a, b))
