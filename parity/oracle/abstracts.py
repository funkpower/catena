"""Turn an inverted abstract back into prose the way the policy pipeline did.

The function below is the original, copied out of the journals module verbatim
— importing it opens a database connection and a catalogue client, and neither
decides anything.

    python3 oracle/abstracts.py < cases.ndjson

One case per line: {name, index}. Writes one JSON answer per line.
"""

from __future__ import annotations

import json
import sys


def _decode_abstract(idx: dict | None) -> str | None:
    if not isinstance(idx, dict) or not idx:
        return None
    positions: list[tuple[int, str]] = []
    for word, locs in idx.items():
        if not isinstance(locs, list):
            continue
        for p in locs:
            if isinstance(p, int):
                positions.append((p, str(word)))
    if not positions:
        return None
    positions.sort(key=lambda t: t[0])
    return " ".join(w for _, w in positions)


for line in sys.stdin:
    if not line.strip():
        continue
    case = json.loads(line)
    sys.stdout.write(
        json.dumps(
            {"name": case["name"], "abstract": _decode_abstract(case["index"])},
            sort_keys=True,
        )
        + "\n"
    )
