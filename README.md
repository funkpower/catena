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

## A worked pipeline

Converted markdown in, retrievable passages out:

```
$ pandoc -t commonmark chapter.xhtml | mdscrub | chunkmd --min-tokens 20
{"v":1,"idx":0,"text":"The obligation ran both ways, and contemporaries said so plainly\nwhenever they were asked.4 A lord who could not\nprotect was a lord who could not command…","tokens":57,"page":41,"chapter":"III. The Second Estate","role":"body","start_time":null,"end_time":null,"flags":[]}
{"v":1,"idx":1,"text":"What the rolls do not say, and what has to be inferred, is how often the\nobligation was actually tested…","tokens":41,"page":42,"chapter":"III. The Second Estate","role":"body","start_time":null,"end_time":null,"flags":[]}
```

A recording instead of a book, with chapter marks a scrubber can use:

```
$ cuenorm episode.vtt | cueseg --duration 7245
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

## The tools

| | |
|---|---|
| `mdscrub` | Converted markdown in, clean markdown out: residual HTML, control characters, headings, page markers |
| `chunkmd` | Markdown into passages that carry their page, chapter and role |
| `pdftriage` | Whether a PDF's text layer is healthy, garbled, sparse, permuted, or absent — and so whether OCR is worth its cost |
| `cuenorm` | Any subtitle format in, one normalised cue stream out |
| `cueseg` | A timed transcript into topic chapters |

## The contract

Every binary in this workspace obeys all of this, or it does not ship.

1. **One job.** A tool that needs a `--mode` flag to change what it *is* is two
   tools.
2. **stdin to stdout.** Input on stdin or a path argument, result on stdout,
   diagnostics on stderr only — never content and chatter on the same stream.
3. **Streams, not documents.** Anything list-shaped is NDJSON: one record per
   line, flushed as it is produced, so a slow stage never blocks the pipe.
4. **Predicates answer with exit codes.** `0` yes, `1` no, `2` could not tell.
   Failures use the `sysexits` range — `64` bad usage, `65` bad input, `69` a
   missing external program, `70` the tool's own fault, `74` I/O — so "no" and
   "broke" are never the same number. A consumer closing the pipe is not a
   failure: `… | head -1` exits `0`, like every other filter.
5. **No network, no database, no config file, no state.** Two exceptions, both
   declared in `--help`: a tool whose whole job is an HTTP call, and a tool
   that shells out to a named external program and fails clearly when it is
   absent.
6. **No ambient knowledge.** No hostnames, no local paths, no private
   vocabularies compiled in. General scheme data — Bible book names, ISBN check
   digits — may be; anything local is a `--rules file.json`.
7. **Deterministic.** Same bytes in, same bytes out, across runs and machines.
8. **`--json` everywhere.** The human-readable default is for a person at a
   terminal; `--json` is the contract, and it does not change shape without a
   major version.

## The records

Three shapes travel between the tools, defined in `corpus-core` and versioned
by a `v` field.

**`doc`** — what a file is.

```json
{"v":1,"path":"scan.pdf","kind":"pdf","declared_ext":"pdf","bytes":9261184,
 "pages":412,"language":"eng","text_layer":"garbled","needs_ocr":true,"notes":[]}
```

**`chunk`** — a retrievable passage with enough provenance to cite it.

```json
{"v":1,"idx":7,"text":"…","tokens":512,"page":37,"chapter":"III. The Second Estate",
 "role":"body","start_time":null,"end_time":null,"flags":[]}
```

`page` and `chapter` for documents, `start_time` and `end_time` for
recordings. One shape either way, so a single index serves a book and a
lecture and returns both with a location you can cite.

**`hit`** — one scored candidate, on its way into a fusion.

```json
{"v":1,"id":"c81a","channel":"vec","rank":1,"score":0.83,"payload":{}}
```

The JSON Schema for all three:

```sh
cargo run -p corpus-core -- schema all
```

## Build

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

## Licence

MPL-2.0, library crates and binaries alike. The notice lives in `LICENSE` and
the `license` field of every manifest rather than in a header on every file,
which is what MPL-2.0 Exhibit A allows for.
