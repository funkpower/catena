# rrf

Several ranked lists into one, without the database.

Back to [the index](README.md).

## What it does

Reciprocal rank fusion. Reads [`hit`
records](records.md#hit--one-scored-candidate) from any number of retrieval
channels and writes one ranked list, best first.

## When you would use it

- **Hybrid search, without adopting anything.** Vector search and full-text
  search disagree about everything except what matters, and their scores are
  not comparable — a cosine distance and a text rank are different quantities
  on different scales. Fusion sidesteps that by throwing the scores away and
  keeping only the ranks: a result both channels put near the top beats one
  that either channel loves alone.
- **Making a metadata match count for more.** When somebody searches for a
  book by its title, the book rarely repeats its own title in its body, so the
  channel that matched the title is the one telling the truth. That is what
  the weights are for.
- **Merging results across corpora**, or across shards, or across any two
  systems that both return ranked lists.

This is the one tool here worth calling in process rather than as a
subprocess: by the time a hit reaches it the data is already yours, so the
isolation a separate process buys is not worth a spawn on an interactive
search path.

## Install

```sh
cargo install --git https://git.pumpin.online/funkpower/catena rrf
```

## Example

Three channels, overlapping:

```
$ cat vector.ndjson fts.ndjson meta.ndjson | rrf
{"v":1,"id":"e901","channel":"meta","rank":1,"score":0.040719196192490745,"payload":{"title":"The Human Condition"}}
{"v":1,"id":"c81a","channel":"vec","rank":2,"score":0.032266458495966696,"payload":{}}
{"v":1,"id":"b455","channel":"fts","rank":3,"score":0.032266458495966696,"payload":{}}
{"v":1,"id":"7f20","channel":"vec","rank":4,"score":0.016129032258064516,"payload":{}}
```

`e901` was third in full-text search and first in metadata, and the metadata
weight carries it to the top. `c81a` and `b455` tie, and the tie is broken by
which was seen first, so the same input always gives the same output.

The top ten:

```sh
cat *.ndjson | rrf --limit 10
```

Trusting full-text matching more than usual:

```sh
cat *.ndjson | rrf --weight fts=2 --weight meta=1
```

## Options

| Option | Default | Effect |
|---|---|---|
| `-n`, `--limit <COUNT>` | all | How many results to keep |
| `--rrf-k <K>` | `60` | The rank constant |
| `--weight <CHANNEL=WEIGHT>` | `vec=1 fts=1 meta=1.5` | What a channel's rank is worth. Repeatable |

The rank constant flattens the difference between rank one and rank two. Sixty
is what the method's own paper uses and what every other implementation uses,
which makes fused scores comparable across systems — worth more than tuning
it.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Done |
| `64` | The command line was wrong |
| `65` | A hit could not be read |
| `74` | The input could not be read |

## Input and output

One [`hit`](records.md#hit--one-scored-candidate) per line, in any order and
interleaved if you like — each carries the rank its own channel gave it, so
the stream does not have to be grouped.

The record that comes back for an id is the one that ranked it best, carrying
its payload, with `rank` and `score` replaced by its place and score in the
fused list. That makes `channel` on the output informative: it names which
channel liked this result most.

## The arithmetic

Each hit contributes `weight / (k + rank)` to its id's score. Nothing else is
used — not the channel's own score, which is exactly what is not comparable.

## As a library

```rust
let fused = rrf::fuse(&hits, &rrf::Weights::default());
```

## See also

- [`emb`](emb.md) — producing the vectors one of the channels searches.
- [The records](records.md) — the shape this reads and writes.
