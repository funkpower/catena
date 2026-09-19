# catena (bindings)

The catena libraries behind one surface, for calling in process rather than as
a subprocess.

Part of [catena](../../README.md).

## What it does

Every tool in this workspace is a library with a thin command on top. A
pipeline written in another language can have either.

A subprocess is the right answer most of the time: it is a real process
boundary, it composes with a shell, and it costs a few milliseconds. In
process is the right answer when the caller is already holding the text — a
stage that would otherwise write a file so that it can read it straight back.

What comes back is the same **record shapes** the commands write to stdout,
as dictionaries rather than lines of JSON. Those shapes are documented and
versioned; the structs behind them are free to move.

## Python

```sh
pip install catena
```

```python
import catena

chunks = catena.chunk_markdown(markdown, target=512, overlap=64)
chunks[0]["page"]        # 41
chunks[0]["chapter"]     # 'III. The Second Estate'

clean = catena.scrub(raw_markdown, pages=True)
catena.count_tokens(clean)

cues = catena.parse_subtitles(open("episode.srt").read())
passages = catena.pack_cues(cues, seconds=75)

catena.identify("atlas.pdf", open("atlas.pdf", "rb").read(1024))["kind"]
```

`split_collection` raises `ValueError` when the document is one work rather
than a collection — the usual answer, with the reason each candidate was
rejected:

```python
try:
    works = catena.split_collection(markdown)
except ValueError as why:
    works = [{"idx": 0, "title": title, "text": markdown}]
```

### What is bound

| Function | The tool it is |
|---|---|
| `chunk_markdown` | [`chunkmd`](../chunkmd/README.md) |
| `scrub` | [`mdscrub`](../mdscrub/README.md) |
| `count_tokens` | [`textkit`](../textkit/README.md) |
| `parse_subtitles` | [`cuenorm`](../cuenorm/README.md) |
| `pack_cues` | [`cuepack`](../cuepack/README.md) |
| `split_collection` | [`mdsplit`](../mdsplit/README.md) |
| `fuse_hits` | [`rrf`](../rrf/README.md) |
| `identify` | [`booksniff`](../booksniff/README.md) |

The tools that are not here are the ones a binding would gain nothing over a
subprocess for: they read files, shell out to another program, or talk to a
server, and none of that is cheaper in process.

## Building the wheel

```sh
cd crates/bindings && maturin build --release
```

`cargo build --features python` does not work and is not supposed to: pyo3's
`extension-module` leaves the Python symbols to be resolved by the interpreter
at load time, so there is nothing to link against until an interpreter loads
it. `cargo clippy --features python` type-checks it without linking, which is
what the gate runs.

## As a Rust library

The same surface, without a binding in the way:

```rust
let chunks = catena::chunk_markdown(&markdown, catena::ChunkBudgets::default());
```

Nothing here grows logic of its own. If a binding needs something the
libraries cannot do, that is a missing library function rather than a binding
feature.

## See also

- [The records](../../docs/records.md) — the shapes these hand back.
- [Recipes](../../docs/recipes.md) — the same pipelines from a shell.
