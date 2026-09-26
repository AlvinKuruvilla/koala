"""Command-line entry point for koala-lab."""

from __future__ import annotations

import argparse
import dataclasses
import sys
from collections.abc import Callable
from pathlib import Path

from rich.console import Console
from rich.progress import (
    BarColumn,
    MofNCompleteColumn,
    Progress,
    TextColumn,
    TimeRemainingColumn,
)
from rich.text import Text

from koala_lab import compare, corpus, report
from koala_lab.build import WORKING_TREE, resolve_build
from koala_lab.errors import LabError
from koala_lab.repo import merge_base, repo_root


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
    _ = record.add_argument(
        "pages", nargs="*", metavar="PAGE", help="default: every page"
    )
    _ = record.add_argument(
        "--rev",
        default="HEAD",
        help="koala build to record with: a commit, or . for the working tree",
    )
    record.set_defaults(handler=_record)

    listing = corpus_sub.add_parser("list", help="show pages and what is recorded")
    listing.set_defaults(handler=_list)

    replay_args = corpus_sub.add_parser(
        "args",
        help="print koala's arguments for TARGET, one per line: a corpus page "
        "replays its recording; a path or URL passes through",
    )
    _ = replay_args.add_argument("target", metavar="TARGET")
    replay_args.set_defaults(handler=_args)

    compare_cmd = commands.add_parser(
        "compare", help="measure two builds on the corpus and report what changed"
    )
    _ = compare_cmd.add_argument(
        "a",
        metavar="A",
        nargs="?",
        help="baseline: a commit, or . (default: where this branch left master)",
    )
    _ = compare_cmd.add_argument(
        "b",
        metavar="B",
        nargs="?",
        default=WORKING_TREE,
        help="candidate: a commit, or . (default: . , the working tree)",
    )
    _ = compare_cmd.add_argument(
        "--pages", nargs="+", default=[], metavar="PAGE", help="default: every page"
    )
    _ = compare_cmd.add_argument(
        "--rounds", type=int, default=7, help="processes per build per page"
    )
    compare_cmd.set_defaults(handler=_compare)

    show = commands.add_parser("show", help="report on a stored run again")
    _ = show.add_argument(
        "run", nargs="?", type=Path, help="run directory (default: the latest)"
    )
    show.set_defaults(handler=_show)
    return parser


def _compare(args: argparse.Namespace) -> int:
    root = repo_root()
    pages = corpus.select(corpus.load_corpus(corpus.manifest_path(root)), args.pages)
    if args.rounds < 2:
        raise LabError("--rounds must be at least 2")
    # With no baseline, measure "my change": everything since this branch
    # left master, not also whatever master gained since.
    if args.a is None:
        base = resolve_build(root, merge_base(root, "master"))
        a = dataclasses.replace(base, rev="where this branch left master")
    else:
        a = resolve_build(root, args.a)
    b = resolve_build(root, args.b)

    # A live bar on stderr, cleared when the run ends; `rich` falls back
    # to nothing when stderr is not a terminal.
    total = compare.steps(pages, args.rounds)
    with Progress(
        TextColumn("{task.description}"),
        BarColumn(),
        MofNCompleteColumn(),
        TimeRemainingColumn(),
        console=Console(stderr=True),
        transient=True,
    ) as progress:
        task = progress.add_task("", total=total)
        started = 0

        def on_step(description: str) -> None:
            # Called before each process, so the ones already started
            # are the ones finished.
            nonlocal started
            progress.update(task, description=description, completed=started)
            started += 1

        run = compare.run(root, a, b, pages, args.rounds, on_step)
    _print_report(run)
    return 0


def _show(args: argparse.Namespace) -> int:
    directory = args.run or compare.latest(repo_root())
    _print_report(compare.load(directory))
    return 0


def _print_report(run: compare.Run) -> None:
    # Colour only helps the eye; `rich` drops it when stdout is not a
    # terminal, so piped output is the plain report.
    console = Console()
    for text, style in report.lines(run):
        console.print(Text(text, style=style or ""), soft_wrap=True)


def _record(args: argparse.Namespace) -> int:
    root = repo_root()
    pages = corpus.select(corpus.load_corpus(corpus.manifest_path(root)), args.pages)
    binary = resolve_build(root, args.rev).binary
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


def _args(args: argparse.Namespace) -> int:
    root = repo_root()
    pages = {p.name: p for p in corpus.load_corpus(corpus.manifest_path(root))}
    page = pages.get(args.target)
    if page is None:
        print(args.target)
        return 0
    archive = corpus.archive_path(root, page)
    if not archive.exists():
        raise LabError(f"{page.name} is not recorded (run: koala-lab corpus record)")
    # One per line so callers can split on newlines: the archive path may
    # contain spaces.
    print("--replay", archive, page.url, sep="\n")
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
