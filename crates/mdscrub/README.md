# mdscrub

Converted markdown in, clean markdown out.

Part of [catena](../../README.md).

## What it does

Takes the markdown that falls out of an EPUB or a PDF converter and removes
everything that means nothing: styling tags that survived, control characters
a text column will reject, headings wrapped in links to their own anchors,
page anchors that should be page markers.

Safe to run twice — the second run changes nothing.

## When you would use it

- **Between a converter and a chunker.** Converted markdown is readable but
  not clean, and every piece of leftover markup ends up in an embedding, in a
  full-text index, and in every result a reader sees.
- **Backfilling text you already stored.** Running it over text that was
  ingested before a cleaning step existed is safe, because it is idempotent.
- **Getting rid of the things that break a database write.** A null byte in
  extracted text is rejected outright by most text columns; the rest of the
  C0 controls are storable and are extraction garbage.

## Install

```sh
cargo install --git https://git.pumpin.online/funkpower/catena mdscrub
```

## Example

A chapter as a converter leaves it:

```
$ cat chapter.md
# [<span id="page41"></span>III. The Second Estate](#ch3) {.unnumbered}

The obligation ran <em>both ways</em>, and contemporaries said so plainly
whenever they were asked.<a href="#n4"><sup>4</sup></a> A lord who could not
protect was a lord who could not command…

<span id="page42"></span>
…
## [Notes]{.section-title}

5. Maitland, *Domesday Book and Beyond* (Cambridge, 1897), 220&ndash;24.
```

and after:

```
$ mdscrub chapter.md
# III. The Second Estate

<!-- Page 41 -->

The obligation ran both ways, and contemporaries said so plainly
whenever they were asked.4 A lord who could not
protect was a lord who could not command…

<!-- Page 42 -->
…
## Notes

5. Maitland, *Domesday Book and Beyond* (Cambridge, 1897), 220–24.
```

The heading is readable, its page anchor became a marker on its own line, the
superscript note reference became the digit it was, and `&ndash;` became the
dash it names.

In a pipeline:

```sh
pandoc -t commonmark chapter.xhtml | mdscrub | chunkmd > chunks.ndjson
```

## Options

| Option | Effect |
|---|---|
| `--pages` | First turn page separators and form feeds into page markers |

`--pages` is for text straight out of a PDF or DjVu extractor, which marks its
page boundaries with a form feed or a rule of dashes. It is off by default
because only the program that just read a PDF knows that a form feed in the
text is a page break rather than a stray control character — in ordinary
markdown it is a control character, and in a code listing it is part of the
listing.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Done |
| `64` | The command line was wrong |
| `65` | The input was not text |
| `74` | The file could not be read |

## What it does, pass by pass

1. **Headings.** A heading wrapped in a link to its own anchor, an attribute
   block, a bracketed span, bold markers around a title set in capitals — all
   removed, leaving the title. A page anchor inside a heading is moved to a
   marker on the line after it, so the heading keeps its position relative to
   the body text.
2. **Page anchors.** `<span id="page42">` in the body becomes
   `<!-- Page 42 -->`, with the number exactly as it was written.
3. **Character references.** `&lt;sup>107` is decoded first, because a tag
   escaped twice is invisible to any pattern looking for a tag — which is how
   markup survives into text.
4. **Tags.** Styling tags go and their text stays. A tag that stands for a
   line or block boundary — `<br>`, `<p>`, `<li>`, a table cell — becomes a
   space, because deleting it welds the words either side together:
   `APPENDICES<br>OF<br>LIST` would otherwise come out as `APPENDICESOFLIST`,
   which is a word no reader can search for. Genuinely inline tags go to
   nothing, so `claim<sup>16</sup>` stays `claim16`.
5. **Fenced code.** Left alone, so a book that quotes markup keeps its
   examples. A decorative rule of tildes is not a fence.
6. **Control characters.** Removed, tab and newline excepted.
7. **Blank lines.** Runs collapse to one.

HTML comments survive every pass, which is what lets page markers through.

## As a library

```rust
let cleaned = mdscrub::scrub(&markdown);

// And, for text straight out of an extractor:
let cleaned = mdscrub::scrub(&mdscrub::pages::normalize(&raw));
```

The passes are also available one at a time — `mdscrub::html::strip_tags`,
`mdscrub::heading::clean`, `mdscrub::unescape::unescape` — for a caller that
wants only one of them.

## See also

- [`mdsplit`](../mdsplit/README.md) — when the file holds more than one work.
- [`chunkmd`](../chunkmd/README.md) — what to do with the markdown once it is clean.
- [`pdftriage`](../pdftriage/README.md) — whether the text was worth extracting at all.
