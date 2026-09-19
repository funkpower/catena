# pdftriage

Whether a PDF's text layer is worth reading, or whether it has to be OCRed.

Part of [catena](../../README.md).

## What it does

Extracts a sample of text from the middle of a PDF and judges it: healthy,
garbled, sparse, reflected-font, or absent — and therefore whether OCR is
worth its cost.

It never runs OCR, never writes to the file, and reads two page ranges rather
than the whole document.

## When you would use it

- **Deciding what to OCR.** OCR is the expensive step in any ingest: minutes
  of GPU time a page, and a result that is worse than a text layer the file
  already had. This is the question worth answering carefully before spending
  that.
- **Finding books that are already broken in an index.** A garbled layer
  extracts as characters, not words, so it indexes silently and returns
  nothing anybody searched for.
- **Recovering a layer that only looks dead.** Some PDFs store every character
  code reflected about a fixed point, so the text extracts as punctuation
  soup — `×²¬®±¼«½¬·±²` is "Introduction". That is recoverable in full, which
  is strictly better than an OCR pass that could only approximate it.

## Install

```sh
cargo install --git https://git.pumpin.online/funkpower/catena pdftriage
```

Needs `pdftotext` and `pdfinfo` on `PATH`. Both ship with poppler. Without
either, the tool exits `69` and says so rather than guessing.

## Example

A book whose text is fine:

```
$ pdftriage book.pdf
healthy, no OCR needed
  1805 characters on the opening pages
  1805 characters over 5 sample pages, ascii text score 1.00
  letters 0.98, latin-1 0.00
  361 characters a page
  english function words 0.44
$ echo $?
1
```

A scan with no text at all:

```
$ pdftriage scan.pdf
absent, needs OCR
  0 characters on the opening pages
$ echo $?
0
```

The exit code is the answer, so a shell can act on it directly:

```sh
for f in *.pdf; do pdftriage --quiet "$f" && ocrmypdf "$f" "ocr/$f"; done
```

The record form:

```
$ pdftriage --json book.pdf
{"v":1,"path":"book.pdf","kind":"pdf","declared_ext":"pdf","bytes":17595,"pages":20,"language":null,"text_layer":"healthy","needs_ocr":false,"notes":["1805 characters on the opening pages","1805 characters over 5 sample pages, ascii text score 1.00","letters 0.98, latin-1 0.00","361 characters a page","english function words 0.44"]}
```

## Options

| Option | Effect |
|---|---|
| `--json` | Emit a [`doc` record](../../docs/records.md#doc--what-a-file-is) instead of a readable verdict |
| `-q`, `--quiet` | Write no verdict; answer with the exit code alone. Diagnostics still go to stderr, because a silent failure is worse than a quiet one |

## Exit codes

| Code | Meaning |
|---|---|
| `0` | The layer needs OCR |
| `1` | It does not |
| `2` | Could not tell — the extractor could not read the file |
| `64` | The command line was wrong |
| `69` | `pdftotext` or `pdfinfo` is not installed |
| `74` | The file could not be read |

## The verdicts

| Verdict | What it means | OCR? |
|---|---|---|
| `healthy` | Extractable and readable as it stands | no |
| `reflected-font` | Permuted by a non-standard font encoding, and recoverable without OCR once the permutation is undone | no |
| `garbled` | Present but mojibake — extraction yields characters, not words | yes |
| `sparse` | Present but too thin to be the whole page: a scan whose layer holds only the page numbers and the running heads | yes |
| `absent` | No text at all | yes |

The sample comes from the middle of the document, not the front. Front matter
is often a bare scan in a book whose body carries good text, and often cleanly
typeset boilerplate in a book whose body is damaged — judging a book by its
title page gets both wrong.

The thresholds are measurements rather than guesses. Each one sits in the
middle of a gap observed across a few thousand real PDFs, and the reason is
recorded beside it in the source.

## As a library

The judging is a pure function over already-extracted text, so it can be
tested and used without a PDF anywhere in sight:

```rust
use pdftriage::{Limits, Sample};

let found = pdftriage::triage(
    &Sample { opening, middle, middle_pages: 5, pages: Some(412) },
    &Limits::default(),
);
println!("{:?} needs_ocr={}", found.layer, found.needs_ocr);
```

Running `pdftotext` is the command's job, not the library's.

## See also

- [`booksniff`](../booksniff/README.md) — whether it is a PDF at all.
- [`mdscrub`](../mdscrub/README.md) — cleaning up the text once it has been extracted.
- [The records](../../docs/records.md) — the shape `--json` writes.
