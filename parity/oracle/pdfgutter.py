"""Decide 2-up scans the way the book pipeline did, for comparison.

The four functions below are the originals, copied out of the preprocessing
module verbatim — importing it would pull in a PDF renderer and a page tree,
and none of that decides anything. What decides is the arithmetic, which is
what this compares.

    python3 oracle/pdfgutter.py < cases.ndjson

One case per line, tagged with what it is asking about. Writes one JSON
answer per line.
"""

from __future__ import annotations

import json
import sys

import numpy as np

_MIN_RATIO = 1.15
_MIN_GUTTER_FRAC = 0.6
_TEXT_COL_INK = 0.02
_INK_THRESHOLD = 160


def _ratio(width: float, height: float) -> float:
    return width / height if height else 0.0


def _col_ink_profile(pixels: list[int], width: int, height: int) -> list[float]:
    """The reduction the renderer's output went through, minus the render."""
    arr = np.asarray(pixels, dtype=np.uint8).reshape(height, width)
    return [float(x) for x in (arr < _INK_THRESHOLD).mean(axis=0)]


def _has_gutter(ink: np.ndarray) -> bool:
    w = len(ink)
    center = ink[int(0.46 * w):int(0.54 * w)]
    left = ink[int(0.10 * w):int(0.44 * w)]
    right = ink[int(0.56 * w):int(0.90 * w)]
    if center.size == 0 or left.size == 0 or right.size == 0:
        return False
    return bool(
        center.min() < _TEXT_COL_INK
        and np.median(left) > _TEXT_COL_INK
        and np.median(right) > _TEXT_COL_INK
    )


def _sample_indices(n: int, k: int = 8) -> list[int]:
    if n <= 6:
        return list(range(n))
    lo, hi = 2, n - 3
    step = max(1, (hi - lo) // k)
    return list(range(lo, hi + 1, step))[:k]


def _verdict(ratios: list[float], gutters: list[bool]) -> dict:
    median_ratio = float(np.median(ratios)) if ratios else 0.0
    gutter_frac = (sum(gutters) / len(gutters)) if gutters else 0.0
    return {
        "median_ratio": median_ratio,
        "gutter_fraction": gutter_frac,
        "two_up": bool(median_ratio > _MIN_RATIO and gutter_frac >= _MIN_GUTTER_FRAC),
    }


def answer(case: dict) -> dict:
    kind = case["kind"]
    if kind == "ratio":
        return {"ratio": _ratio(case["width"], case["height"])}
    if kind == "gutter":
        return {"gutter": _has_gutter(np.asarray(case["ink"], dtype=float))}
    if kind == "sample":
        return {"indices": _sample_indices(case["pages"])}
    if kind == "greymap":
        return {
            "profile": _col_ink_profile(case["pixels"], case["width"], case["height"])
        }
    if kind == "verdict":
        return _verdict(case["ratios"], case["gutters"])
    raise SystemExit(f"no such case: {kind}")


for line in sys.stdin:
    if not line.strip():
        continue
    case = json.loads(line)
    out = {"name": case["name"], "kind": case["kind"]}
    out.update(answer(case))
    sys.stdout.write(json.dumps(out, sort_keys=True) + "\n")
