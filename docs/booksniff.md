# booksniff

What a file actually is, whatever its name claims.

Back to [the index](README.md).

## What it does

Reads the first few bytes of a file, and an archive's directory at the back,
and says what format it really is. It never reads the whole file, so a
directory of nine-hundred-megabyte scans costs the same as a directory of
anything else.

Writes one [`doc` record](records.md#doc--what-a-file-is) per file, with a
note for anything worth remarking on: a name that disagrees with the bytes, a
file too small to be a book, a name that says "sample".

## When you would use it

- **Before a conversion step**, so a converter is never handed a format it
  cannot read. Most conversion failures are this, and the error a converter
  gives for it is rarely the error you want.
- **Auditing a library you did not build**, where files have been renamed,
  re-saved, and moved between machines that disagreed about extensions.
- **Finding the files that are not books** — the samples, the readmes, the
  cover images that ended up in a folder of PDFs.

A library accumulates files that lie: a DjVu named `.pdf`, a Word document
named `.epub`, an EPUB saved as `.zip`, a single PDF that somebody zipped and
renamed back to `.pdf`.

## Install

```sh
cargo install --git https://git.pumpin.online/funkpower/catena booksniff
```

## Example

Three files, one of which is not what it says it is:

```
$ booksniff --plain book.pdf atlas.pdf chapter.md
pdf	book.pdf
djvu	atlas.pdf
	named .pdf, but the bytes say DjVu
txt	chapter.md
	925 bytes, which is too small to be a book
```

The record form, which is what a pipeline wants:

```
$ booksniff book.pdf
{"v":1,"path":"book.pdf","kind":"pdf","declared_ext":"pdf","bytes":17595,"pages":null,"language":null,"text_layer":null,"needs_ocr":null,"notes":[]}
```

Finding everything in a folder that is not really a PDF:

```sh
booksniff library/*.pdf | jq -r 'select(.kind != "pdf") | "\(.kind)\t\(.path)"'
```

## Options

| Option | Effect |
|---|---|
| `--plain` | Print a line a person can read instead of the record |

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Every file was identified |
| `1` | At least one was not |
| `64` | The command line was wrong |
| `74` | A file could not be read |

A file that cannot be read is reported on stderr and the rest still run, so
one bad file does not stop a sweep.

## What it recognises

| Kind | How |
|---|---|
| `pdf` | `%PDF-` |
| `djvu` | The IFF container tag |
| `ole2` | The compound-file header every binary Word document starts with |
| `epub` | A zip holding `mimetype` and `META-INF/container.xml` |
| `zip-wrapped-pdf` | A zip whose only real member is one PDF |
| `azw3`, `mobi` | A Palm database tagged `BOOKMOBI`, told apart by the format version inside the first record |
| `html` | Markup at the front, after any byte-order mark |
| `txt` | Text that is none of the above |
| `unknown` | Anything else, with a note saying what was seen |

An archive holding several PDFs is a collection, not a wrapped book: choosing
among them would be a guess, and naming one chapter as the whole work is worse
than refusing.

## As a library

```rust
use booksniff::bytes::File;

let file = File::open(std::path::Path::new("atlas.pdf"))?;
let doc = booksniff::sniff(&file, "atlas.pdf");
assert_eq!(doc.kind, corpus_core::Kind::Djvu);
```

`sniff` works over anything implementing `booksniff::bytes::Bytes`, which is
implemented for `&[u8]` as well as for a file — so a caller that already has
the bytes need not write them to disk first.

## See also

- [`pdftriage`](pdftriage.md) — once you know it is a PDF, whether its text is
  worth reading.
- [The records](records.md) — the shape this writes.
