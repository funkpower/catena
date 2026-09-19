"""Read a series and a volume off a title the way the book pipeline did.

The functions the module exports are imported rather than copied — it has no
import-time side effects, unlike the modules the other oracles copy from.

    python3 oracle/series.py < cases.ndjson

One case per line: {name, title, author}. Writes one JSON answer per line.
"""

from __future__ import annotations

import json
import sys

from book_pipeline.series import (  # noqa: E402
    parse_publisher_series,
    parse_volume,
    roman_or_int,
)


def answer(case: dict) -> dict:
    kind = case["kind"]
    if kind == "number":
        return {"number": roman_or_int(case["text"])}
    if kind == "volume":
        found = parse_volume(case["title"])
        return {"series": found[0] if found else None,
                "volume": found[1] if found else None}
    if kind == "publisher":
        found = parse_publisher_series(case["title"], case.get("author"))
        return {"proposed_title": found[0] if found else None,
                "series": found[1] if found else None}
    raise SystemExit(f"no such case: {kind}")


for line in sys.stdin:
    if not line.strip():
        continue
    case = json.loads(line)
    out = {"name": case["name"], "kind": case["kind"]}
    out.update(answer(case))
    sys.stdout.write(json.dumps(out, sort_keys=True) + "\n")
