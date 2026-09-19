"""Segment a transcript the way the Python pipeline does.

Reads a cue stream — one `{"start", "end", "text"}` object per line — and a
duration in seconds as the first argument, and writes one chapter per line.

    CATENA_PARITY_PYTHONPATH=/path/to/podcast-pipeline/src \
        python3 oracle/cueseg.py 7245 < cues.ndjson
"""

import json
import os
import sys

for path in os.environ.get("CATENA_PARITY_PYTHONPATH", "").split(os.pathsep):
    if path:
        sys.path.insert(0, path)

from podcast_pipeline.topic_segments import Cue, segment_transcript  # noqa: E402


def main() -> int:
    duration = float(sys.argv[1])
    cues = []
    for line in sys.stdin.buffer.read().decode("utf-8").split("\n"):
        if not line.strip():
            continue
        cue = json.loads(line)
        cues.append(Cue(cue["start"], cue["end"], cue["text"]))

    out = sys.stdout
    for seg in segment_transcript(cues, duration):
        out.write(
            json.dumps(
                {
                    "start": seg.start_time,
                    "end": seg.end_time,
                    "title": seg.fallback_title,
                    "excerpt": seg.excerpt,
                },
                ensure_ascii=False,
            )
            + "\n"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
