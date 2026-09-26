"""Smoke tests for koala_lab."""

from __future__ import annotations

import koala_lab


def test_version_is_set() -> None:
    assert koala_lab.__version__


def test_main_runs() -> None:
    from koala_lab.main import main

    assert main([]) == 0
