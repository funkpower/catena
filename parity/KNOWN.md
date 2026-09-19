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

## chunkmd, role and footnote patterns — `\s` excludes four control characters

**Python:** `\s` in a `str` pattern matches the Unicode `White_Space`
property *and* `U+001C`–`U+001F`, the four ASCII information separators.

**Rust:** `regex` and `fancy-regex` match the `White_Space` property only, so
those four characters are not whitespace to a ported pattern.

**Why this is acceptable:** every one of those codepoints is deleted by
`mdscrub` before a chunker sees it — they fall inside the control-character
class that a text column rejects. For the pattern to behave differently, a
document would have to reach `chunkmd` without having been cleaned, and carry
an information separator in exactly the position where a pattern expects
whitespace.

Note that this does **not** apply to the sentence and paragraph splitters in
`textkit`, which match Python's definition exactly — see
`textkit::segment::is_space`. Those decide chunk boundaries, so the four
characters are handled there rather than argued about.

**Measured:** zero differences over the parity corpus; the sample in
`parity/src/sample.rs` includes `U+001C` deliberately and the token counts
agree over all ten thousand lines.
