"""Split collections the way the Python pipeline splits them.

Reads a markdown document on stdin and writes what the anthology splitter
makes of it.

    CATENA_PARITY_PYTHONPATH=/path/to/book-pipeline/src \
        python3 oracle/mdsplit.py < case.md
"""

import json
import os
import sys

for path in os.environ.get("CATENA_PARITY_PYTHONPATH", "").split(os.pathsep):
    if path:
        sys.path.insert(0, path)

from book_pipeline import anthology  # noqa: E402


def main() -> int:
    markdown = sys.stdin.buffer.read().decode("utf-8")
    try:
        sections = anthology.detect_sections(markdown)
        record = {
            "split": True,
            "looks_like_bible": anthology._looks_like_bible(markdown),
            "sections": [
                {
                    "idx": s.idx,
                    "title": s.title,
                    "slug": anthology._slugify(s.title),
                    "text": s.text,
                }
                for s in sections
            ],
        }
    except anthology.SplitError:
        record = {
            "split": False,
            "looks_like_bible": anthology._looks_like_bible(markdown),
            "sections": [],
        }
    sys.stdout.write(json.dumps(record, ensure_ascii=False, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
