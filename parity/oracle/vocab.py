"""Cluster vocabulary the way the Python pipeline clusters it.

Reads one `{"kind", "items": [{"id","name","count"}]}` object per line and
writes the proposals each list produces.

    CATENA_PARITY_PYTHONPATH=/path/to/book-pipeline/src \
        python3 oracle/vocab.py < cases/vocab/lists.ndjson
"""

import json
import os
import sys

for path in os.environ.get("CATENA_PARITY_PYTHONPATH", "").split(os.pathsep):
    if path:
        sys.path.insert(0, path)

from book_pipeline import vocab  # noqa: E402


def main() -> int:
    out = sys.stdout
    for line in sys.stdin.buffer.read().decode("utf-8").split("\n"):
        if not line.strip():
            continue
        case = json.loads(line)
        items = [
            vocab.VocabItem(calibre_id=i["id"], name=i["name"], book_count=i["count"])
            for i in case["items"]
        ]
        proposals = vocab.cluster(case["kind"], items)
        out.write(
            json.dumps(
                {
                    "keys": [
                        {
                            "norm": vocab._norm(i.name),
                            "author_tokens": list(vocab._author_tokens(i.name)),
                            "pub_key": vocab._pub_key(i.name),
                            "tag_key": vocab._tag_key(i.name),
                            "junk": vocab.junk_tag_reason(i.name),
                        }
                        for i in items
                    ],
                    "canonical": (
                        vocab.pick_canonical(case["kind"], items).name if items else None
                    ),
                    "proposals": [
                        {
                            "action": p.action,
                            "keep": p.keep_name,
                            "drop": p.drop_name,
                            "keep_id": p.keep_calibre_id,
                            "drop_id": p.drop_calibre_id,
                            "keep_count": p.book_count_keep,
                            "drop_count": p.book_count_drop,
                            "method": p.method,
                            "confidence": p.confidence,
                            "evidence": p.evidence,
                        }
                        for p in proposals
                    ],
                },
                ensure_ascii=False,
                sort_keys=True,
            )
            + "\n"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
