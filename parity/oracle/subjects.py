"""Apply a subject scheme the way the book pipeline did, for comparison.

The four functions below are the originals, copied out of the subjects module
verbatim and with the module-level scheme lifted into an argument — importing
it opens a database connection and loads a chat client, and neither decides
anything. What decides is the arithmetic of the scheme, which is what this
compares.

    python3 oracle/subjects.py < cases.ndjson

One case per line, carrying its own scheme. Writes one JSON answer per line.
"""

from __future__ import annotations

import json
import re
import sys

_STUDY_TITLE_RE = re.compile(
    r"\b(companion to|guidebook to|very short introduction|"
    r"letters of|collected letters|memoirs?\b|autobiograph|"
    r"\bdiary\b|\bessays\b|\blectures\b|\bspeeches\b|"
    r"a life of|biography of)\b",
    re.IGNORECASE,
)


def canonical_tag(name, scheme):
    subject_lower = {s.lower(): s for s in scheme["subjects"]}
    alias_lower = {k.lower(): v for k, v in scheme["aliases"].items()}
    drop_lower = {d.lower().strip("*[]") for d in scheme["drop"]}
    t = (name or "").strip()
    if not t:
        return None
    low = t.lower()
    if low in drop_lower or low.strip("*[]") in drop_lower:
        return None
    if low in subject_lower:
        return subject_lower[low]
    if low in alias_lower:
        return alias_lower[low]
    return None


def expand_parents(subjects, scheme):
    subject_set = frozenset(scheme["subjects"])
    chosen = set()
    for s in subjects:
        if s not in subject_set:
            continue
        chosen.add(s)
        parent = scheme["parents"].get(s)
        if parent:
            chosen.add(parent)
    return [s for s in scheme["subjects"] if s in chosen]


def sanitize_subjects(subjects, scheme, title=None):
    blocks = frozenset(scheme["blocks_fiction"])
    genres = frozenset(scheme["fiction_genres"])
    chosen = set(subjects)
    study = bool(title and _STUDY_TITLE_RE.search(title))
    if chosen & blocks or study:
        chosen -= genres
    return [s for s in scheme["subjects"] if s in chosen]


def compose_tags(subjects, scheme, title=None):
    return sanitize_subjects(expand_parents(subjects, scheme), scheme, title=title)


def needs_classify(existing, scheme):
    subject_set = frozenset(scheme["subjects"])
    for raw in existing:
        c = canonical_tag(raw, scheme)
        if c and c in subject_set:
            return False
    return True


def answer(case):
    scheme, kind = case["scheme"], case["kind"]
    if kind == "canonical":
        return {"subject": canonical_tag(case["tag"], scheme)}
    if kind == "parents":
        return {"subjects": expand_parents(case["subjects"], scheme)}
    if kind == "fiction":
        return {"subjects": sanitize_subjects(case["subjects"], scheme, title=case.get("title"))}
    if kind == "compose":
        return {"subjects": compose_tags(case["subjects"], scheme, title=case.get("title"))}
    if kind == "needs":
        return {"needs": needs_classify(case["existing"], scheme)}
    raise SystemExit(f"no such case: {kind}")


for line in sys.stdin:
    if not line.strip():
        continue
    case = json.loads(line)
    out = {"name": case["name"], "kind": case["kind"]}
    out.update(answer(case))
    sys.stdout.write(json.dumps(out, sort_keys=True) + "\n")
