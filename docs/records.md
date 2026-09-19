# The records

Five shapes travel between these tools. Each carries a version field `v`, so a
reader can refuse a stream it does not understand rather than parsing it into
whatever happens to line up.

Every list-shaped stream is [NDJSON](contract.md#anything-list-shaped-is-ndjson): one
record per line, flushed as it is produced.

Part of [catena](../README.md).

## `doc` — what a file is

```json
{"v":1,"path":"scan.pdf","kind":"pdf","declared_ext":"pdf","bytes":9261184,
 "pages":412,"language":"eng","text_layer":"garbled","needs_ocr":true,"notes":[]}
```

| Field | Meaning |
|---|---|
| `path` | Where the file was read from, as given |
| `kind` | `pdf`, `epub`, `djvu`, `azw3`, `mobi`, `txt`, `html`, `ole2`, `zip-wrapped-pdf`, `unknown` |
| `declared_ext` | The extension the name claimed, lowercased, without the dot |
| `bytes` | Size on disk |
| `pages` | Page count, for paginated formats |
| `language` | Language code, when one was detected |
| `text_layer` | `healthy`, `garbled`, `sparse`, `reflected-font`, `absent` |
| `needs_ocr` | Whether the file has to be OCRed to be searchable |
| `notes` | Observations, in the order they were made |

A tool fills what it can and leaves the rest null: a format sniffer fills
`kind` and leaves `text_layer` empty, a triage tool does the reverse.

Written by [`booksniff`](../crates/booksniff/README.md) and [`pdftriage`](../crates/pdftriage/README.md).

## `chunk` — a retrievable passage

```json
{"v":1,"idx":7,"text":"…","tokens":512,"page":37,"chapter":"III. The Second Estate",
 "role":"body","start_time":null,"end_time":null,"flags":[]}
```

| Field | Meaning |
|---|---|
| `idx` | Position in the stream this chunk was cut from, from zero |
| `text` | The passage |
| `tokens` | Token count of `text` |
| `page` | Page the passage starts on, for paginated sources |
| `chapter` | Nearest enclosing heading, as written |
| `role` | `body`, `bibliography`, `index`, `toc`, `frontmatter` |
| `start_time`, `end_time` | Seconds from the start, for recordings |
| `flags` | Set tags, sorted and deduplicated |

`page` and `chapter` for documents, `start_time` and `end_time` for
recordings. One shape either way, which is what lets a single index serve a
book and a lecture and cite both.

Written by [`chunkmd`](../crates/chunkmd/README.md) and [`cuepack`](../crates/cuepack/README.md).

## `hit` — one scored candidate

```json
{"v":1,"id":"c81a","channel":"vec","rank":1,"score":0.83,"payload":{}}
```

| Field | Meaning |
|---|---|
| `id` | Identifier of the thing retrieved, opaque to fusion |
| `channel` | `vec`, `fts` or `meta` |
| `rank` | Position within that channel's own ranking, from one |
| `score` | The channel's raw score, on whatever scale it uses |
| `payload` | Anything the producer wants carried through untouched |

`rank` is what fusion consumes. `score` is carried along because it is useful
to a person reading a trace and useless for comparing across channels.

Read and written by [`rrf`](../crates/rrf/README.md).

## `cue` — one line of a transcript

```json
{"v":1,"start":12.5,"end":15.0,"text":"the mill stood at the northern end"}
```

Smaller than a chunk on purpose: a cue is what a subtitle file holds, a few
seconds and often half a sentence.

Written by [`cuenorm`](../crates/cuenorm/README.md), read by [`cuepack`](../crates/cuepack/README.md) and
[`cueseg`](../crates/cueseg/README.md).

## `segment` — one topic chapter

```json
{"v":1,"start":0.0,"end":1190.7,"title":"The Mill Stood At The Northern End",
 "excerpt":"the mill stood at the northern end of the valley where…"}
```

The marks a scrubber needs, and the spans a summariser should be given one at
a time.

Written by [`cueseg`](../crates/cueseg/README.md).

## Records that belong to one tool

Not everything is shared. [`bibclean`](../crates/bibclean/README.md) reads and
writes catalogue records, author suggestions, slug proposals and vocabulary
merge proposals; those shapes are described where that tool is, because
nothing else consumes them.

## Getting the schemas

The JSON Schema for the first three is printed by the record library itself:

```
$ cargo run -p corpus-core -- schema chunk
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Chunk",
  …
}
```

See [`corpus-core`](../crates/corpus-core/README.md).
