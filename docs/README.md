# catena

Small programs that each do one part of turning a pile of files into a
searchable corpus, and hand the next one text on stdout.

Every one of them is a filter: bytes in, records out, diagnostics on stderr,
and an exit code that means something. None of them needs a database, a GPU, a
queue or a config file.

## The tools

### Working out what you have

| | |
|---|---|
| [`booksniff`](booksniff.md) | What a file actually is, whatever its name claims |
| [`pdftriage`](pdftriage.md) | Whether a PDF's text layer is worth reading, or whether it has to be OCRed |
| [`bibclean`](bibclean.md) | Catalogue records audited and repaired, with a reason for every change |

### Turning documents into passages

| | |
|---|---|
| [`mdscrub`](mdscrub.md) | Converted markdown in, clean markdown out |
| [`chunkmd`](chunkmd.md) | Markdown into passages that carry their page, chapter and role |

### Turning recordings into passages

| | |
|---|---|
| [`cuenorm`](cuenorm.md) | Any subtitle format in, one normalised cue stream out |
| [`cuepack`](cuepack.md) | Cues into passages worth embedding, keeping the times they were said at |
| [`cueseg`](cueseg.md) | A timed transcript into topic chapters |

### Searching

| | |
|---|---|
| [`emb`](emb.md) | Lines into vectors, against an embedding server |
| [`rrf`](rrf.md) | Several ranked lists into one, without the database |

## The libraries underneath

Every tool is a library crate with a thin command on top, so anything a
command does can be done in process instead.

| | |
|---|---|
| [`corpus-core`](corpus-core.md) | The record types and the stream plumbing everything shares |
| [`textkit`](textkit.md) | Token counting, sentence and paragraph splitting |
| [`cuekit`](cuekit.md) | The cue model, and the three subtitle formats it is read from |
| [`catena-cli`](catena-cli.md) | The parts of a filter every command does the same way |

## Reference

- [The records](records.md) — the shapes that travel between the tools.
- [The contract](contract.md) — what every one of these programs promises.

## A worked pipeline

A book, from a converted chapter to passages an index can hold:

```
$ mdscrub chapter.md | chunkmd --min-tokens 20
{"v":1,"idx":0,"text":"The obligation ran both ways, and contemporaries said so plainly…","tokens":56,"page":41,"chapter":"III. The Second Estate","role":"body","start_time":null,"end_time":null,"flags":[]}
{"v":1,"idx":1,"text":"What the rolls do not say, and what has to be inferred…","tokens":143,"page":42,"chapter":"III. The Second Estate","role":"body","start_time":null,"end_time":null,"flags":[]}
```

A recording, through the same shape:

```
$ cuenorm episode.vtt | cuepack --seconds 10
{"v":1,"idx":0,"text":"The mill stood at the northern end of the valley, where the fall of the river was steepest and the ice came latest.","tokens":27,"page":null,"chapter":null,"role":"body","start_time":1.0,"end_time":8.0,"flags":[]}
{"v":1,"idx":1,"text":"Its wheel was undershot, which was wasteful,","tokens":11,"page":null,"chapter":null,"role":"body","start_time":11.0,"end_time":19.0,"flags":[]}
```

Both produce the same record, so one index answers across a book and a lecture
and returns both with a location you can cite — a page number for one, a
timestamp for the other.
