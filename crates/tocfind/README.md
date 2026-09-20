# tocfind

A book's own table of contents.

Part of [catena](../../README.md).

## What it does

A book usually knows its own chapters. An EPUB keeps them in a navigation
document, a PDF in an outline, and both are put there by the publisher — which
makes them better than any heading guessed at from converted text.

Entries come out in reading order, each carrying how deeply it is nested, so a
flat stream still has the shape of the book.

## When you would use it

- **Real chapter titles** for chunks, instead of the extractive ones inferred
  from converted markdown.
- **Splitting a book** at the seams its publisher declared.
- **Judging a conversion**: a book whose contents list forty chapters and
  whose markdown has three was converted badly.

## Install

```sh
cargo install --git https://git.pumpin.online/funkpower/catena tocfind
```

## Example

```
$ tocfind enlightenment.epub --outline
Cover page
Halftitle page
Title page
Copyright page
Contents
1. Introduction: Radical Enlightenment and “Modernity” (1650–1850)
  1.1. Basic Argument and Scope
  1.2. Definitions and Categories
```

As records:

```
$ tocfind enlightenment.epub | head -2
{"v":1,"idx":0,"depth":0,"title":"Cover page","target":"000_ACover.xhtml"}
{"v":1,"idx":1,"depth":0,"title":"Halftitle page","target":"002_FM_halftitle.xhtml"}
```

A PDF's outline carries page numbers instead of documents:

```
$ tocfind history.pdf --outline
Part One  (1)
  I. The Setting  (3)
  II. The Second Estate  (41)
Part Two  (118)
```

Only the real chapters, skipping the front matter every book opens with:

```sh
tocfind book.epub | jq -r 'select(.depth == 0) | .title' | tail -n +5
```

The books whose conversion lost the structure:

```sh
for f in *.epub; do
  declared=$(tocfind "$f" | wc -l)
  found=$(grep -c '^# ' "converted/${f%.epub}.md")
  [ "$declared" -gt $(( found * 3 )) ] && echo "$f: $declared entries, $found headings"
done
```

## Options

| Option | Effect |
|---|---|
| `--outline` | Print an indented list rather than records |
| `--quiet`, `-q` | Say nothing; the exit code is the answer |
| `--man` | Write this page as a manual page, in roff |

## Exit codes

| Code | Meaning |
|---|---|
| `0` | It had a table of contents |
| `1` | It had none |
| `64` | The command line was wrong |
| `65` | Not a format that keeps one |
| `74` | The file could not be read |

Having none is an answer; not being able to have one is not, which is why
those are different codes.

## Where it reads from

| Format | Where |
|---|---|
| EPUB | The NCX, or an EPUB 3 `<nav epub:type="toc">` |
| PDF | The catalogue's outline |

**The NCX first**, even for books that declare themselves EPUB 3. It is what
books actually carry — every one of thirteen real books sampled had one and
none had a nav document — and its labels are plain text where a nav
document's are markup.

From a nav document only the contents are read, never the page list or the
landmarks, and markup inside a title is not part of the title.

A PDF outline entry resolves to a page where its destination names one
directly. A named destination is not chased: that is a second index, and an
entry without a page is still an entry.

## As a library

```rust
match tocfind::read(std::path::Path::new("book.epub")) {
    Ok(entries) => for entry in entries {
        println!("{}{}", "  ".repeat(entry.depth as usize), entry.title);
    },
    Err(why) => eprintln!("no contents: {why}"),
}
```

`tocfind::epub::from_ncx` and `from_nav` read a navigation document already in
hand.

## See also

- [`mdsplit`](../mdsplit/README.md) — splitting a collection by its seams.
- [`chunkmd`](../chunkmd/README.md) — which infers a chapter when the book
  does not say.
- [`bookmeta`](../bookmeta/README.md) — the rest of what a file carries.
