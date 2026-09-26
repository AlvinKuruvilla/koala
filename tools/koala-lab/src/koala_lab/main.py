"""Command-line entry point for koala-lab."""

from __future__ import annotations

import argparse
import sys
from collections.abc import Callable

from koala_lab import corpus
from koala_lab.build import koala_binary
from koala_lab.errors import LabError
from koala_lab.repo import repo_root, resolve_rev


def main(argv: list[str] | None = None) -> int:
    """Run koala-lab and return a process exit code."""
    parser = _parser()
    args = parser.parse_args(argv)
    handler: Callable[[argparse.Namespace], int] | None = getattr(args, "handler", None)
    if handler is None:
        parser.print_help()
        return 2
    try:
        return handler(args)
    except LabError as err:
        print(f"koala-lab: {err}", file=sys.stderr)
        return 1


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="koala-lab",
        description="Measure koala builds against each other and report what changed.",
    )
    commands = parser.add_subparsers(metavar="COMMAND")

    corpus_cmd = commands.add_parser("corpus", help="the pages koala-lab measures")
    corpus_sub = corpus_cmd.add_subparsers(metavar="ACTION")

    record = corpus_sub.add_parser(
        "record", help="fetch pages through koala and save them for replay"
    )
    record.add_argument("pages", nargs="*", metavar="PAGE", help="default: every page")
    record.add_argument(
        "--rev", default="HEAD", help="build of koala to record with (default: HEAD)"
    )
    record.set_defaults(handler=_record)

    listing = corpus_sub.add_parser("list", help="show pages and what is recorded")
    listing.set_defaults(handler=_list)
    return parser


def _record(args: argparse.Namespace) -> int:
    root = repo_root()
    pages = corpus.select(corpus.load_corpus(corpus.manifest_path(root)), args.pages)
    binary = koala_binary(root, resolve_rev(root, args.rev))
    width = max(len(page.name) for page in pages)
    for page in pages:
        recording = corpus.record(binary, page, corpus.archive_path(root, page))
        failed = f"  {len(recording.failures)} failed" if recording.failures else ""
        print(
            f"{page.name:<{width}}  {recording.responses:>3} responses  "
            f"{_size(recording.size_bytes):>8}{failed}"
        )
        # Failures are replayed as failures, so a recording is still
        # usable; but a page that loses its stylesheet measures a different
        # page, so name each one.
        for url, message in recording.failures:
            print(f"{'':<{width}}  {url}: {message}")
    return 0


def _list(args: argparse.Namespace) -> int:
    del args  # no options
    root = repo_root()
    pages = corpus.load_corpus(corpus.manifest_path(root))
    width = max(len(page.name) for page in pages)
    for page in pages:
        archive = corpus.archive_path(root, page)
        if archive.exists():
            recording = corpus.summarize(archive)
            status = (
                f"{recording.responses:>3} responses  {_size(recording.size_bytes):>8}"
            )
        else:
            status = "not recorded (run: koala-lab corpus record)"
        print(f"{page.name:<{width}}  {status}  {page.url}")
    return 0


def _size(size_bytes: int) -> str:
    if size_bytes >= 1_000_000:
        return f"{size_bytes / 1_000_000:.1f} MB"
    return f"{size_bytes / 1_000:.0f} kB"


if __name__ == "__main__":
    raise SystemExit(main())
