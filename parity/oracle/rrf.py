"""Fuse ranked lists the way the retrieval layer fuses them.

The pipeline's fusion is written inline in a database query function, so the
scoring loop is spelled out again here — but the two constants that decide
everything are imported from it, so a change to either shows up as a parity
failure rather than as a silently different ranking.

Reads hit records on stdin and writes `{"id", "rank", "score"}` per line.

    CATENA_PARITY_PYTHONPATH=/path/to/knowledge-mcp/src \
        python3 oracle/rrf.py < hits.ndjson
"""

import json
import os
import sys

for path in os.environ.get("CATENA_PARITY_PYTHONPATH", "").split(os.pathsep):
    if path:
        sys.path.insert(0, path)

from knowledge_mcp.retrieval import META_WEIGHT, RRF_K  # noqa: E402

WEIGHTS = {"vec": 1.0, "fts": 1.0, "meta": META_WEIGHT}


def main() -> int:
    pool: dict[str, dict] = {}
    for line in sys.stdin.buffer.read().decode("utf-8").split("\n"):
        if not line.strip():
            continue
        hit = json.loads(line)
        entry = pool.setdefault(hit["id"], {"score": 0.0, "seen": {}})
        channel = hit["channel"]
        weight = WEIGHTS.get(channel, 1.0)
        previous = entry["seen"].get(channel)
        if previous is not None:
            entry["score"] -= weight / (RRF_K + previous)
        entry["seen"][channel] = hit["rank"]
        entry["score"] += weight / (RRF_K + hit["rank"])

    # Python's dict keeps insertion order and its sort is stable, so ties come
    # out in the order the ids were first seen.
    ranked = sorted(pool.items(), key=lambda kv: kv[1]["score"], reverse=True)

    out = sys.stdout
    for rank, (hit_id, entry) in enumerate(ranked, 1):
        out.write(json.dumps({"id": hit_id, "rank": rank, "score": entry["score"]}) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
