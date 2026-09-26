"""The koala checkout koala-lab runs in, and where it keeps its state."""

from __future__ import annotations

import subprocess
from pathlib import Path

from koala_lab.errors import LabError


def repo_root() -> Path:
    """Top level of the git checkout containing the current directory."""
    result = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise LabError("koala-lab must run inside the koala git checkout")
    return Path(result.stdout.strip())


def state_dir(root: Path) -> Path:
    """Gitignored directory holding archives, builds, and runs."""
    return root / ".koala-lab"


def resolve_rev(root: Path, rev: str) -> str:
    """Full commit hash for `rev`: a branch, tag, hash, or `HEAD`."""
    result = subprocess.run(
        [
            "git",
            "-C",
            str(root),
            "rev-parse",
            "--verify",
            "--quiet",
            f"{rev}^{{commit}}",
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise LabError(f"'{rev}' is not a commit in this repository")
    return result.stdout.strip()
