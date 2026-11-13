# Decisions

Places where one of these tools does something a reader might reasonably
expect it not to, each with the argument for it. The obvious behaviour is not
written down; this is the list of the deliberate exceptions, so that a change
to any of them is a change to a decision rather than to an accident.

- [cuenorm — settings after the end time are the format, not a malformation](#cuenorm--settings-after-the-end-time-are-the-format-not-a-malformation)
- [pdftriage — the letter table is the newest one available](#pdftriage--the-letter-table-is-the-newest-one-available)
- [mdscrub — nothing manufactures a page boundary](#mdscrub--nothing-manufactures-a-page-boundary)
- [chunkmd — a page number saturates rather than widening every record](#chunkmd--a-page-number-saturates-rather-than-widening-every-record)
- [chunkmd — a budget that permits nothing yields nothing](#chunkmd--a-budget-that-permits-nothing-yields-nothing)
- [chunkmd — a passage is placed where it begins](#chunkmd--a-passage-is-placed-where-it-begins)
- [chunkmd — citation material without a marker on it](#chunkmd--citation-material-without-a-marker-on-it)
- [avplan — the encoder is found rather than configured](#avplan--the-encoder-is-found-rather-than-configured)
- [avplan — the command line names the destination](#avplan--the-command-line-names-the-destination)
- [avplan — a failed probe is an answer](#avplan--a-failed-probe-is-an-answer)
- [avplan — a path is taken apart properly](#avplan--a-path-is-taken-apart-properly)
- [pdfgutter — the page is the crop box, consistently](#pdfgutter--the-page-is-the-crop-box-consistently)
- [pdfgutter — a page tree that does not add up is refused](#pdfgutter--a-page-tree-that-does-not-add-up-is-refused)
- [pdfgutter — a sheet on its side is cut the way it is read](#pdfgutter--a-sheet-on-its-side-is-cut-the-way-it-is-read)
- [pdfgutter — an unreadable file is an answer, not a false](#pdfgutter--an-unreadable-file-is-an-answer-not-a-false)
- [bookmeta — a cover is what its bytes say it is](#bookmeta--a-cover-is-what-its-bytes-say-it-is)
- [Every pattern — two regex engines, two Unicode tables](#every-pattern--two-regex-engines-two-unicode-tables)

## cuenorm — settings after the end time are the format, not a malformation

A timestamp line may carry positioning after it:

```
00:00:01.000 --> 00:00:03.000 line:0 position:20%
```

Each half of the line is reduced to the one token that could be a timestamp
rather than being read whole, so the trailing settings are ignored and the cue
reads. In WebVTT that is not a malformation but the format, and reading it any
other way would need a second parser differing in a single line. The rule can
only ever recover a cue that a stricter reader discarded; there is no input
where it changes a cue that already read.

## pdftriage — the letter table is the newest one available

Whether a text layer is worth reading turns on what share of it is letters, so
the answer depends on which code points count as letters — and that table
moves with every Unicode release. A book typeset in Cyrillic Extended-C reads
as entirely letters against a current table and as entirely non-letters
against an older one, which is the difference between "keep this text" and
"spend an hour re-OCRing a page that was already fine".

Erring toward not re-OCRing a book whose text is healthy is the right
direction for this gate to err in, so the table is the newest one available
rather than the most widely deployed.

## mdscrub — nothing manufactures a page boundary

Page boundaries are not touched unless `--pages` asks for them, and even then
text with no separators and no form feeds is returned unchanged. Nothing wraps
un-paginated text in a `<!-- Page 1 -->` marker so that a later stage may
assume one exists.

A cleanup filter has to be safe to run twice, and manufacturing a marker is
not: a second pass wraps the first pass's marker in another one. Removing a
tag can also leave a dash rule flush against a newline, so a pagination pass
running in the same sweep would find a boundary on the second run that was not
there on the first. The assumption that a marker exists belongs to a converter
that knows it just read a PDF, not to a filter handed markdown of unknown
provenance — a form feed in a code listing is a control character, and a dash
rule in a quoted document is not a page break.

The chunker already handles a document with no page markers by emitting chunks
with no page number, which is the honest answer.

## chunkmd — a page number saturates rather than widening every record

The page field is 32 bits and saturates rather than wrapping. A document with
four billion pages does not exist, and a 64-bit field would make every chunk
record wider to carry a value nothing can produce. Saturating rather than
wrapping at least makes the wrong answer obviously wrong.

This applies only where a page number becomes a *number*. Where one is copied
into text — a page anchor in the body of a document — it is carried through
exactly as written, padding and all.

## chunkmd — a budget that permits nothing yields nothing

`chunk` with a hard maximum of zero returns no chunks rather than failing: no
piece could satisfy the limit, and a filter that returns nothing for a budget
that permits nothing is the more useful of the two answers. The command
rejects it before reading any input, so this is reachable only from the
library.

## chunkmd — a passage is placed where it begins

The chapter a passage carries is the nearest heading above the point the
passage *starts*, found by byte offset into the source.

The heading level that divides a document is the shallowest one used more
than once, not simply the shallowest one present: a converted EPUB carries the
book's title as a lone `#` above sixty-two `##` chapters, and a level used
once is a title rather than a chapter map.

The offset has to be the passage's own. A document with no page markers is a
single page beginning at zero, so asking where the *page* started puts every
chunk in the book under whichever heading came first in the file — which also
takes the role classification down with it, since that reads the chapter
title. Measured over a corpus of 5,386 books: every one of the 1,232
unpaginated books with twenty or more chunks carried exactly one chapter title
from cover to cover.

## chunkmd — citation material without a marker on it

A chunk is flagged `footnote_citations` when it carries citations, and two
shapes of that arrive with nothing announcing them.

**A back-reference marker.** An arrow pointing back to where the note was
called, sometimes a link and sometimes not, wrapped in escaped brackets:

```
\[←788 \]
\[ [←193] (index_split_011.xhtml#citation-193) \]
```

(the second written with spaces here so it is not read as a link)

It is what a common EPUB conversion leaves behind. Ten books of one corpus use
it, including both volumes of *The Decline of the West*, whose endnotes run to
830 entries of `Krumbacher, op. cit., pp. 969-90` — every one of those books
extracted zero citations while the marker went unread. `citefind` strips the
same marker when it cuts a page of notes into notes.

**A run of entries with nothing marking them** — no heading over them, no
numbers beside them:

```
Block, N. 1981. Psychologism and behaviorism. Philosophical Review 90:5-43.
Dennett, D. C. 1987. The Intentional Stance. MIT Press.
Geach, P. 1957. Mental Acts. Routledge and Kegan Paul.
```

That is how a reader or a companion sets the further-reading list at the end
of each chapter, indented under the last paragraph of the essay. The opening
may be a bullet, because a converted bibliography is frequently a bulleted
list and the bullet sits exactly where the surname would.

An index line opens identically — `Newton, Isaac, 5, 93, 141`. What tells them
apart is that a reference names a work: five words of three letters or more on
the entry's own line, and not two page numbers written as links. Over 15,925
unflagged body chunks sampled from a corpus this fires on 2.6%, and seven of
eight sampled were real reference lists.

## avplan — the encoder is found rather than configured

`ffmpeg -encoders` is asked what is installed and the best of them is taken,
hardware first. No environment variable names it.

An environment variable naming the encoder is per-host configuration, and a
filter that reads configuration is a filter whose answer depends on where it
ran — the one thing the [contract](contract.md) rules out. Detection reaches
the same answer on every machine the variable was set correctly on and a
better one everywhere else, where an unset variable would otherwise build an
NVENC command line for a machine with no NVIDIA card. `--encoder` names one
explicitly for anyone who needs it.

## avplan — the command line names the destination

The command writes where it says, rather than to a temporary that something
else is expected to rename. `avplan` does not run the command, so it cannot do
the rename half of that pattern, and a command line writing to a temporary
nobody moves is worse than one writing to the destination. Anything running
these in a loop can name a temporary of its own with `--output`.

## avplan — a failed probe is an answer

When `ffprobe` cannot read a file, the answer is exit 2, could not tell — not
a guess from metadata recorded elsewhere. Inventing a codec is how a
compatible file gets re-encoded for forty minutes.

## avplan — a path is taken apart properly

`film.mp4/.` is a directory and has no extension; `/talk.webm` becomes
`/talk.h264.mp4` rather than `//talk.h264.mp4`; `x.mp4//` becomes
`x.h264.mp4`; and a destination beginning with `-` is written `./-name`, which
is the same file and cannot be read as an option.

Byte arithmetic on a path gets every one of those wrong — a double leading
separator is left to the system and some systems read it as something else,
`x.m.h264.mp4` names a file nobody asked for, a directory reported as a
playable MP4 is a wrong answer outright, and a command line whose destination
parses as an option fails on a file that was otherwise fine.

## pdfgutter — the page is the crop box, consistently

The crop box is what a reader shows, so it is what everything measures: the
aspect ratio, the render (`pdftoppm -cropbox`), and where the cut goes. A page
with no crop box is its media box, which is what the spec says and what most
files are.

Taking the ratio from the media box and the ink from the crop box measures the
shape of one rectangle and the contents of another. Most scanned books carry
both, so the bands a gutter is looked for in fall somewhere other than the
middle of what was rendered, and a real gutter is missed. Worse, a wide sheet
cropped to show one of its two pages still measures wide, so it is split
again — down the middle of a page rather than between two.

Because two rasterisers antialias differently, a pixel on the edge of a
letterform can land either side of the ink threshold in one and not the other.
What can be pinned is everything that decides: the aspect ratio, which pages
get sampled, the reduction from pixels to a column profile, whether a profile
has a gutter, and the verdict a set of ratios and gutters adds up to.

## pdfgutter — a page tree that does not add up is refused

The page tree is walked directly, and a walk is stricter than a renderer: a
node missing its `/Type`, a `/Type` written as a reference, a kid that is a
dictionary rather than a reference to one — each is a page a strict walk does
not see and a reader does. So the number of pages found is checked against the
number the file says it has, and a document where they disagree is refused
with exit 2.

Splitting replaces the page tree, and a page left out of the new tree is a
page deleted. A file this does not fully understand is one it must not
rewrite. Refusing costs a document that could perhaps have been handled; not
refusing costs pages, silently, in the copy you keep.

## pdfgutter — a sheet on its side is cut the way it is read

The media box is turned by whatever `/Rotate` asks for before it is measured,
and a quarter turn is cut across rather than down, in the order a reader would
read the halves.

The rotation is in the file precisely so that readers honour it. Ignoring it
means a sheet stored tall and displayed wide is never a split candidate, and
one stored wide and displayed tall is cut down an axis with nothing down the
middle of it — leaving a page with the top of one page and the bottom of
another on it, which nothing downstream can recover from. Documents with no
rotation, which is nearly all of them, are unaffected.

## pdfgutter — an unreadable file is an answer, not a false

Exit 2 for a file it cannot judge, exit 74 for one it cannot read, and exit 1
only for a document it looked at and found to be one page per sheet. A corrupt
file, an encrypted one and an ordinary single-page book are three different
answers, and the [contract](contract.md) gives them three different numbers. A
caller wanting them collapsed treats 1 and 2 alike; a caller wanting to know
which files it failed on can now find out.

## bookmeta — a cover is what its bytes say it is

An EPUB's manifest gives every image a media type, and the cover's is not
used: the type comes from the image's first bytes, and a disagreement is a
note. Elsewhere `bookmeta` reports what the file says rather than what is
true, and this is the one place it does not.

The difference is that a title nobody believes is still a title, while a
media type that is wrong is an image that does not open. A caller handed
`image/png` for a JPEG writes `cover.png`, serves it as PNG, or hands it to a
decoder that refuses it. An image can be replaced by a converter or by hand
without its manifest entry changing, and the note keeps what the file said,
so nothing is lost by correcting it.

The same reasoning goes the other way: bytes that are not an image are not a
cover, whatever the manifest calls them.

## Every pattern — two regex engines, two Unicode tables

Patterns here spell their character classes explicitly rather than leaning on
`\s`, `\w` or `\b`, whose meanings differ between regex engines. What is left
is `\d`, which disagrees across Unicode versions on decimal digits added
between them — Garay, added in Unicode 16, is the current example — and
case-insensitive matching, which under simple case folding leaves Turkish
dotless `ı` and dotted `İ` alone rather than folding them onto `i`.

So a chapter heading reading `Bıblıography` is classified as body text, and a
page marker written in Garay digits reads. Brute-forcing every code point
below `U+11000`: `\d` differs on ten, case folding on three.
