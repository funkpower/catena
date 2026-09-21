#!/usr/bin/env python3
"""Check that every link between the markdown files goes somewhere.

Documentation that links to a page that has been renamed is worse than
documentation that does not link at all, because the reader trusts it. Run
from the repository root:

    python3 tools/check-links.py
"""

import pathlib
import re
import sys

LINK = re.compile(r"\[[^\]]*\]\(([^)]+)\)")


def prose(text: str) -> str:
    """The document with its fenced code blocks taken out.

    An example that happens to contain a link is an example, not a link.
    """
    out, fenced = [], False
    for line in text.split("\n"):
        if line.startswith("```"):
            fenced = not fenced
            continue
        if not fenced:
            out.append(line)
    return "\n".join(out)


def anchors(text: str) -> set[str]:
    """The headings of a document, as the fragments that reach them."""
    found = set()
    for line in prose(text).split("\n"):
        if line.startswith("#"):
            title = line.lstrip("#").strip().lower()
            found.add(re.sub(r"[^a-z0-9 -]", "", title).replace(" ", "-"))
    return found


def main() -> int:
    root = pathlib.Path(".").resolve()
    # Fixtures are not documentation: a test case is deliberately malformed
    # markdown, and a link in one is part of what is being tested.
    skip = {"target", "cases", "golden"}
    files = sorted(
        path
        for path in root.rglob("*.md")
        if skip.isdisjoint(path.parts) and not path.name.startswith(".")
    )
    headings = {path: anchors(path.read_text(encoding="utf-8")) for path in files}

    broken = []
    for path in files:
        for target in LINK.findall(prose(path.read_text(encoding="utf-8"))):
            if target.startswith(("http://", "https://", "mailto:")):
                continue
            where, _, fragment = target.partition("#")
            dest = (path.parent / where).resolve() if where else path
            if not dest.exists():
                broken.append(f"{path.relative_to(root)} -> {target}: no such file")
            elif fragment and dest in headings and fragment not in headings[dest]:
                broken.append(f"{path.relative_to(root)} -> {target}: no such heading")

    for line in broken:
        print(f"broken link: {line}", file=sys.stderr)
    print(f"{len(files)} files, {len(broken)} broken links")
    return 1 if broken else 0


if __name__ == "__main__":
    raise SystemExit(main())
