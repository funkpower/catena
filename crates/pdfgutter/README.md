# pdfgutter

Finds books scanned two pages to a sheet, and splits them back into pages.

Part of [catena](../../README.md).

## What it does

A book held open under a camera gives one wide image per shot, with two pages
on it. Everything that reads it afterwards runs the two columns together: the
lines interleave, the reading order comes out scrambled, and what lands in the
index looks like prose and is not.

`pdfgutter` says whether a PDF is that, and splits it when it is. The split
changes the page boxes and nothing else — nothing is re-rendered, the text and
the images are the ones that were there, and a 250-sheet book becomes a
500-page book at almost no cost in size.

## When you would use it

- **Before OCR.** Layout models read a spread as one page and produce jumbled
  text, or none.
- **Before chunking.** A passage that alternates lines between two pages is
  unusable and does not look it.
- **Sorting a scanned library.** Most books are fine; the answer tells you
  which ones are not.

## Install

```sh
cargo install --git https://git.pumpin.online/funkpower/catena pdfgutter
```

Needs `pdftoppm`, from poppler. Nothing is split without first being looked
at, so it is needed either way.

## Example

A book photographed two pages at a time:

```
$ pdfgutter scan.pdf
two pages to a sheet
  12 pages, median ratio 1.40, gutter on 100% of the wide ones
```

An ordinary one:

```
$ pdfgutter book.pdf
one page to a sheet
  12 pages, median ratio 0.68, gutter on 0% of the wide ones
$ echo $?
1
```

Splitting it:

```
$ pdfgutter --into split.pdf scan.pdf
two pages to a sheet
  12 pages, median ratio 1.40, gutter on 100% of the wide ones
20 pages written to split.pdf
```

Twelve sheets became twenty pages: eight spreads split in two, and the four
covers and end leaves, which were single pages already, left alone.

What it looked at:

```
$ pdfgutter --json scan.pdf | jq -c '.sampled[:3][]'
{"number":3,"ratio":1.4,"gutter":true}
{"number":4,"ratio":1.4,"gutter":true}
{"number":5,"ratio":1.4,"gutter":true}
```

Over a directory, splitting only what needs it:

```sh
for f in *.pdf; do
  pdfgutter --quiet "$f" && pdfgutter --quiet --into "split/$f" "$f" || cp "$f" "split/$f"
done
```

## Options

| Option | Effect |
|---|---|
| `--into <FILE>` | Write the split document here |
| `--json` | Emit a record — every page looked at, and what it measured |
| `--quiet`, `-q` | Say nothing; the exit code is the answer |
| `--min-ratio <RATIO>` | How wide a sheet must be to hold two pages. Default `1.15` |
| `--gutter-fraction <FRACTION>` | What share of wide sheets must show a gutter. Default `0.6` |
| `--man` | Write this page as a manual page, in roff |

`--into` writes whatever the verdict was, so a loop over a directory does not
have to branch. A document with no sheet wide enough to cut is not altered at
all — every page comes through as it was.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Two pages to a sheet |
| `1` | One page to a sheet |
| `2` | It could not be judged — encrypted, no pages, or a page tree that does not add up |
| `64` | The command line was wrong |
| `69` | `pdftoppm` is not installed |
| `74` | The file could not be read or written |

## How it decides

**Shape is not enough.** A foldout, a plate, a page set in one narrow centred
column are all genuinely wide, and splitting one down the middle destroys it.
So the shape of the ink is checked as well.

Up to eight pages are sampled, evenly spaced and away from the ends — covers
and end matter are single pages even in a book otherwise shot two at a time,
and sampling them would argue against a document that plainly is a spread.

Each sampled page that is wide enough to hold two pages gets rendered small
and read as a column-by-column measure of how much ink is on it. A gutter is:

- a **blank column** somewhere in the middle eight per cent of the width,
- with **real text** in the third of the page to its left,
- and **real text** in the third to its right.

The outer tenth of the sheet is ignored on both sides, because that is where
a scan carries the shadow of the binding and of the table the book sat on.

A document is two-up when the **middle** sampled page is wide enough and
**at least three in five** of the wide ones showed a gutter. The middle rather
than the average throughout, so that one plate or one blank leaf does not move
the answer for a whole book.

### The split

Each wide sheet becomes two pages pointing at the same content, differing only
in how much of it they show. A sheet the file asks to be turned on its side is
cut across rather than down, and the halves come out in the order a reader
would read them. An annotation is kept on the half it sits on rather than
copied onto both.

Splitting is safe to repeat: the halves are portrait, so a second pass finds
nothing to do.

Splitting replaces the page tree, which means a page this could not find in
that tree would be a page dropped from the result. So the number of pages
found is checked against the number the file says it has, and a document where
those disagree is refused rather than written short.

### What it measures

The crop box, where a page has one — that is the rectangle a reader shows and
the one that gets rendered. A wide sheet cropped down to a single page is
therefore not a spread, which is what stops a second pass cutting a page in
half. Pages with no crop box, which is most of them, are their media box.

## The record

`--json` writes one of these, whatever the verdict:

| Field | |
|---|---|
| `path` | The file it is about |
| `two_up` | Whether it is two pages to a sheet |
| `pages` | How many pages it has |
| `median_ratio` | The middle ratio of the pages looked at |
| `gutter_fraction` | What share of the wide ones showed a gutter |
| `sampled` | One entry per page looked at: its `number`, its `ratio`, and whether it had a `gutter` — `null` where the page was too narrow to be worth rendering |

It is a shape of its own rather than one of [the records](../../docs/records.md)
the tools pass between them, because nothing downstream consumes it.

## As a library

```rust
let mut pdf = pdfgutter::pdf::Pdf::open(Path::new("scan.pdf"))?;
for index in pdfgutter::sample_indices(pdf.len()) {
    println!("page {} ratio {:.2}", index + 1, pdf.geometry(index).ratio());
}
let pages = pdf.split(pdfgutter::MIN_RATIO);
pdf.save(Path::new("split.pdf"))?;
println!("{pages} pages");
```

`pdfgutter::has_gutter` takes a column ink profile and answers on its own, for
anything that rasterises its own pages. `pdfgutter::ink::profile` turns a
greymap into one.

## See also

- [`pdftriage`](../pdftriage/README.md) — whether the text layer is worth
  keeping, which is the next question after this one.
- [`booksniff`](../booksniff/README.md) — whether the file is a PDF at all.
