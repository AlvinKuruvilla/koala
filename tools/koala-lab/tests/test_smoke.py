"""Smoke tests for koala_lab."""

from __future__ import annotations

import koala_lab
from koala_lab.main import main


def test_version_is_set() -> None:
    assert koala_lab.__version__


def test_no_command_prints_help_and_fails() -> None:
    assert main([]) == 2
