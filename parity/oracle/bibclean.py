"""Audit catalogue records the way the Python pipeline audits them.

Reads one `{"title", "author", "isbn", "path"}` object per line and writes
what each of the metadata functions makes of it.

    CATENA_PARITY_PYTHONPATH=/path/to/book-pipeline/src \
        python3 oracle/bibclean.py < cases/bibclean/records.ndjson
"""

import json
import os
import sys

for path in os.environ.get("CATENA_PARITY_PYTHONPATH", "").split(os.pathsep):
    if path:
        sys.path.insert(0, path)

from book_pipeline import metadata  # noqa: E402


def main() -> int:
    out = sys.stdout
    for line in sys.stdin.buffer.read().decode("utf-8").split("\n"):
        if not line.strip():
            continue
        book = json.loads(line)
        title = book.get("title")
        author = book.get("author")
        isbn = book.get("isbn")
        reasons = metadata.problem_reasons(title, author, isbn)
        record = {
            "reasons": reasons,
            "severity": metadata.severity(reasons),
            "title_in_author": metadata.title_in_author(title, author),
            "looks_like_person": metadata._looks_like_person(author or ""),
            "is_shouting": metadata.is_shouting(title),
            "title_case": metadata.title_case(title) if title is not None else None,
            "clean_underscores": metadata.clean_underscores(title) if title is not None else None,
            "valid_isbn": metadata.valid_isbn(isbn),
            "isbn_from_filename": metadata.isbn_from_filename(book.get("path")),
            "normalise": metadata._norm(author),
        }
        out.write(json.dumps(record, ensure_ascii=False, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
