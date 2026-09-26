"""Corpus manifest parsing, page selection, and archive summaries."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from koala_lab.corpus import Page, load_corpus, manifest_path, select, summarize
from koala_lab.errors import LabError

VALID = """
[[page]]
name = "google"
url = "https://www.google.com/"
exercises = "big JS"

[[page]]
name = "example"
url = "https://example.com/"
exercises = "nothing much"
"""


def write(tmp_path: Path, text: str) -> Path:
    path = tmp_path / "corpus.toml"
    path.write_text(text)
    return path


def test_loads_pages_in_order(tmp_path: Path) -> None:
    pages = load_corpus(write(tmp_path, VALID))
    assert [page.name for page in pages] == ["google", "example"]
    assert pages[0].url == "https://www.google.com/"


def test_committed_manifest_is_valid() -> None:
    root = Path(__file__).resolve().parents[3]
    assert load_corpus(manifest_path(root))


@pytest.mark.parametrize(
    ("entry", "message"),
    [
        ('name = "a"\nurl = "https://a.test/"', "missing 'exercises'"),
        ('name = "A b"\nurl = "https://a.test/"\nexercises = "x"', "lowercase"),
        ('name = "a"\nurl = "file:///a.html"\nexercises = "x"', "must be http"),
    ],
)
def test_rejects_invalid_entries(tmp_path: Path, entry: str, message: str) -> None:
    with pytest.raises(LabError, match=message):
        load_corpus(write(tmp_path, f"[[page]]\n{entry}\n"))


def test_rejects_duplicate_names(tmp_path: Path) -> None:
    text = (
        VALID
        + '\n[[page]]\nname = "google"\nurl = "https://g.test/"\nexercises = "x"\n'
    )
    with pytest.raises(LabError, match="used twice"):
        load_corpus(write(tmp_path, text))


def test_rejects_empty_manifest(tmp_path: Path) -> None:
    with pytest.raises(LabError, match="lists no pages"):
        load_corpus(write(tmp_path, ""))


PAGES = [
    Page("google", "https://g.test/", "x"),
    Page("example", "https://e.test/", "y"),
]


def test_select_defaults_to_every_page() -> None:
    assert select(PAGES, []) == PAGES


def test_select_keeps_the_requested_order() -> None:
    assert [page.name for page in select(PAGES, ["example", "google"])] == [
        "example",
        "google",
    ]


def test_select_names_unknown_pages() -> None:
    with pytest.raises(LabError, match=r"unknown page.*nope.*google, example"):
        select(PAGES, ["nope"])


def test_summarize_counts_responses_and_failures(tmp_path: Path) -> None:
    archive = tmp_path / "page.json"
    archive.write_text(
        json.dumps(
            {
                "format": "koala-fetch-archive",
                "version": 1,
                "entries": {
                    "https://e.test/": {"body": "aGk=", "sha256": "..."},
                    "https://e.test/gone.css": {"error": "connection refused"},
                    "https://e.test/missing": {
                        "body": "",
                        "sha256": "...",
                        "status": 404,
                    },
                },
            }
        )
    )
    recording = summarize(archive)
    assert recording.responses == 3
    assert recording.failures == [
        ("https://e.test/gone.css", "connection refused"),
        ("https://e.test/missing", "HTTP 404"),
    ]
