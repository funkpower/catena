# chunkmd

Markdown into passages that carry their page, chapter and role.

Part of [catena](../../README.md).

## What it does

Cuts a document into pieces of about the size an embedding model wants, and
puts on each one the page it came from, the chapter it is in, and what part of
the work it belongs to.

Writes [`chunk` records](../../docs/records.md#chunk--a-retrievable-passage), one per
line.

## When you would use it

- **Before embedding anything.** A whole chapter is too big to embed usefully
  and a sentence is too small to mean anything; this is the step in between.
- **When a result has to be citable.** A passage a reader cannot locate in the
  book is a passage they cannot check, so the page and chapter are carried
  from the first stage rather than reconstructed later.
- **To keep the apparatus out of search results.** An index, a table of
  contents and a bibliography all match a query beautifully and answer
  nothing — they are lists of the words somebody is searching for. Marking
  them lets a search exclude them while keeping them addressable for the
  consumers that want them.

## Install

```sh
cargo install --git https://git.pumpin.online/funkpower/catena chunkmd
```

## Example

```
$ mdscrub chapter.md | chunkmd --min-tokens 20
{"v":1,"idx":0,"text":"The obligation ran both ways, and contemporaries said so plainly\nwhenever they were asked.4 A lord who could not\nprotect was a lord who could not command, and the men who owed him service\nsaid as much in language that has survived in the court rolls.","tokens":56,"page":41,"chapter":"III. The Second Estate","role":"body","start_time":null,"end_time":null,"flags":[]}
{"v":1,"idx":1,"text":"What the rolls do not say, and what has to be inferred, is how often the\nobligation was actually tested…","tokens":143,"page":42,"chapter":"III. The Second Estate","role":"body","start_time":null,"end_time":null,"flags":[]}
```

Just the text, for an embedder:

```sh
mdscrub chapter.md | chunkmd | jq -r .text | emb --model bge-m3 --dim 1024
```

Only the body, leaving the bibliography and the index behind:

```sh
chunkmd book.md | jq -c 'select(.role == "body")'
```

## Options

| Option | Default | Effect |
|---|---|---|
| `--target <TOKENS>` | `512` | Flush a chunk at this many tokens |
| `--overlap <TOKENS>` | `64` | Carry this much of each chunk into the next |
| `--min-tokens <TOKENS>` | `50` | Drop chunks smaller than this |
| `--hard-max <TOKENS>` | `768` | Never emit a chunk larger than this |

The defaults suit a model with a 512-token comfortable window. A larger model
wants roughly double everything:

```sh
chunkmd --target 1024 --overlap 128 --min-tokens 80 --hard-max 1280
```

Budgets that cannot produce anything sensible — an overlap at or above the
target, a hard maximum below the target — are refused before any input is
read.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Done |
| `64` | The command line was wrong |
| `65` | The input was not text |
| `74` | The file could not be read |

## How it cuts

Page first, paragraph second, sentence only when a paragraph is itself too
big.

1. **Split on page markers**, so a chunk never spans a page and its page
   number is exact rather than inferred. Text before the first marker is kept,
   with no page number — it is still the book.
2. **Pack paragraphs** toward the token target within each page, carrying an
   overlap forward so a sentence that answers a question is not orphaned from
   the sentence that asked it.
3. **Enforce the hard maximum** on token boundaries, whatever the structure
   said.
4. **Drop what is too small** to be worth an embedding.
5. **Classify roles** across the whole document at once, because deciding a
   stray chunk needs to see its neighbours: a short run of body chunks
   sandwiched between two bibliography chunks is part of the bibliography.

A chunk whose text is a page of numbered notes is flagged `footnote_citations`,
because footnotes are citations in disguise — they read as body text, rank as
body text, and answer a query with a publisher and a page number.

## The roles

| Role | What it is |
|---|---|
| `body` | Running text |
| `bibliography` | A works-cited or references list |
| `index` | A back-of-book index |
| `toc` | A table of contents |
| `frontmatter` | Title page, copyright page, dedication |

## As a library

```rust
use chunkmd::Budgets;

for chunk in chunkmd::chunk(&markdown, &Budgets::default()) {
    println!("{} tokens on page {:?}", chunk.tokens, chunk.page);
}
```

## See also

- [`mdscrub`](../mdscrub/README.md) — what to run before this.
- [`mdsplit`](../mdsplit/README.md) — when the file holds more than one work.
- [`cuepack`](../cuepack/README.md) — the same record, from a recording.
- [`emb`](../emb/README.md) — turning the text into vectors.
- [The records](../../docs/records.md) — the shape this writes.
