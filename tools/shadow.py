#!/usr/bin/env python3
"""Run a Python pipeline stage and its catena binding over the same corpus,
and report every place they disagree.

This is the thing to run before swapping a stage, and again after. It writes
nothing, changes nothing, and touches no database: it reads a directory of
real files, runs both implementations over each, and compares the answers
field by field.

    tools/shadow.py --stage chunk --corpus /path/to/converted
    tools/shadow.py --stage scrub --corpus /path/to/converted --limit 200

Exit codes follow the suite's own: 0 they agree everywhere, 1 they disagree
somewhere, 69 something needed is not installed, 74 the corpus could not be
read.

The Python side is imported from `book_pipeline`, so PYTHONPATH must point at
its `src`. The catena side is the installed wheel.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

# The commit each stage was ported from. A comparison against a working tree
# that has moved on measures the drift rather than the port, and reports
# failures that are nobody's bug.
PORTED_FROM = "7499e3257572968b437ecfdc79feccc91b96585b"

OK, DIFFER, UNAVAILABLE, IO = 0, 1, 69, 74


def die(code: int, message: str) -> int:
    print(f"shadow: {message}", file=sys.stderr)
    return code


# ---------------------------------------------------------------------------
# The stages
# ---------------------------------------------------------------------------


def chunk_python(text: str) -> list[dict]:
    from book_pipeline import chunk as chunker

    return [
        {
            "idx": c.chunk_idx,
            "page": c.page,
            "chapter": c.chapter_title,
            "text": c.text,
            "tokens": c.token_count,
        }
        for c in chunker.chunk_markdown(text)
    ]


def chunk_catena(text: str) -> list[dict]:
    import catena

    return [
        {
            "idx": c["idx"],
            "page": c["page"],
            "chapter": c["chapter"],
            "text": c["text"],
            "tokens": c["tokens"],
        }
        for c in catena.chunk_markdown(text)
    ]


def scrub_python(text: str) -> str:
    from book_pipeline.convert import _epub_postprocess

    return _epub_postprocess(text)


def scrub_catena(text: str) -> str:
    import catena

    return catena.scrub(text)


def tokens_python(text: str) -> int:
    from book_pipeline.chunk import _ntok

    return _ntok(text)


def tokens_catena(text: str) -> int:
    import catena

    return catena.count_tokens(text)


STAGES = {
    "chunk": (chunk_python, chunk_catena, "book_pipeline.chunk.chunk_markdown"),
    "scrub": (scrub_python, scrub_catena, "book_pipeline.convert._epub_postprocess"),
    "tokens": (tokens_python, tokens_catena, "book_pipeline.chunk._ntok"),
}


# ---------------------------------------------------------------------------
# Comparing
# ---------------------------------------------------------------------------


def differences(theirs, ours) -> list[str]:
    """Every way two answers disagree, in a form that says what to look at."""
    if isinstance(theirs, str) or isinstance(theirs, int):
        if theirs == ours:
            return []
        return [f"python {shorten(theirs)!r} != catena {shorten(ours)!r}"]

    out: list[str] = []
    if len(theirs) != len(ours):
        out.append(f"{len(theirs)} records from python, {len(ours)} from catena")
    for i, (t, o) in enumerate(zip(theirs, ours)):
        for field in t:
            if t[field] != o.get(field):
                out.append(f"[{i}].{field}: {describe(t[field], o.get(field))}")
    return out


def describe(theirs, ours) -> str:
    """How two values differ, shown where they actually differ.

    Two chunk texts usually share a long opening and part company somewhere in
    the middle. Printing the first ninety characters of each shows two
    identical-looking strings and says nothing.
    """
    if not (isinstance(theirs, str) and isinstance(ours, str)):
        return f"python {theirs!r} != catena {ours!r}"

    at = first_difference(theirs, ours)
    if at is None:
        return f"python {len(theirs)} characters != catena {len(ours)}"
    return (
        f"differ at character {at} of {len(theirs)}/{len(ours)}: "
        f"python {window(theirs, at)!r} != catena {window(ours, at)!r}"
    )


def first_difference(a: str, b: str) -> int | None:
    for i, (x, y) in enumerate(zip(a, b)):
        if x != y:
            return i
    return len(a) if len(a) != len(b) else None


def window(text: str, at: int, width: int = 40) -> str:
    """The text around a position, with a marker for what came before."""
    start = max(0, at - width // 4)
    piece = text[start : at + width]
    return ("…" if start else "") + piece + ("…" if at + width < len(text) else "")


def corpus_files(root: Path, limit: int | None) -> list[Path]:
    files = sorted(p for p in root.rglob("*.md") if p.is_file())
    return files[:limit] if limit else files


def ported_commit_matches() -> tuple[bool, str]:
    """Whether the book_pipeline being imported is the one catena was ported
    from."""
    try:
        import book_pipeline
    except ImportError:
        return False, "book_pipeline is not importable"
    repo = Path(book_pipeline.__file__).resolve().parents[2]
    try:
        head = subprocess.run(
            ["git", "-C", str(repo), "rev-parse", "HEAD"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
    except (subprocess.CalledProcessError, FileNotFoundError):
        return False, f"{repo} is not a git checkout"
    if head == PORTED_FROM:
        return True, head
    return False, f"{repo} is at {head[:8]}, ported from {PORTED_FROM[:8]}"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", required=True, choices=sorted(STAGES))
    parser.add_argument("--corpus", required=True, type=Path)
    parser.add_argument("--limit", type=int, help="Look at this many files")
    parser.add_argument(
        "--show", type=int, default=5, help="Differences to print per file"
    )
    parser.add_argument("--json", action="store_true", help="Emit a record per file")
    parser.add_argument(
        "--any-commit",
        action="store_true",
        help="Compare even where the Python has moved since the port",
    )
    args = parser.parse_args()

    try:
        import catena  # noqa: F401
    except ImportError:
        return die(UNAVAILABLE, "catena is not installed: pip install catena")

    matches, where = ported_commit_matches()
    if not matches:
        message = f"the Python is not the port's source — {where}"
        if not args.any_commit:
            return die(
                UNAVAILABLE,
                f"{message}\n         "
                "`git pull` it, or pass --any-commit to compare anyway",
            )
        print(f"shadow: {message}", file=sys.stderr)

    if not args.corpus.is_dir():
        return die(IO, f"{args.corpus}: not a directory")
    files = corpus_files(args.corpus, args.limit)
    if not files:
        return die(IO, f"{args.corpus}: no markdown in it")

    theirs_fn, ours_fn, which = STAGES[args.stage]
    print(
        f"shadow: {args.stage} — {which} against catena, over {len(files)} files",
        file=sys.stderr,
    )

    disagreed = 0
    failed = 0
    for path in files:
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError as e:
            print(f"shadow: {path}: {e}", file=sys.stderr)
            failed += 1
            continue

        try:
            theirs, ours = theirs_fn(text), ours_fn(text)
        except Exception as e:  # noqa: BLE001 — one bad file must not stop the run
            print(f"shadow: {path}: {type(e).__name__}: {e}", file=sys.stderr)
            failed += 1
            continue

        found = differences(theirs, ours)
        if args.json:
            print(
                json.dumps(
                    {
                        "path": str(path),
                        "stage": args.stage,
                        "agrees": not found,
                        "differences": found[: args.show],
                    }
                )
            )
        elif found:
            print(f"\n{path}  ({len(found)} differences)")
            for line in found[: args.show]:
                print(f"  {line}")
            if len(found) > args.show:
                print(f"  … and {len(found) - args.show} more")
        if found:
            disagreed += 1

    agreed = len(files) - disagreed - failed
    print(
        f"\nshadow: {agreed} agreed, {disagreed} disagreed, {failed} could not be read",
        file=sys.stderr,
    )
    return DIFFER if disagreed else OK


if __name__ == "__main__":
    raise SystemExit(main())
