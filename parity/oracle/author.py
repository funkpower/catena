"""Recover authors the way the Python pipeline recovers them.

Reads one `{"title", "front_matter", "candidate"}` object per line and writes
what each of the author functions makes of it.

    CATENA_PARITY_PYTHONPATH=/path/to/book-pipeline/src \
        python3 oracle/author.py < cases/author/records.ndjson
"""

import json
import os
import sys

for path in os.environ.get("CATENA_PARITY_PYTHONPATH", "").split(os.pathsep):
    if path:
        sys.path.insert(0, path)

from book_pipeline import authors  # noqa: E402


def as_dict(suggestion) -> dict | None:
    if suggestion is None:
        return None
    return {
        "author": suggestion.author,
        "method": suggestion.method,
        "confidence": suggestion.confidence,
        "evidence": suggestion.evidence,
        "isbn": suggestion.isbn,
    }


def main() -> int:
    out = sys.stdout
    for line in sys.stdin.buffer.read().decode("utf-8").split("\n"):
        if not line.strip():
            continue
        case = json.loads(line)
        title = case.get("title") or ""
        front = case.get("front_matter")
        candidate = case.get("candidate")

        record = {
            "front_matter": [as_dict(s) for s in authors.mine_front_matter(front or "")],
            "title": [as_dict(s) for s in authors.mine_title(title)],
            "best": as_dict(authors.best_suggestion(title, front)),
            "clean": authors._clean_name(candidate) if candidate is not None else None,
            "name_prefix": authors._take_name_prefix(candidate) if candidate is not None else None,
        }
        out.write(json.dumps(record, ensure_ascii=False, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
