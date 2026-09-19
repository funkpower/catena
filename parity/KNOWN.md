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

## pdftriage — a newer Unicode table knows letters an older one does not

**Python:** `str.isalpha` is true for the letter categories of the Unicode
version the interpreter was built against.

**Rust:** the same test against a table built from a later Unicode version,
which knows 4,924 code points the older one does not.

**Why this is acceptable:** the newer answer is the correct one, and the
difference is entirely one-directional — every code point the two disagree
about is a letter that Rust knows and Python does not, never the reverse. It
matters because the letter share is what calls a text layer dead: a book
typeset in Cyrillic Extended-C is judged garbled by the older table, which
sends a perfectly readable text layer for an hour of OCR that can only make it
worse. Erring toward not re-OCRing a book whose text is fine is the right
direction for this gate to err in.

Both tables will keep moving, which is the other reason this entry exists: the
golden files pin one pair of Unicode versions, and a change to either shows up
as a parity failure rather than as a silently different verdict.

**Measured:** `parity/cases/pdftriage/samples.ndjson`, case
`cyrillic-extended-c`: letter share 1.00 here against 0.00 there, which is the
difference between healthy and garbled. Brute-forced over the whole code
space: 4,924 disagreements, none of them in the other direction.

## mdscrub — un-paginated text is not given a page 1 marker

**Python:** `_normalize_marker_pages` ends by wrapping any text it found no
pagination in as `<!-- Page 1 -->\n\n…`, so that downstream can assume a
marker exists.

**Rust:** page boundaries are not touched at all unless `--pages` asks for
them, and even then text with no separators and no form feeds is returned
unchanged.

**Why this is acceptable:** a cleanup filter has to be safe to run twice, and
the Python's version is not — a second pass wraps the first pass's marker in
another one. The assumption it exists to serve belongs to the converter that
knows it just read a PDF, not to a filter that is handed markdown of unknown
provenance: a form feed in a code listing is a control character, and a dash
rule in a quoted document is not a page break. The chunker already handles a
document with no page markers by emitting chunks with no page number, which is
the honest answer.

Separating the two passes is also what makes the cleanup idempotent. Removing
a tag can leave a dash rule flush against a newline, and a pagination pass
running afterwards would find a page boundary on the second run that was not
there on the first.

**Measured:** not exercised by any parity case, since the Python function is
only reachable from the PDF conversion path. Covered by
`mdscrub::pages::tests::text_with_no_page_boundaries_is_left_exactly_as_it_was`
and `mdscrub::tests::nothing_manufactures_a_page_boundary_on_a_second_run`.

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

Note that this applies only where a page number becomes a *number*. Where one
is copied into text — a page anchor in the body of a document — it is carried
through exactly as it was written, padding and all.

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

## avplan — the encoder is found rather than configured

**Original:** reads `COMPATIBLE_VIDEO_ENCODER` and trusts it, defaulting to
`h264_nvenc`. A machine with no NVIDIA card and an unset variable builds an
NVENC command line that fails the moment it runs.

**Rust:** asks `ffmpeg -encoders` what is installed and takes the best of
them, hardware first.

**Why this is acceptable:** an environment variable naming the encoder is
per-host configuration, and a filter that reads configuration is a filter
whose answer depends on where it ran. Detection reaches the same answer on
every machine the variable was set correctly on, and a better one on the
machines it was not. The encoder table itself — which arguments each encoder
gets — is identical, and `--encoder` names one explicitly for anyone who
needs to.

**Measured:** all four branches of the encoder table are compared against the
original in `cases/avplan`, each with the encoder named, so the argument
lists agree exactly.

## avplan — the output path is the destination, not a temporary

**Original:** writes to `<output>.tmp.mp4` and renames on success, so a
transcode killed halfway leaves no file that looks finished.

**Rust:** the command line names the destination.

**Why this is acceptable:** avplan does not run the command, so it cannot do
the rename half of the pattern, and a command line that writes to a temporary
nobody moves is worse than one that writes where it says. Anything running
these in a loop can write to a temporary of its own choosing with `--output`.

## avplan — a failed probe is an answer, not a fallback

**Original:** when `ffprobe` cannot read a file it falls back to the codec
recorded in the database, which came from the downloader's pre-download
metadata and is wrong often enough that the comment above it says so.

**Rust:** exits 2, could not tell.

**Why this is acceptable:** there is no database, and inventing a codec is
how a compatible file gets re-encoded for forty minutes. Saying so is the
honest answer and the cheap one.
