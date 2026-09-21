"""A book to rows a database can take, without a subprocess in sight.

This is the shape a pipeline stage ends up as once the reading is a library
call: convert, then loop, and what is left is the part that is actually about
your schema.

    python examples/python/book_to_citations.py path/to/book.epub

With no argument it fetches Gibbon's first volume from Project Gutenberg,
which is public domain and has a few hundred pages of footnotes.
"""

import pathlib
import subprocess
import sys
import urllib.request

import catena

GUTENBERG = "https://www.gutenberg.org/cache/epub/731/pg731.epub"


def fetch(into: pathlib.Path) -> pathlib.Path:
    """The sample book, downloaded once."""
    book = into / "gibbon.epub"
    if not book.exists():
        into.mkdir(parents=True, exist_ok=True)
        print(f"fetching {GUTENBERG}")
        with urllib.request.urlopen(GUTENBERG, timeout=120) as reply:  # noqa: S310
            book.write_bytes(reply.read())
    return book


def to_markdown(book: pathlib.Path) -> str:
    """Pandoc is the one part of this that is somebody else's program.

    `commonmark` rather than `commonmark_x`: the extended flavour writes
    anchors as bracketed spans, and they end up inside the passages.
    """
    done = subprocess.run(
        ["pandoc", "-f", "epub", "-t", "commonmark", "--wrap=none", str(book)],
        capture_output=True,
        text=True,
        check=True,
    )
    return done.stdout


def rows(markdown: str) -> list[dict]:
    """Passages in, citation rows out.

    Two library calls do the work. `chunk_markdown` cuts the book into
    passages and flags the ones carrying citation material — which is the
    whole reason to ask it rather than looking for a heading, since a page of
    notes at the foot of a chapter has no heading over it. `find_citations`
    then reads each of those.
    """
    out = []
    for chunk in catena.chunk_markdown(catena.scrub(markdown)):
        if "footnote_citations" not in chunk["flags"] and chunk["role"] != "bibliography":
            continue
        for found in catena.find_citations(chunk["text"]):
            # An `ibid.` or a short cite points at an entry above rather than
            # naming a work of its own, so there is nothing to store.
            if found["continuation"]:
                continue
            if not (found["title"] or found["authors"] or found["ids"]):
                continue
            out.append(
                {
                    "page": chunk["page"],
                    "chapter": chunk["chapter"],
                    "citation_text": found["text"],
                    "referenced_work": found["title"],
                    "referenced_author": "; ".join(found["authors"]) or None,
                    "referenced_year": found["year"],
                    "referenced_container": found["container"],
                    "kind": found["kind"],
                    "unsettled": found["unsettled"],
                }
            )
    return out


def main() -> int:
    where = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else fetch(pathlib.Path("work"))

    print(f"== {where.name}")
    identified = catena.identify(where.name, where.read_bytes())
    print(f"   a {identified['kind']}, {identified['bytes']} bytes")

    markdown = to_markdown(where)
    passages = catena.chunk_markdown(catena.scrub(markdown))
    carrying = [c for c in passages if "footnote_citations" in c["flags"]]
    print(f"   {len(passages)} passages, {len(carrying)} carrying citation material")

    found = rows(markdown)
    named = [r for r in found if r["referenced_work"]]
    print(f"\n== {len(found)} rows, {len(named)} naming a work")

    # What a stage like this would do next is its own business: one executemany
    # against your own table. The point is that everything above it is two
    # function calls rather than a parser you have to keep.
    print("\n   INSERT INTO citations (citation_text, referenced_work, referenced_author, …)")
    whole = [r for r in named if r["referenced_author"]] or named
    for row in whole[:5]:
        work = (row["referenced_work"] or "")[:40]
        who = (row["referenced_author"] or "—")[:22]
        print(f"     (…, {work!r}, {who!r}, {row['referenced_year']})")

    # Nothing is invented. Where a field could not be read the record says so
    # rather than guessing, and a caller that wants a model to fill the gaps
    # has `citation_prompt` and `merge_citation_reply` for exactly that.
    # Gibbon cites in the classical short form — `Tacit. Annal. xiii. 5.` —
    # so most of his notes name a passage rather than a book. Nothing is
    # invented to cover that: the record says which fields it could not read,
    # and `citation_prompt` and `merge_citation_reply` are there for a caller
    # that wants a model to finish them.
    unsettled = [r for r in found if r["unsettled"]]
    print(f"\n   {len(unsettled)} rows name something they could not finish reading:")
    for row in unsettled[:3]:
        print(f"     {row['citation_text'][:56]} → {', '.join(row['unsettled'])}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
