# mdsplit

Splits one markdown file into the works inside it.

Part of [catena](../../README.md).

## What it does

Finds the seams in a collection and cuts along them, writing one record per
work — or one file per work.

A document that is simply one work is not a collection, and says so rather
than pretending to split.

## When you would use it

- **Before chunking a collected edition.** Forty books in one file, indexed
  whole, means a query for any of them retrieves a passage that could be from
  any other, and the title on the result names the collection rather than the
  work.
- **Making a collection citable.** Each section gets its own identifier, so a
  passage can be attributed to the work it is actually from.
- **Scripture**, which needs its own rule and gets one.

## Install

```sh
cargo install --git https://git.pumpin.online/funkpower/catena mdsplit
```

## Example

A collection of twelve works:

```
$ mdsplit works.md
{"v":1,"idx":0,"title":"Work 1","slug":"work-1","text":"# Work 1\n\nThe obligation ran both ways…"}
{"v":1,"idx":1,"title":"Work 2","slug":"work-2","text":"# Work 2\n\nThe obligation ran both ways…"}
{"v":1,"idx":2,"title":"Work 3","slug":"work-3","text":"# Work 3\n\nThe obligation ran both ways…"}
```

Scripture, grouped by book rather than by chapter:

```
$ mdsplit bible.md | jq -r '"\(.idx)\t\(.title)\t\(.text | length) chars"'
0	Genesis	29929 chars
1	Exodus	29914 chars
2	Leviticus	29959 chars
3	Numbers	29929 chars
4	Deuteronomy	29989 chars
5	Joshua	29914 chars
```

Ninety chapter headings became six books. Splitting on the headings
themselves would have given ninety fragments, none of them a work.

One work is not a collection:

```
$ mdsplit politics.md
mdsplit: one heading level gave 1 sections (only 1 sections, fewer than 5); the next gave 0 (only 0 sections, fewer than 5)
$ echo $?
1
```

Writing files instead of records:

```
$ mdsplit --into sections/ bible.md
sections/000-genesis.md
sections/001-exodus.md
sections/002-leviticus.md
…
```

In a pipeline:

```sh
mdsplit --into sections/ complete-works.md
for f in sections/*.md; do mdscrub "$f" | chunkmd > "chunks/$(basename "$f" .md).ndjson"; done
```

## Options

| Option | Effect |
|---|---|
| `--into <DIR>` | Write one file per section instead of records |

Files are named for their position and their title, so a directory listing is
in reading order.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | It split |
| `1` | It is not a collection |
| `64` | The command line was wrong |
| `65` | The input was not text |
| `74` | The file could not be read |

Exit `1` is an answer, not a failure — most documents are one work, and the
diagnostic on stderr says what each heading level produced.

## How it decides

Three candidates in order — scripture, top-level headings, the level below —
and the first plausible one wins.

A candidate is plausible unless:

- it found **fewer than five sections**, which means the document has no
  collection structure;
- it found **more than fifteen hundred**, which means it found structure that
  is not there;
- the **typical section is under 1,250 characters**, which means it is cutting
  at the line level rather than at the work level.

The median is used rather than the average, because one enormous work among
sixty ordinary ones is a real shape that an average would hide.

Otherwise it is permissive on purpose. Collections vary wildly: one giant work
among sixty, five hundred short homilies, a hundred and eighty distinct works.

### Two details worth knowing

**Whatever sits above the first heading** — a contents page, an
acknowledgement — goes onto the first section rather than being dropped. It is
usually the only place a collection describes itself.

**A heading with nothing under it folds into the next one.** Anthologies emit
them in pairs — `# CHAPTER I` immediately followed by `# THE WORK` — and the
second owns the content. The stub's own text is kept, so the chapter number is
still searchable as context.

## Scripture

Recognised when a document has at least thirty top-level headings and more
than forty per cent of them name a book.

The book names are general reference data rather than anything local:
Septuagint primaries with the English and Protestant spellings as variants,
because an edition uses one convention throughout and both have to resolve to
the same book. `1 Samuel` and `1 Kingdoms` are one section. So are
`The First Book of the Kingdoms`, `Annotations for Job` (which belongs with
Job), `Tobit†`, and `Susanna` (which belongs with Daniel).

Books come out in canonical order whatever order the edition printed them in,
and a heading that names no book — a preface, a table of weights — becomes its
own section and sorts after the canon.

An edition that interrupts itself still gets one section per book: a prologue
between a book's introduction and its first chapter would otherwise make that
book appear twice, as two separate works.

## As a library

```rust
match mdsplit::split(&markdown) {
    Ok(sections) => for section in sections {
        println!("{} {}", section.idx, section.title);
    },
    Err(why) => eprintln!("not a collection: {why}"),
}
```

`mdsplit::bible::classify` is usable on its own, for anything that needs to
know which book a heading names.

## See also

- [`mdscrub`](../mdscrub/README.md) — what to run on each section afterwards.
- [`chunkmd`](../chunkmd/README.md) — turning each work into passages.
