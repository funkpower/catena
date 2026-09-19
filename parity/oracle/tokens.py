"""Token counts from the Python tokeniser, one per line of the sample.

Reads the sample on stdin as UTF-8 bytes and splits it on newlines itself,
rather than iterating the stream, so that universal-newline translation
cannot silently change how many lines there are.

    python3 oracle/tokens.py < sample.txt > golden/tokens.counts
"""

import sys

import tiktoken

ENC = tiktoken.get_encoding("cl100k_base")


def main() -> int:
    text = sys.stdin.buffer.read().decode("utf-8")
    out = sys.stdout
    for line in text.split("\n"):
        out.write(f"{len(ENC.encode(line, disallowed_special=()))}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
