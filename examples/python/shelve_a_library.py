"""What a book says about itself, checked against what a register says back.

A catalogue's subjects and titles come from wherever its files came from. Two
things can correct that without a model: the cataloguing block the book prints
in its own front matter, and the register that assigned its ISBN. Neither is
trustworthy alone — a scan that starts mid-frontmatter yields a running head
where the title should be, and an ISBN a catalogue has been carrying wrongly
returns a confidently wrong book. Together they are worth acting on.

    python examples/python/shelve_a_library.py converted/*.md

With no arguments it reads a book from Project Gutenberg, which prints no
cataloguing block at all — the honest common case, and worth seeing.
"""

import pathlib
import re
import sys
import urllib.request

import catena

# The closed list this library is browsed by. `Heading = term, term` gives the
# words that file a book under it; a heading on its own matches its own name.
SHELF = """
Philosophy = philosophy, philosophical
Ancient Philosophy = ancient philosophy, plato, aristotle
Political Science = political science, politics, government
Theology = theology, christianity, religion
Church History = church history, ecclesiastical history
Byzantine History = byzantium, byzantine, constantinople
History = history, historiography
Literature = literature, fiction, poetry
Psychology = psychology
Ethics
Logic
"""

GUTENBERG = "https://www.gutenberg.org/cache/epub/5827/pg5827.txt"


def sample(into: pathlib.Path) -> str:
    """Locke's *Essay*, which Gutenberg strips the front matter from."""
    book = into / "locke.txt"
    if not book.exists():
        into.mkdir(parents=True, exist_ok=True)
        print(f"fetching {GUTENBERG}", file=sys.stderr)
        with urllib.request.urlopen(GUTENBERG, timeout=120) as reply:  # noqa: S310
            book.write_bytes(reply.read())
    return book.read_text(encoding="utf-8", errors="replace")


def same(a: str, b: str) -> bool:
    """Whether two printings of one title are the same title.

    Compared on letters and digits alone. A register and a title page differ
    over case, over a curly apostrophe against a straight one, and over
    whether the subtitle is part of the title — none of which is a
    disagreement, and all of which a plain string comparison calls one.
    """
    fold = lambda s: re.sub(r"[^a-z0-9]+", "", (s or "").lower())
    a, b = fold(a), fold(b)
    return bool(a) and bool(b) and (a in b or b in a)


def look(text: str, name: str) -> None:
    printed = catena.read_colophon(text, shelf=SHELF)

    title = printed.get("title")
    isbns = printed.get("isbns") or []
    print(f"\n{name}")
    print(f"  printed title   {title or '— (no title page in these pages)'}")
    if printed.get("authors"):
        print(f"  printed authors {', '.join(printed['authors'])}")
    if printed.get("shelved"):
        print(f"  shelved under   {', '.join(printed['shelved'])}")
    for raw in printed.get("unshelved", []):
        # Not a failure: a heading the shelf has no place for is how you find
        # out the shelf is missing one.
        print(f"  no shelf for    {raw}")
    if not printed.get("subjects"):
        print("  subjects        — (no cataloguing block printed)")

    if not isbns:
        print("  register        — (nothing to look up; one source only)")
        return

    # The only call here that opens a socket, and it blocks.
    found = catena.fetch_identifier(isbns[0], polite="you@example.org")
    if not found or not found.get("found"):
        print(f"  register        {isbns[0]} is unknown to every register asked")
        return

    print(f"  register says   {found['title']} / {', '.join(found.get('authors') or [])}")
    agrees = bool(title) and same(title, found["title"])
    print(f"  verdict         {'corroborated' if agrees else 'two sources disagree — ask a person'}")


def main() -> int:
    paths = [pathlib.Path(p) for p in sys.argv[1:]]
    if not paths:
        look(sample(pathlib.Path("/tmp/catena-example")), "Locke, from Gutenberg")
        return 0
    for path in paths:
        look(path.read_text(encoding="utf-8", errors="replace"), path.name)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
