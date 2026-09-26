"""Release builds of koala to measure.

A commit is built in a separate worktree under the state directory, so
the user's checkout is never touched. All commits share one target
directory, so a second commit only recompiles the crates that differ. A
finished binary is copied to `bins/<sha>/koala` and reused for as long as
it exists.

The revision `.` means the working tree as it is, uncommitted changes
included. It is built in place with its own target directory (sharing the
worktree's would make cargo rebuild every workspace crate each time the
two alternate, since fingerprints include the source path) and never
cached, since there is no commit to key it by.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

from koala_lab.errors import LabError
from koala_lab.repo import resolve_rev, state_dir

WORKING_TREE = "."

_CARGO_BUILD = ["cargo", "build", "--release", "--features", "bench", "--bin", "koala"]


@dataclass(frozen=True)
class Build:
    """A `koala` binary and the source it was built from."""

    # How the user named it: a branch, hash, `HEAD`, or `.`.
    rev: str
    # The commit it was built from; for `.`, the commit the working tree
    # is on.
    sha: str
    # Whether the working tree differed from `sha` (only ever true for `.`).
    dirty: bool
    binary: Path

    def describe(self) -> str:
        if self.rev == WORKING_TREE:
            state = "with uncommitted changes" if self.dirty else "clean"
            return f"working tree ({self.sha[:7]}, {state})"
        if self.rev.startswith(self.sha[:7]):
            return self.sha[:7]
        return f"{self.rev} ({self.sha[:7]})"


def resolve_build(root: Path, rev: str) -> Build:
    """Build `koala` for `rev`, reusing a cached binary when there is one."""
    if rev == WORKING_TREE:
        return _working_tree_build(root)
    sha = resolve_rev(root, rev)
    return Build(rev=rev, sha=sha, dirty=False, binary=_commit_binary(root, sha))


def _commit_binary(root: Path, sha: str) -> Path:
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
    _cargo_build(worktree, target, f"at {sha[:10]}")

    # Copy under a temporary name and rename, so an interrupted copy never
    # leaves a truncated binary that later runs would trust.
    cached.parent.mkdir(parents=True, exist_ok=True)
    partial = cached.with_suffix(".partial")
    shutil.copy2(target / "release" / "koala", partial)
    partial.rename(cached)
    return cached


def _working_tree_build(root: Path) -> Build:
    sha = resolve_rev(root, "HEAD")
    # Untracked files count: a new source file is part of the change.
    status = subprocess.run(
        ["git", "-C", str(root), "status", "--porcelain"],
        capture_output=True,
        text=True,
        check=False,
    )
    dirty = bool(status.stdout.strip())

    print("building koala from the working tree", file=sys.stderr)
    target = state_dir(root) / "target-working-tree"
    _cargo_build(root, target, "in the working tree")
    return Build(
        rev=WORKING_TREE, sha=sha, dirty=dirty, binary=target / "release" / "koala"
    )


def _cargo_build(source: Path, target: Path, where: str) -> None:
    result = subprocess.run(
        _CARGO_BUILD,
        cwd=source,
        env={**os.environ, "CARGO_TARGET_DIR": str(target)},
        check=False,
    )
    if result.returncode != 0:
        raise LabError(f"cargo build failed {where}; see the output above")


def _git(root: Path, *args: str) -> None:
    result = subprocess.run(
        ["git", "-C", str(root), *args], capture_output=True, text=True, check=False
    )
    if result.returncode != 0:
        raise LabError(f"git {' '.join(args)} failed: {result.stderr.strip()}")
