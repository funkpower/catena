"""Clean converted markdown the way the Python pipeline does.

Reads markdown on stdin, writes the cleaned markdown on stdout.

    CATENA_PARITY_PYTHONPATH=/path/to/book-pipeline/src \
        python3 oracle/mdscrub.py < case.md
"""

import os
import sys

for path in os.environ.get("CATENA_PARITY_PYTHONPATH", "").split(os.pathsep):
    if path:
        sys.path.insert(0, path)

from book_pipeline.convert import _epub_postprocess  # noqa: E402


def main() -> int:
    markdown = sys.stdin.buffer.read().decode("utf-8")
    sys.stdout.write(_epub_postprocess(markdown))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
