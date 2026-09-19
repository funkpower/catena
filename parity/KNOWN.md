# Known divergences

Places where the Rust and the Python deliberately do not agree, each with the
reason it is acceptable. A divergence that is not written down here is a bug,
and loosening a test instead of adding an entry here is how a port rots.

Format:

```
## <function> — <one-line summary>

**Python:** what it does.
**Rust:** what it does instead.
**Why this is acceptable:** the argument, including what input class is
affected and how often it occurs in practice.
**Measured:** how many records out of how many differ.
```

---

## cuenorm — a cue line with trailing settings still reads

**Original:** the timestamp line is split on the arrow and each half is read
whole, so `00:00:01,000 --> 00:00:03,000  X1:040 X2:560` gives an end time
that is not a number and the cue is dropped entirely.

**Rust:** each half is reduced to the one token that could be a timestamp, so
the trailing text is ignored and the cue reads.

**Why this is acceptable:** it is what makes the same reader take WebVTT,
where positioning settings after the end time are not a malformation but the
format — `00:00:01.000 --> 00:00:03.000 line:0 position:20%` is an ordinary
cue. Without it the tool would need a second parser that differed in one
line. The divergence can only ever recover a cue that the original discarded;
there is no input where it changes a cue the original read.

**Measured:** `parity/cases/cuenorm/coordinates.srt` — one cue of two
recovered. Across the other eight cases, including deliberately malformed
timestamps, the two agree cue for cue.

## mdscrub — un-paginated text is not given a page 1 marker

**Python:** `_normalize_marker_pages` ends by wrapping any text it found no
pagination in as `<!-- Page 1 -->\n\n…`, so that downstream can assume a
marker exists.

**Rust:** text with no page separators and no form feeds is returned
unchanged.

**Why this is acceptable:** a cleanup filter has to be safe to run twice, and
the Python's version is not — a second pass wraps the first pass's marker in
another one. The assumption it exists to serve belongs to the converter that
knows it just read a PDF, not to a filter that is handed markdown of unknown
provenance. The chunker already handles a document with no page markers by
emitting chunks with no page number, which is the honest answer.

**Measured:** not exercised by any parity case, since the Python function is
only reachable from the PDF conversion path. Covered by
`mdscrub::pages::tests::text_with_no_page_boundaries_is_left_exactly_as_it_was`.

## Every ported pattern — the two engines disagree about `\d` and `(?i)`

**Python:** `re` compiles against the Unicode version of the interpreter;
`(?i)` folds `ı` and `İ` onto `i`.

**Rust:** `regex` compiles against the Unicode version it was built with, and
`(?i)` uses simple case folding, which leaves the dotless and dotted `i`
alone.

**Why this is acceptable:** `\s`, `\w` and `\b` were the differences that
mattered and they are closed — every ported pattern now spells Python's
classes explicitly, from `textkit::pattern`. What is left is a `\d` that
disagrees only on decimal digits added between the two Unicode versions
(Garay, added in Unicode 16, is the current example), and a case fold that
disagrees only for Turkish dotless `i`. A chapter heading reading
`Bıblıography` is classified as body text here and as a bibliography there;
a page marker written in Garay digits reads here and does not there.

Both will drift again at every dependency bump, which is the real reason this
entry exists: the golden files pin one pair of Unicode versions, and a change
to either shows up as a parity failure rather than as a silent difference.

**Measured:** zero differences over the parity corpus. Found by brute-forcing
every code point below `U+11000` through both engines: `\d` differs on ten,
`(?i)` on three.

## chunkmd — a page number too large for the field saturates

**Python:** integers are unbounded, so `<!-- Page 99999999999 -->` records
page 99,999,999,999.

**Rust:** the page field is a 32-bit number and saturates at 4,294,967,295.

**Why this is acceptable:** the number is a page number. A document with four
billion pages does not exist, and the alternative — a 64-bit field — would
make every chunk record wider to carry a value nothing can produce. The
saturation is deliberate rather than a wrap, so the wrong answer is at least
obviously wrong.

**Measured:** no document in any corpus this was run against has a page
marker above five digits.

## chunkmd — a hard maximum of zero yields nothing rather than raising

**Python:** `_enforce_hard_max` with `hard_max=0` raises `ValueError:
range() arg 3 must not be zero`.

**Rust:** `chunkmd::chunk` returns no chunks, since no piece could satisfy
the limit.

**Why this is acceptable:** the command-line tool rejects it before reading
any input, so this is reachable only by a library caller, and a filter that
returns nothing for a budget that permits nothing is the more useful answer
of the two. Every other degenerate budget — overlap at or above target, a
hard maximum below the target, a minimum of zero — agrees with the Python
exactly.

**Measured:** checked against the oracle for all of those combinations; only
the zero hard maximum differs.
