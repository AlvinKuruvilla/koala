"""Release builds of koala at a given commit, cached by commit hash.

Builds happen in a separate worktree under the state directory, so the
user's checkout is never touched, and share one target directory, so a
second commit only recompiles the crates that differ. A finished binary
is copied to `bins/<sha>/koala` and reused for as long as it exists.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

from koala_lab.errors import LabError
from koala_lab.repo import state_dir


def koala_binary(root: Path, sha: str) -> Path:
    """Path to a release `koala` built with the `bench` feature at `sha`,
    building it first if no cached copy exists."""
    state = state_dir(root)
    cached = state / "bins" / sha / "koala"
    if cached.exists():
        return cached

    worktree = state / "worktree"
    if worktree.exists():
        _git(root, "-C", str(worktree), "checkout", "--quiet", "--detach", sha)
    else:
        _git(root, "worktree", "add", "--quiet", "--detach", str(worktree), sha)

    print(f"building koala at {sha[:10]} (cached for later runs)", file=sys.stderr)
    target = state / "target"
    result = subprocess.run(
        ["cargo", "build", "--release", "--features", "bench", "--bin", "koala"],
        cwd=worktree,
        env={**os.environ, "CARGO_TARGET_DIR": str(target)},
        check=False,
    )
    if result.returncode != 0:
        raise LabError(f"cargo build failed at {sha[:10]}; see the output above")

    # Copy under a temporary name and rename, so an interrupted copy never
    # leaves a truncated binary that later runs would trust.
    cached.parent.mkdir(parents=True, exist_ok=True)
    partial = cached.with_suffix(".partial")
    shutil.copy2(target / "release" / "koala", partial)
    partial.rename(cached)
    return cached


def _git(root: Path, *args: str) -> None:
    result = subprocess.run(
        ["git", "-C", str(root), *args], capture_output=True, text=True, check=False
    )
    if result.returncode != 0:
        raise LabError(f"git {' '.join(args)} failed: {result.stderr.strip()}")
