"""The significance test and multiple-comparison correction."""

from __future__ import annotations

import pytest

from koala_lab.stats import holm, p_value, smallest_p


def test_complete_separation_reaches_the_smallest_p() -> None:
    a = [10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0]
    b = [20.0, 21.0, 22.0, 23.0, 24.0, 25.0, 26.0]
    assert p_value(a, b) == pytest.approx(smallest_p(7))
    assert smallest_p(7) == pytest.approx(2 / 3432)


def test_identical_samples_are_not_significant() -> None:
    a = [1.0, 2.0, 3.0, 4.0, 5.0]
    assert p_value(a, a) == pytest.approx(1.0)


def test_five_rounds_cannot_survive_eight_tests() -> None:
    # The reason `compare` defaults to 7 rounds: with 5, even complete
    # separation misses Holm's first threshold for 8 metrics.
    assert smallest_p(5) > 0.05 / 8
    assert smallest_p(7) < 0.05 / 8


def test_holm_matches_a_worked_example() -> None:
    # m = 4, so the sorted p-values face thresholds 0.0125, 0.0167, 0.025,
    # 0.05. Sorted: 0.005 and 0.01 pass; 0.03 fails 0.025, which stops
    # the procedure, so 0.04 is kept even though it is below 0.05.
    assert holm([0.01, 0.04, 0.03, 0.005]) == [True, False, False, True]
    # All four clear their thresholds.
    assert holm([0.01, 0.012, 0.02, 0.04]) == [True, True, True, True]


def test_holm_returns_results_in_input_order() -> None:
    assert holm([0.9, 0.0001]) == [False, True]
