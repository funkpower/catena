"""Decode HTML character references the way Python does, one line at a time.

Each line of stdin is one case; each line of stdout is that case decoded and
JSON-encoded, because a decoded reference can itself be a newline.

    python3 oracle/unescape.py < cases/unescape/entities.txt
"""

import html
import json
import sys


def main() -> int:
    text = sys.stdin.buffer.read().decode("utf-8")
    out = sys.stdout
    for line in text.split("\n"):
        out.write(json.dumps(html.unescape(line), ensure_ascii=False) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
