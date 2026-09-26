"""The pages koala-lab measures, and the archives recorded from them.

The manifest (`corpus.toml`, committed) names each page and its URL. Page
content is never committed: recording loads the page once through
`koala --record`, which saves every response koala fetched to
`<state>/archives/<name>.json`. Measurements replay that archive.
"""

from __future__ import annotations

import json
import re
import subprocess
import tomllib
from dataclasses import dataclass
from pathlib import Path

from koala_lab.errors import LabError
from koala_lab.repo import state_dir

# Names become archive file names and appear in reports.
_NAME = re.compile(r"[a-z0-9][a-z0-9-]*")


@dataclass(frozen=True)
class Page:
    """One corpus entry."""

    name: str
    url: str
    exercises: str


@dataclass(frozen=True)
class Recording:
    """What recording a page captured."""

    responses: int
    size_bytes: int
    # (url, error message) for each fetch that failed while recording.
    failures: list[tuple[str, str]]


def manifest_path(root: Path) -> Path:
    return root / "tools" / "koala-lab" / "corpus.toml"


def load_corpus(path: Path) -> list[Page]:
    """Parse and validate the manifest at `path`."""
    try:
        data = tomllib.loads(path.read_text())
    except (OSError, tomllib.TOMLDecodeError) as err:
        raise LabError(f"cannot read corpus manifest {path}: {err}") from err

    pages: list[Page] = []
    for index, entry in enumerate(data.get("page", [])):
        where = f"{path}, page #{index + 1}"
        try:
            page = Page(
                name=str(entry["name"]),
                url=str(entry["url"]),
                exercises=str(entry["exercises"]),
            )
        except KeyError as err:
            raise LabError(f"{where} is missing '{err.args[0]}'") from err
        if not _NAME.fullmatch(page.name):
            raise LabError(
                f"{where}: name '{page.name}' must be lowercase letters, digits, "
                "and hyphens"
            )
        if not page.url.startswith(("http://", "https://")):
            raise LabError(f"{where}: url '{page.url}' must be http(s)")
        if any(existing.name == page.name for existing in pages):
            raise LabError(f"{where}: name '{page.name}' is used twice")
        pages.append(page)

    if not pages:
        raise LabError(f"{path} lists no pages")
    return pages


def select(pages: list[Page], names: list[str]) -> list[Page]:
    """The pages named in `names`, or all of them when it is empty."""
    if not names:
        return pages
    by_name = {page.name: page for page in pages}
    unknown = [name for name in names if name not in by_name]
    if unknown:
        known = ", ".join(by_name)
        raise LabError(f"unknown page(s) {', '.join(unknown)}; the corpus has {known}")
    return [by_name[name] for name in names]


def archive_path(root: Path, page: Page) -> Path:
    return state_dir(root) / "archives" / f"{page.name}.json"


def record(binary: Path, page: Page, dest: Path) -> Recording:
    """Load `page` once through `binary --record`, replacing `dest` only if
    the recording succeeds."""
    dest.parent.mkdir(parents=True, exist_ok=True)
    partial = dest.with_suffix(".partial")
    result = subprocess.run(
        [str(binary), "--record", str(partial), page.url],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        partial.unlink(missing_ok=True)
        detail = result.stderr.strip().splitlines()[-1:] or ["no output"]
        raise LabError(f"recording {page.name} ({page.url}) failed: {detail[0]}")
    partial.rename(dest)
    return summarize(dest)


def summarize(archive: Path) -> Recording:
    """Count the responses and failures in a recorded archive."""
    entries: dict[str, dict[str, str]] = json.loads(archive.read_text())["entries"]
    failures = [
        (url, entry["error"]) for url, entry in entries.items() if "error" in entry
    ]
    return Recording(
        responses=len(entries),
        size_bytes=archive.stat().st_size,
        failures=failures,
    )
