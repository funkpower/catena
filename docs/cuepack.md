# cuepack

Cues into passages worth embedding, keeping the times they were said at.

Back to [the index](README.md).

## What it does

Packs consecutive cues into passages of about a minute of continuous speech,
and writes each one as a [`chunk`
record](records.md#chunk--a-retrievable-passage) carrying the start of its
first cue and the end of its last.

## When you would use it

- **Before embedding a transcript.** A cue is a few seconds of speech and
  often half a sentence; an embedding of one says almost nothing, and a search
  over them returns fragments.
- **When a result has to point somewhere.** Because the passage keeps the
  first cue's start and the last cue's end, a hit can be cited to the second
  and a player can jump to it.
- **To index recordings and books together.** The record is the same one
  [`chunkmd`](chunkmd.md) writes, so one index answers across both and returns
  each with a location: a page number for one, a timestamp for the other.

## Install

```sh
cargo install --git https://git.pumpin.online/funkpower/catena cuepack
```

## Example

```
$ cuenorm episode.vtt | cuepack --seconds 10
{"v":1,"idx":0,"text":"The mill stood at the northern end of the valley, where the fall of the river was steepest and the ice came latest.","tokens":27,"page":null,"chapter":null,"role":"body","start_time":1.0,"end_time":8.0,"flags":[]}
{"v":1,"idx":1,"text":"Its wheel was undershot, which was wasteful,","tokens":11,"page":null,"chapter":null,"role":"body","start_time":11.0,"end_time":19.0,"flags":[]}
{"v":1,"idx":2,"text":"and nobody replaced it for two hundred years.","tokens":9,"page":null,"chapter":null,"role":"body","start_time":19.0,"end_time":24.0,"flags":[]}
```

Straight to an embedder:

```sh
cuenorm episode.vtt | cuepack | jq -r .text | emb --model bge-m3 --dim 1024
```

It also reads a subtitle file directly, so the middle step can be skipped when
nothing else needs the cues:

```sh
cuepack episode.srt > passages.ndjson
```

## Options

| Option | Default | Effect |
|---|---|---|
| `--seconds <SECONDS>` | `75` | Flush a passage once it covers this much audio |
| `--max-tokens <TOKENS>` | `512` | Never let a passage exceed this many tokens |

Seventy-five seconds is about a paragraph of speech: long enough to hold an
argument, short enough that a result points at a place in the recording rather
than at a region of it.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Done |
| `64` | The command line was wrong |
| `65` | The cue stream could not be read |
| `74` | The file could not be read |

## How it packs

A passage is flushed when the window from its first cue's start to the current
cue's end reaches the seconds budget, or when adding the current cue would
take it past the token budget. The cue that trips the budget starts the next
passage rather than finishing this one.

Immediately repeated lines are dropped rather than packed. Automatic
captioning repeats each line across overlapping windows, and a passage padded
with its own echo embeds as something nobody said.

## Input

Either the cue stream [`cuenorm`](cuenorm.md) writes, or a subtitle file. A
cue stream begins with a JSON object and a subtitle file does not, which is
enough to tell them apart.

## As a library

```rust
use cuepack::Budgets;

for passage in cuepack::pack(&cues, &Budgets::default()) {
    println!("{:?}–{:?}: {}", passage.start_time, passage.end_time, passage.text);
}
```

## See also

- [`cuenorm`](cuenorm.md) — what to run before this.
- [`cueseg`](cueseg.md) — the other thing to do with a cue stream.
- [`chunkmd`](chunkmd.md) — the same record, from a book.
