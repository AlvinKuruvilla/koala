"""The error koala-lab reports to the user."""

from __future__ import annotations


class LabError(Exception):
    """A failure the user can act on. The message says what went wrong and
    what to do; the CLI prints it without a traceback."""
