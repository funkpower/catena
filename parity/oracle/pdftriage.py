"""Measure text layers the way the Python pipeline measures them.

Reads one `{"name", "text"}` JSON object per line and writes back the numbers
each gate is decided on, so the judgement can be compared without shipping any
book. Two of the measurements — the letter share and the Latin-1 share — are
written inline in the Python's garbled gate rather than in a function of their
own, so they are spelled out again here, copied line for line.

    CATENA_PARITY_PYTHONPATH=/path/to/book-pipeline/src \
        python3 oracle/pdftriage.py < cases/pdftriage/samples.ndjson
"""

import json
import os
import sys

for path in os.environ.get("CATENA_PARITY_PYTHONPATH", "").split(os.pathsep):
    if path:
        sys.path.insert(0, path)

from book_pipeline import convert  # noqa: E402


def letter_share(body: list[str]) -> float:
    if not body:
        return 0.0
    return sum(1 for c in body if c.isalpha()) / len(body)


def latin1_share(body: list[str]) -> float:
    if not body:
        return 0.0
    return sum(1 for c in body if 0xA1 <= ord(c) <= 0xFF) / len(body)


def english_share(text: str, min_words: int = 80) -> float | None:
    words = [w.strip(".,;:!?()[]\"'").lower() for w in text.split()]
    words = [w for w in words if w]
    if len(words) < min_words:
        return None
    return sum(1 for w in words if w in convert._COMMON_WORDS) / len(words)


def main() -> int:
    out = sys.stdout
    for line in sys.stdin.buffer.read().decode("utf-8").split("\n"):
        if not line.strip():
            continue
        case = json.loads(line)
        text = case["text"]
        body = [c for c in text if not c.isspace()]
        shape = convert._word_shape(text)
        decoded = convert.undo_reflected_font(text)
        record = {
            "name": case["name"],
            "body": len(body),
            "ascii": convert._ascii_text_score(text),
            "letters": letter_share(body),
            "latin1": latin1_share(body),
            "vowelled": None if shape is None else shape[0],
            "tidy": None if shape is None else shape[1],
            "english": english_share(text),
            "decoded": decoded,
        }
        out.write(json.dumps(record, ensure_ascii=False, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
