# colophon

The record a book prints in its own front matter.

Part of [catena](../../README.md).

## What it does

A book states what it is twice before the text starts. The title page carries
the title as the publisher set it and the name of whoever wrote it. The
copyright page carries the imprint, the year, the ISBN, and — in most
English-language books printed since the 1970s — the cataloguing block, which
is a catalogue record a librarian made from the manuscript, subject headings
and all.

So this reads the pages rather than the file. A PDF whose embedded title is
`Untitled` and whose filename says `Oxford` opens on *The First World War: A
Very Short Introduction*, by Michael Howard, and that is what comes back.

Every field names the page it was read off. Nothing is inferred: a scan that
begins at the preface printed no title page, and the record of that is an
empty one with every field listed in `unsettled`.

## A shelf of your own

A cataloguing block states subjects the way a national library does:

```text
Political science – Early works to 1800
English literature—Middle English, 1100–1500—History and criticism
```

Nobody browses by those. `--shelf` takes the closed list you actually file by
and returns the headings from it that the book's own subjects hit:

```sh
colophon --shelf shelf.txt book.md
```

```text
# one heading per line; `= terms` gives the words that match it,
# and a heading with no terms matches its own name.
Byzantine History = byzantium, byzantine, constantinople
Political Science = political science, politics
Ethics
```

The longest term wins, so `Ancient Philosophy` beats `Philosophy` and a period
of thought does not collapse into its parent. Matching is on whole words after
punctuation is folded away, because a block separates with em dashes, en
dashes, full stops and hyphens, and OCR turns any of those into any other.

Headings that match nothing come back under `unshelved`, kept as printed. A
subject quietly dropped reads exactly like a book that printed none.

## When you would use it

- **Cataloguing scans and secondhand files**, where the embedded metadata is
  whatever the last tool to touch the file happened to write.
- **Checking a catalogue against the books themselves.** A record that
  disagrees with the book's own title page is a record worth looking at.
- **Recovering an ISBN** that only exists in print, which turns a book with
  no identifier into one [`bibfetch`](../bibfetch/README.md) can look up.
- **Getting subject headings** that nobody had to invent, from the
  cataloguing block of the books that print one.

## Install

```sh
cargo install --git https://git.pumpin.online/funkpower/catena colophon
```

## Example

```
$ colophon first-world-war.md
{"v":1,"path":"first-world-war.md","title":"The First World War","subtitle":"A Very Short Introduction","authors":["Michael Howard"],"editors":[],"isbns":["9780192803535"],"publisher":"Oxford University Press","place":"Oxford","year":2002,"subjects":[],"read_from":{"authors":"title_page","isbns":"copyright_page","publisher":"copyright_page","subtitle":"title_page","title":"title_page","year":"copyright_page"},"unsettled":["subjects"],"pages_read":16}
```

A library at once, and the books that say nothing about themselves:

```sh
colophon library/*.md | jq -r 'select(.title == null) | .path'
```

An ISBN that only exists in print, looked up:

```sh
colophon book.md | jq -r '.isbns[]' | idnorm | bibfetch --polite you@example.org
```

## What it will not read

Two pages in a book's front matter look like a title page and are not, and
both of them are how a catalogue ends up naming the wrong book. They are
found and refused.

The **series advertisement** lists other titles in the same series in the
same shape a title page uses — a work in capitals, a person beside it —
sometimes for four pages, with only the first saying what it is. The
**uniform title** in a translation's cataloguing block is the original
title, printed in brackets above the real one as a filing heading.

## Asking a model about the rest

Roughly a third of books print nothing this can read: an extraction that
starts at chapter one has no front matter in it at all. `--prompt` writes a
question about exactly the fields left `unsettled`, and `--merge` folds the
answer back in.

```sh
colophon --prompt book.md | ollama run qwen3:8b > reply.json
colophon --merge reply.json book.md
```

Nothing is sent from here: the question goes to stdout, and the model is
yours. What a model answers is marked `model` in `read_from`, so a record
always says which of its fields were read off a page and which were
answered.

The pages the question shows have had the advertising taken out of them
first. Handing a model a series list is how a book becomes catalogued as
whichever of two hundred other titles the model's eye fell on.

## Where this stops

It reports what the book printed, not what is true. `THE HUMAN CONDITION` is
reported in the capitals it was set in, and straightening that is
[`bibclean`](../bibclean/README.md)'s job. What a file's own metadata claims
is [`bookmeta`](../bookmeta/README.md)'s. What a register says about the
ISBN is [`bibfetch`](../bibfetch/README.md)'s. Which series a book belongs to
is [`seriesof`](../seriesof/README.md)'s.

## Options

| Option | Effect |
|---|---|
| `--pages <COUNT>` | How many pages in from the front to read. Default `16` |
| `--prompt` | Write the question to put to a model about what the pages left unsettled |
| `--merge <FILE>` | Fill in from a model's reply and write the completed record |
| `--quiet` | Say nothing; the exit code is the answer |
| `--man` | Write this page as a manual page, in roff |

## Exit codes

| Code | Meaning |
|---|---|
| `0` | The book printed something about itself |
| `1` | It printed nothing this could read |
| `65` | The input was not readable as text |
| `74` | The file could not be read |
