"""Deciding whether two builds differ.

Each measurement process yields one number per metric (the p50 of its
samples), so a build measured over `n` rounds gives `n` independent
values. Samples inside one process are not independent of each other:
they share the process's CPU-frequency state and allocator history. That
is why the test runs on per-process values, never on pooled samples.

The test is an exact two-sided Mann-Whitney U. It assumes nothing about
the shape of the distribution, which matters because load times have
long right tails. Its smallest possible p-value, reached only when every
process of one build beats every process of the other, is
`2 / C(2n, n)`: 0.0079 for 5 rounds, 0.00058 for 7.

A comparison tests several metrics at once, so Holm's step-down
correction keeps the chance of reporting any false change at or below
`ALPHA` for the whole run.
"""

from __future__ import annotations

from math import comb

from scipy.stats import mannwhitneyu

# Chance the whole run reports at least one change that is not real.
ALPHA = 0.05


def p_value(a: list[float], b: list[float]) -> float:
    """Exact two-sided Mann-Whitney U p-value for `a` against `b`."""
    return float(mannwhitneyu(a, b, alternative="two-sided", method="exact").pvalue)


def smallest_p(n: int) -> float:
    """The smallest p-value `p_value` can return with `n` values per side."""
    return 2 / comb(2 * n, n)


def holm(p_values: list[float], alpha: float = ALPHA) -> list[bool]:
    """Which hypotheses to reject under Holm's step-down procedure.

    Sort the p-values ascending and compare the k-th smallest (k from 0)
    with `alpha / (m - k)`; reject each until the first that fails, and
    none after it. Result is in the input's order.
    """
    m = len(p_values)
    order = sorted(range(m), key=lambda i: p_values[i])
    rejected = [False] * m
    for k, i in enumerate(order):
        if p_values[i] > alpha / (m - k):
            break
        rejected[i] = True
    return rejected
