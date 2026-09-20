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
| [`booksniff`](crates/booksniff/README.md) | What a file actually is, whatever its name claims |
| [`bookmeta`](crates/bookmeta/README.md) | The metadata a file already carries |
| [`tocfind`](crates/tocfind/README.md) | A book's own table of contents |
| [`langid`](crates/langid/README.md) | What language a document is in |
| [`pdfgutter`](crates/pdfgutter/README.md) | Books scanned two pages to a sheet, split back into pages |
| [`pdftriage`](crates/pdftriage/README.md) | Whether a PDF's text layer is worth reading, or whether it has to be OCRed |
| [`bibclean`](crates/bibclean/README.md) | Catalogue records audited and repaired, with a reason for every change |
| [`vocabmerge`](crates/bibclean/README.md#vocabmerge--the-other-command) | The near-duplicate headings in a catalogue's vocabulary |
| [`seriesof`](crates/seriesof/README.md) | The series a book belongs to, and which volume this is |
| [`idnorm`](crates/idnorm/README.md) | DOIs, ISSNs, ISBNs and arXiv ids, read and checked |
| [`bibfetch`](crates/bibfetch/README.md) | What a register says about an identifier |
| [`citefind`](crates/citefind/README.md) | The references in a bibliography or a page of notes |
| [`mdscrub`](crates/mdscrub/README.md) | Converted markdown in, clean markdown out |
| [`mdsplit`](crates/mdsplit/README.md) | One markdown file into the works inside it |
| [`chunkmd`](crates/chunkmd/README.md) | Markdown into passages that carry their page, chapter and role |
| [`cuenorm`](crates/cuenorm/README.md) | Any subtitle format in, one normalised cue stream out |
| [`cuepack`](crates/cuepack/README.md) | Cues into passages worth embedding, keeping the times they were said at |
| [`cueseg`](crates/cueseg/README.md) | A timed transcript into topic chapters |
| [`emb`](crates/emb/README.md) | Lines into vectors, against an embedding server |
| [`rrf`](crates/rrf/README.md) | Several ranked lists into one, without the database |
| [`avplan`](crates/avplan/README.md) | Whether a recording plays, and the one `ffmpeg` call that fixes it |

Each links to its own page: what it does, when you would use it, worked
examples, options and exit codes.

**→ [Recipes](docs/recipes.md) — using these tools together.** A scanned book
to a searchable passage, a transcript to a citation with a timestamp, vectors
into pgvector, and hybrid search fused on top.

## The libraries underneath

Every tool is a library crate with a thin command on top, so anything a
command does can be done in process instead.

| | |
|---|---|
| [`corpus-core`](crates/corpus-core/README.md) | The record types and the stream plumbing everything shares |
| [`textkit`](crates/textkit/README.md) | Token counting, sentence and paragraph splitting |
| [`cuekit`](crates/cuekit/README.md) | The cue model, and the three subtitle formats it is read from |
| [`catena-cli`](crates/catena-cli/README.md) | The parts of a filter every command does the same way |
| [`bindings`](crates/bindings/README.md) | The same libraries from Python, C and JavaScript |

## From another language

The tools are filters first, and a subprocess is the right answer most of the
time. When the caller is already holding the text, the same libraries are
callable in process from three languages:

| | Install | Import |
|---|---|---|
| **Python** | `pip install catena` | `import catena` |
| **JavaScript** | `npm install catena` | `import { chunkMarkdown } from "catena"` |
| **C** | build the `capi` feature | `#include <catena.h>` |

```python
chunks = catena.chunk_markdown(markdown, target=512)
chunks[0]["page"]        # 41
```

Each hands back the same record shapes the commands write — dictionaries in
Python, plain objects in JavaScript, JSON in C — so a stage that was parsing
NDJSON from a subprocess can call the function and get what it was already
parsing. Details in [`bindings`](crates/bindings/README.md).

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

## What they promise

Eight things hold for all of them, and together they are what lets any of
these go into a pipeline without first reading how it works:

- Each does a single job, with no mode switch that changes what it is.
- Input on stdin, results on stdout, diagnostics on stderr — never mixed.
- Anything list-shaped is NDJSON, one record per line, flushed as produced.
- Exit codes carry the answer: `0` yes, `1` no, `2` could not tell, and
  failures in the `sysexits` range, so "no" and "broke" are never the same
  number.
- Nothing reaches the network, a database, a config file or any stored state,
  bar two exceptions that say so in `--help`.
- Nothing is assumed about where it is running: no hostnames, no paths, no
  private vocabularies built in.
- The same input gives the same output, on every run and every machine.
- `--json` is the stable form, and does not change shape without a major
  version.
- `--man` writes the program's manual page, generated from the same definition
  `--help` is.

The whole of it, with the reasoning: [what these programs
promise](docs/contract.md).

## The records

Five shapes travel between the tools, versioned, defined in
[`corpus-core`](crates/corpus-core/README.md) and described in [the
records](docs/records.md).

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
