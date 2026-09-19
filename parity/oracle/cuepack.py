"""Pack cues into passages the way the Python pipeline packs them.

Reads a cue stream — one `{"start", "end", "text"}` object per line — with
the seconds and token budgets as arguments, and writes one passage per line.

    CATENA_PARITY_PYTHONPATH=/path/to/podcast-pipeline/src \
        python3 oracle/cuepack.py 75 512 < cues.ndjson
"""

import json
import os
import sys

for path in os.environ.get("CATENA_PARITY_PYTHONPATH", "").split(os.pathsep):
    if path:
        sys.path.insert(0, path)

from podcast_pipeline.transcripts import Cue, chunk_cues  # noqa: E402


def main() -> int:
    seconds = float(sys.argv[1])
    max_tokens = int(sys.argv[2])
    cues = []
    for line in sys.stdin.buffer.read().decode("utf-8").split("\n"):
        if not line.strip():
            continue
        cue = json.loads(line)
        cues.append(Cue(start=cue["start"], end=cue["end"], text=cue["text"]))

    out = sys.stdout
    for passage in chunk_cues(cues, seconds, max_tokens):
        out.write(
            json.dumps(
                {
                    "idx": passage.chunk_idx,
                    "start_time": passage.start_time,
                    "end_time": passage.end_time,
                    "tokens": passage.token_count,
                    "text": passage.text,
                },
                ensure_ascii=False,
            )
            + "\n"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
