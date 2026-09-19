# catena

Small programs that each do one part of turning a pile of files into a
searchable corpus, and hand the next one text on stdout.

A catena is a commentary assembled out of excerpts drawn from many sources and
chained in order, each one carrying the name of who said it and where. That is
both what these programs produce and how they run.

Nothing here needs a database, a GPU, a queue or a config file. Each tool is a
filter: bytes in, records out, diagnostics on stderr, and an exit code that
means something.

## Install

```sh
cargo install --git https://git.pumpin.online/funkpower/catena <tool>
```

Each tool installs by name, so you can take one without taking the rest.

## The tools

| | |
|---|---|
| [`booksniff`](docs/booksniff.md) | What a file actually is, whatever its name claims |
| [`pdftriage`](docs/pdftriage.md) | Whether a PDF's text layer is worth reading, or whether it has to be OCRed |
| [`bibclean`](docs/bibclean.md) | Catalogue records audited and repaired, with a reason for every change |
| [`mdscrub`](docs/mdscrub.md) | Converted markdown in, clean markdown out |
| [`chunkmd`](docs/chunkmd.md) | Markdown into passages that carry their page, chapter and role |
| [`cuenorm`](docs/cuenorm.md) | Any subtitle format in, one normalised cue stream out |
| [`cuepack`](docs/cuepack.md) | Cues into passages worth embedding, keeping the times they were said at |
| [`cueseg`](docs/cueseg.md) | A timed transcript into topic chapters |
| [`emb`](docs/emb.md) | Lines into vectors, against an embedding server |
| [`rrf`](docs/rrf.md) | Several ranked lists into one, without the database |

Full documentation, including the libraries underneath, is in
**[docs/](docs/README.md)**.

## A worked pipeline

Converted markdown in, retrievable passages out:

```
$ pandoc -t commonmark chapter.xhtml | mdscrub | chunkmd --min-tokens 20
{"v":1,"idx":0,"text":"The obligation ran both ways, and contemporaries said so plainly\nwhenever they were asked.4 A lord who could not\nprotect was a lord who could not command…","tokens":56,"page":41,"chapter":"III. The Second Estate","role":"body","start_time":null,"end_time":null,"flags":[]}
{"v":1,"idx":1,"text":"What the rolls do not say, and what has to be inferred, is how often the\nobligation was actually tested…","tokens":143,"page":42,"chapter":"III. The Second Estate","role":"body","start_time":null,"end_time":null,"flags":[]}
```

A recording instead of a book, with chapter marks a scrubber can use:

```
$ cuenorm episode.vtt | cueseg --duration 7200
{"v":1,"start":0.0,"end":1190.6,"title":"The Mill Stood At The Northern End","excerpt":"the mill stood at the northern end of the valley where…"}
{"v":1,"start":1190.6,"end":2381.3,"title":"Parliament Dissolved Itself Again","excerpt":"parliament dissolved itself again amid furious objections…"}
```

And before any of that, whether a scanned PDF is worth reading at all:

```
$ pdftriage scan.pdf
garbled, needs OCR
  1820 characters on the opening pages
  1820 characters over 5 sample pages, ascii text score 0.15
  letters 0.13, latin-1 0.29

$ for f in *.pdf; do pdftriage --quiet "$f" && ocrmypdf "$f" "ocr/$f"; done
```

## The contract

Every binary in this workspace obeys all of this, or it does not ship. The
whole of it is in [docs/contract.md](docs/contract.md).

1. **One job.** A tool that needs a `--mode` flag to change what it *is* is two
   tools.
2. **stdin to stdout.** Input on stdin or a path argument, result on stdout,
   diagnostics on stderr only.
3. **Streams, not documents.** Anything list-shaped is NDJSON: one record per
   line, flushed as it is produced.
4. **Predicates answer with exit codes.** `0` yes, `1` no, `2` could not tell;
   failures use the `sysexits` range, so "no" and "broke" are never the same
   number.
5. **No network, no database, no config file, no state**, with two declared
   exceptions.
6. **No ambient knowledge.** No hostnames, no local paths, no private
   vocabularies compiled in.
7. **Deterministic.** Same bytes in, same bytes out, across runs and machines.
8. **`--json` everywhere.** The human-readable default is for a person at a
   terminal; `--json` is the contract.

## The records

Three shapes travel between the tools, versioned, defined in `corpus-core` and
described in [docs/records.md](docs/records.md).

```
$ cargo run -p corpus-core -- schema all
```

## Build

```sh
./check.sh
```

Formatting, lints, tests and documentation, which is the same gate CI runs.

## Licence

MPL-2.0, library crates and binaries alike. The notice lives in `LICENSE` and
the `license` field of every manifest rather than in a header on every file,
which is what MPL-2.0 Exhibit A allows for.
