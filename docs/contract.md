# The contract

Every program here obeys all of this, or it does not ship. It is worth reading
once, because it is what lets these be used together without reading any of
them.

Back to [the index](README.md).

## One job

A tool that needs a `--mode` flag to change what it *is* is two tools.

## stdin to stdout

Input on stdin or a path argument, result on stdout, diagnostics on stderr
only — never content and chatter on the same stream. A tool that takes a path
instead of a stream says why in its `--help`; [`pdftriage`](pdftriage.md) does,
because it reads two specific page ranges rather than the whole file.

## Streams, not documents

Anything list-shaped is NDJSON: one [record](records.md) per line, flushed as
it is produced, so a slow stage never blocks the pipe behind it.

The reader is strict on purpose. It refuses a record whose version it does not
know, it stops at the first bad line rather than reading past a disagreement
about the format, and it bounds how much one line may cost — a filter pointed
at a binary file should fail, not allocate the file.

## Predicates answer with exit codes

| Code | Meaning |
|---|---|
| `0` | Yes, or done |
| `1` | No |
| `2` | Could not tell |
| `64` | The command line was wrong |
| `65` | The input was not what the tool can work on |
| `69` | A program it needs is not installed |
| `70` | The tool's own fault |
| `74` | Reading or writing failed |

The failure codes start where `sysexits.h` puts them so that "no" and "broke"
are never the same number. That is what makes this work:

```sh
for f in *.pdf; do pdftriage --quiet "$f" && ocrmypdf "$f" "ocr/$f"; done
```

A consumer closing the pipe is not a failure. `… | head -1` exits `0`, like
every other filter.

## No network, no database, no config file, no state

Two exceptions, both declared in `--help`:

- [`emb`](emb.md), whose whole job is an HTTP call.
- [`pdftriage`](pdftriage.md), which shells out to `pdftotext` and `pdfinfo`
  and exits `69` when either is missing.

## No ambient knowledge

No hostnames, no local paths, no private vocabularies compiled in. General
scheme data — ISBN check digits, the HTML character references — may be;
anything local is a `--rules` file.

## Deterministic

Same bytes in, same bytes out, across runs and machines. No hash-order
dependence, no clock, no locale reads. Floats that JSON cannot carry are
refused where they are produced rather than written as `null` and lost.

## `--json` everywhere

The human-readable default is for a person at a terminal; `--json` is the
contract, and it does not change shape without a major version. Tools whose
only sensible output is a stream of records write that stream and have no
`--json` to add.
