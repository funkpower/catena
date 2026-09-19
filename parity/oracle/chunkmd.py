"""Chunk markdown the way the Python pipeline does, for comparison.

Reproduces the production order exactly: chunk, then the footnote signal on
the text *before* the structural marker is stripped, then role classification
and smoothing across the whole document.

    CATENA_PARITY_PYTHONPATH=/path/to/book-pipeline/src \
        python3 oracle/chunkmd.py 512 64 50 768 < case.md

Emits one JSON record per line, in the field order the Rust records use.
"""

import json
import os
import sys

for path in os.environ.get("CATENA_PARITY_PYTHONPATH", "").split(os.pathsep):
    if path:
        sys.path.insert(0, path)

from book_pipeline import chunk as chunker  # noqa: E402
from book_pipeline.citations import looks_like_footnote_citations  # noqa: E402
from book_pipeline.convert import FOOTNOTE_SENTINEL  # noqa: E402


def main() -> int:
    target, overlap, min_tokens, hard_max = (int(a) for a in sys.argv[1:5])
    markdown = sys.stdin.buffer.read().decode("utf-8")

    chunks = chunker.chunk_markdown(
        markdown,
        target=target,
        overlap=overlap,
        min_tokens=min_tokens,
        hard_max=hard_max,
    )

    footnotes = {c.chunk_idx: looks_like_footnote_citations(c.text) for c in chunks}
    for c in chunks:
        c.text = c.text.replace(FOOTNOTE_SENTINEL, "")

    roles = chunker.smooth_roles(
        [chunker.classify_chunk_role(c.text, c.chapter_title) for c in chunks]
    )

    out = sys.stdout
    for c, role in zip(chunks, roles, strict=True):
        record = {
            "v": 1,
            "idx": c.chunk_idx,
            "text": c.text,
            "tokens": c.token_count,
            "page": c.page,
            "chapter": c.chapter_title,
            "role": role,
            "start_time": None,
            "end_time": None,
            "flags": ["footnote_citations"] if footnotes[c.chunk_idx] else [],
        }
        out.write(json.dumps(record, ensure_ascii=False, separators=(",", ":")) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
