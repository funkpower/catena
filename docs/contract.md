# What these programs promise

Eight things hold for every program here. Together they are what lets a person
put any of them in a pipeline without first reading how it works.

Part of [catena](../README.md).

## Each one does a single job

No program here has a mode switch that changes what it is. Where two jobs
would otherwise share a name, there are two programs — which is why
[`cuenorm`](../crates/cuenorm/README.md), [`cuepack`](../crates/cuepack/README.md) and [`cueseg`](../crates/cueseg/README.md) are
separate commands rather than one command with a flag.

## Input on stdin, results on stdout, diagnostics on stderr

Output and commentary never share a stream, so a result can always be piped
somewhere without filtering anything out of it.

Almost all of them read stdin or a path. Where one takes a path only, it says
so in `--help` and says why: [`pdftriage`](../crates/pdftriage/README.md) reads two specific
page ranges rather than the whole file, which a stream cannot offer.

## Anything list-shaped is NDJSON

One [record](records.md) per line, flushed as it is produced — so a slow stage
downstream sees each record as it appears rather than when the producer
finishes.

Readers are strict, and each strictness prevents a failure that would
otherwise be silent. A record from a version the reader does not know is
refused rather than parsed into whatever lines up. A bad line ends the stream
instead of being skipped, because it means the two ends disagree about the
format. Lines are bounded, so a program pointed at a binary file fails instead
of allocating it. A byte-order mark and blank lines are ignored, so a stream
that was concatenated or written on Windows still reads.

## Exit codes carry the answer

| Code | Meaning |
|---|---|
| `0` | Yes, or done |
| `1` | No |
| `2` | Could not tell |
| `64` | The command line was wrong |
| `65` | The input was not what the program can work on |
| `69` | A program it needs is not installed |
| `70` | Its own fault |
| `74` | Reading or writing failed |

Failures start where `sysexits.h` puts them, so "no" and "broke" are never the
same number. That is what makes this work:

```sh
for f in *.pdf; do pdftriage --quiet "$f" && ocrmypdf "$f" "ocr/$f"; done
```

A consumer closing the pipe is not a failure. `… | head -1` exits `0`, the
same as any other filter.

## Nothing reaches outside the process

No network, no database, no configuration file, no state kept between runs.
Five exceptions, each declared in `--help`:

- [`emb`](../crates/emb/README.md) makes an HTTP call, which is the whole of what it does.
- [`bibfetch`](../crates/bibfetch/README.md) asks a register about an identifier, likewise.
- [`pdftriage`](../crates/pdftriage/README.md) runs `pdftotext` and `pdfinfo`.
- [`pdfgutter`](../crates/pdfgutter/README.md) runs `pdftoppm` to look at a page.
- [`avplan`](../crates/avplan/README.md) runs `ffprobe` to read a file's streams, and
  `ffmpeg` to ask which encoders the machine has.

The three that call another program exit `69` when it is missing, rather than
guessing at what it would have said.

## Nothing is assumed about where it is running

No hostnames, no paths, no private vocabularies are built in. General
reference data is — ISBN check digits, the HTML character references, the
names of well-known publishers — and anything specific to one collection is
supplied in a file rather than compiled in.

## The same input gives the same output

On every run and on every machine. No dependence on hash ordering, no reading
of the clock or the locale. Numbers that JSON cannot represent are refused
where they are produced rather than written as `null` and lost somewhere
downstream.

## `--json` is the stable form

Where a program prints something for a person to read, `--json` prints the
same answer as a record, and that record does not change shape without a major
version. Programs whose only sensible output is a stream of records write that
stream and have no `--json` to add.

## Every one writes its own manual page

`--man` writes the program's manual page, in roff, to stdout:

```sh
pdfgutter --man | gzip > /usr/local/share/man/man1/pdfgutter.1.gz
```

It is generated from the same definition `--help` is, so the two can never
come to say different things.

See also [the recipes](recipes.md), for what these rules buy when the
tools are put in a row.
