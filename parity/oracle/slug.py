"""Judge slugs the way the Python pipeline judges them.

Reads one `{"slug", "author", "title", "id"}` object per line and writes what
the slug module makes of each.

    CATENA_PARITY_PYTHONPATH=/path/to/book-pipeline/src \
        python3 oracle/slug.py < cases/slug/records.ndjson
"""

import json
import os
import sys

for path in os.environ.get("CATENA_PARITY_PYTHONPATH", "").split(os.pathsep):
    if path:
        sys.path.insert(0, path)

from slugify import slugify  # noqa: E402

from book_pipeline import slugs  # noqa: E402


def main() -> int:
    out = sys.stdout
    for line in sys.stdin.buffer.read().decode("utf-8").split("\n"):
        if not line.strip():
            continue
        record = json.loads(line)
        author, title, book_id = record.get("author"), record.get("title"), record["id"]
        proposal = slugs.evaluate("x", record["slug"], author, title, book_id)
        out.write(
            json.dumps(
                {
                    "slugify": slugify(title or ""),
                    "slug_for": slugs.slug_for(author, title, book_id),
                    "lead_author": slugs._lead_author(author),
                    "tier": proposal.tier if proposal else None,
                    "proposed": proposal.proposed_slug if proposal else None,
                    "reasons": proposal.reasons if proposal else [],
                },
                ensure_ascii=False,
                sort_keys=True,
            )
            + "\n"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
