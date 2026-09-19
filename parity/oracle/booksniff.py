"""Identify containers the way the Python pipeline identifies them.

Reads one `{"name", "hex"}` object per line, writes the bytes to a temporary
file under that name, and reports what each of the Python's container tests
makes of it.

    CATENA_PARITY_PYTHONPATH=/path/to/book-pipeline/src \
        python3 oracle/booksniff.py < cases/booksniff/files.ndjson
"""

import json
import os
import sys
import tempfile
from pathlib import Path

for path in os.environ.get("CATENA_PARITY_PYTHONPATH", "").split(os.pathsep):
    if path:
        sys.path.insert(0, path)

from book_pipeline import convert  # noqa: E402


def main() -> int:
    out = sys.stdout
    with tempfile.TemporaryDirectory() as tmp:
        for line in sys.stdin.buffer.read().decode("utf-8").split("\n"):
            if not line.strip():
                continue
            case = json.loads(line)
            path = Path(tmp) / case["name"]
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(bytes.fromhex(case["hex"]))

            record = {
                "name": case["name"],
                "djvu": convert._is_djvu_document(path),
                "zip": convert._is_zip_container(path),
                "ole2": convert._is_ole2_document(path),
                "zip_kind": None,
                "sole_pdf": None,
            }
            if record["zip"]:
                record["zip_kind"] = convert._classify_zip(path)
                try:
                    import zipfile

                    with zipfile.ZipFile(path) as z:
                        record["sole_pdf"] = convert._sole_pdf_member(z.namelist())
                except Exception:
                    record["sole_pdf"] = None
            out.write(json.dumps(record, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
