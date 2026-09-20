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

## pdfgutter — a different rasteriser, so the profiles are compared rather than the pixels

**Python:** renders each sampled page with pdfium at a scale that makes it a
thousand pixels wide, and counts dark pixels per column.

**Rust:** asks `pdftoppm` for the same thousand-pixel-wide greymap and counts
the same way.

**Why this is acceptable:** two rasterisers antialias differently, so a pixel
on the edge of a letterform can land either side of the ink threshold in one
and not the other. No amount of care makes two renderers agree pixel for
pixel, and a parity test that demanded it would be a test of pdfium rather
than of this.

What is compared instead is everything that decides: the aspect ratio, which
pages get sampled, the reduction from pixels to a column profile, whether a
given profile has a gutter, and the verdict a set of ratios and gutters adds
up to. Both sides are handed the same pixels and the same profiles, and have
to agree exactly.

The two renderers do have to be pointed at the same rectangle, which is what
`-cropbox` is for — see the next note.

**Measured:** 94 cases, including profiles either side of the ink threshold by
one ten-thousandth, profiles too narrow to hold the three bands the test looks
at, and greymaps whose heights do not divide evenly so that both sides have to
round a fraction the same way.

## pdfgutter — the page is the crop box, consistently

**Python:** takes the aspect ratio from the media box and the ink profile
from `page.render()`, which draws the **crop box**. On a page that carries
both — most scanned books do — those are two different rectangles, so the
shape is measured on one and the ink on the other. The bands the gutter is
looked for in then fall somewhere other than the middle of what was
rendered, and a real gutter is missed.

**Rust:** the crop box is what a reader shows, so it is what everything
measures: the ratio, the render (`pdftoppm -cropbox`), and where the cut
goes. A page with no crop box is its media box, which is what the spec says
and what most files are.

**Why this is acceptable:** the original is not internally consistent, and on
a wide sheet cropped to show only one of its two pages it gets the answer
wrong in the destructive direction — the sheet still measures wide, so it is
split again, down the middle of a page rather than between two. Measuring one
rectangle throughout is the only version of this that is right.

**Measured:** a wide sheet cropped to one page is no longer a split
candidate, and a cropped spread's gutter lands where it is looked for. The
arithmetic that both sides share is compared exactly, as above.

## pdfgutter — a page tree that does not add up is refused

**Python:** pdfium enumerates the pages and the question never comes up.

**Rust:** the page tree is walked directly, and a walk is stricter than a
renderer: a node missing its `/Type`, a `/Type` written as a reference, a kid
that is a dictionary rather than a reference to one — each of those is a page
a strict walk does not see and a reader does. So the number of pages found is
checked against the number the file says it has, and a document where they
disagree is refused with exit 2.

**Why this is acceptable:** splitting replaces the page tree, and a page left
out of the new tree is a page deleted. A file this does not fully understand
is one it must not rewrite. Refusing costs a document that could perhaps have
been handled; not refusing costs pages, silently, in the copy you keep.

**Measured:** five shapes that a strict walk under-counts, each refused rather
than written short. A document with nothing wide enough to cut is not
rewritten at all, so every page comes through however the tree is built.

## pdfgutter — the thresholds are arguments, not environment variables

**Python:** reads `BOOK_PIPELINE_2UP_MIN_RATIO` and
`BOOK_PIPELINE_2UP_GUTTER_FRAC` at import.

**Rust:** `--min-ratio` and `--gutter-fraction`, with the same defaults.

**Why this is acceptable:** an environment variable read at import makes the
answer depend on the shell that happened to launch the process, which is the
one thing a filter must not do. A flag says the same thing where a reader can
see it, and shows up in the record `--json` writes.

## pdfgutter — a sheet on its side is cut the way it is read

**Python:** takes the media box and ignores `/Rotate` entirely. A sheet stored
tall and displayed wide is never a split candidate; one stored wide and
displayed tall is a candidate, and would be cut down the middle of an axis
that has nothing down the middle of it.

**Rust:** turns the media box by whatever `/Rotate` asks for before measuring
it, and cuts across rather than down for a quarter turn, in the order a
reader would read the halves.

**Why this is acceptable:** the wrong half of a rotated sheet is a page with
the top of one page and the bottom of another on it, and no pipeline
downstream can recover from that. The rotation is in the file precisely so
that readers honour it. Documents with no rotation — nearly all of them —
behave identically.

## pdfgutter — an unreadable file is an answer, not a false

**Python:** `is_double_page` catches everything and returns `False`, so a
corrupt file, an encrypted one and an ordinary single-page book are the same
answer.

**Rust:** exit 2 for a file it cannot judge, exit 74 for one it cannot read,
and exit 1 only for a document it looked at and found to be one page per
sheet.

**Why this is acceptable:** "no" and "could not tell" are different answers
and the contract gives them different numbers. A caller that wants the
Python's behaviour treats both as no; a caller that wants to know which files
it failed on can now find out.

## avplan — separators in a path, and a destination that starts with a dash

**Original:** builds the destination by byte arithmetic — the path, minus as
many bytes as the extension is long, plus `.h264.mp4`. Its extension function
ignores trailing separators while the subtraction does not, so `x.mp4//`
becomes `x.m.h264.mp4`. Duplicate separators are carried through, and a
destination beginning with `-` is emitted bare.

**Rust:** takes the path apart properly. `film.mp4/.` is a directory and has
no extension rather than an `mp4` one; `/talk.webm` becomes `/talk.h264.mp4`
rather than `//talk.h264.mp4`; `x.mp4//` becomes `x.h264.mp4`. A destination
beginning with `-` is written `./-name`, which is the same file and cannot be
read as an option.

**Why this is acceptable:** every one of these is a path the original gets
wrong. A double leading separator is left to the system to interpret and some
systems interpret it as something else; `x.m.h264.mp4` names a file nobody
asked for; a directory reported as a playable MP4 is a wrong answer outright;
and a command line whose destination is read as an option fails on a file
that was otherwise fine. The original never met any of them because its paths
were always joined onto an absolute root.

**Measured:** 32 cases, including a file at the root, a name beginning with a
dash, a name that is only a dot, a non-ASCII name and an extension that is
only a dot. Ordinary paths — the overwhelming majority — are byte-identical.

---

## chunkmd — the back-reference marker a whole class of EPUBs leaves

`looks_like_citations` recognises one note marker the Python it was ported
from did not:

    \[←788 \]
    \[[←193](index_split_011.xhtml#citation-193) \]

An arrow pointing back to where the note was called, sometimes as a link and
sometimes not, wrapped in escaped brackets. It is what a common EPUB
conversion leaves behind, and ten books of the corpus use it — including both
volumes of *The Decline of the West*, whose endnotes run to 830 entries of
`Krumbacher, op. cit., pp. 969-90` and were being read as body prose. Every
one of those books had zero citations extracted.

The parity suite does not cover the footnote signal and no fixture carries
the marker, so the tests agree either way. This is recorded because it is a
deliberate difference from the Python, not because anything caught it.

`citefind` strips the same marker when it cuts a page of notes into notes.

