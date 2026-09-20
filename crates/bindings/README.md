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

Three of them: Python, JavaScript through WebAssembly, and a C ABI. All three
call the same Rust and hand back the same records.

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

# Chapters, compared by the words a window repeats …
chapters = catena.segment_cues(cues, duration=7200)

# … or by vectors you made, which know that ships and vessels are one subject.
vectors = embed(catena.cue_windows(cues))
chapters = catena.segment_cues(cues, duration=7200, embeddings=vectors)
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
| `segment_cues` | [`cueseg`](../cueseg/README.md) |
| `cue_windows` | the windows `segment_cues` measures, to embed |
| `split_collection` | [`mdsplit`](../mdsplit/README.md) |
| `fuse_hits` | [`rrf`](../rrf/README.md) |
| `identify` | [`booksniff`](../booksniff/README.md) |
| `series_of` | [`seriesof`](../seriesof/README.md) |
| `parse_identifier` | [`idnorm`](../idnorm/README.md) |
| `find_citations` | [`citefind`](../citefind/README.md) |
| `looks_like_footnote_citations` | whether a body chunk carries notes worth reading |
| `suggest_author` | [`bibclean`](../bibclean/README.md)'s author recovery |
| `canonical_subject` | what a closed subject scheme calls a heading |
| `compose_subjects` | the subjects a book should carry under one |
| `needs_subject` | whether it carries a heading the scheme knows |
| `clean_record` | a whole catalogue record audited and repaired |
| `record_problems`, `record_severity` | what is wrong with one, and how much |
| `title_in_author`, `looks_like_person` | the two commonest catalogue faults |
| `is_shouting`, `title_case`, `clean_underscores` | the repairs that need no lookup |
| `valid_isbn`, `isbn_from_text`, `isbn_from_filename` | ISBNs, checked and found |
| `same_work` | whether two titles name the same book |
| `citation_prompt` | the question `citefind --prompt` writes |
| `merge_citation_reply` | what `citefind --merge` does with the answer |

The tools that are not here are the ones a binding would gain nothing over a
subprocess for: they read files, shell out to another program, or talk to a
server, and none of that is cheaper in process.

## JavaScript

```sh
npm install catena
```

```js
import { chunkMarkdown, scrub, countTokens, fuseHits } from "catena";

const chunks = chunkMarkdown(markdown, 512, 64);
chunks[0].page;      // 41
chunks[0].chapter;   // 'III. The Second Estate'

fuseHits([...vectorHits, ...textHits], undefined, { vec: 2 }, 20);
```

Records come back as plain objects, not handles: nothing to free,
`JSON.stringify` works, and an array of chunks spreads into state without
ceremony. The declarations are hand-written, so a caller gets `Chunk[]` rather
than `any`.

`splitCollection` throws when the document is one work rather than a
collection, which is the usual answer.

Two builds, because the loaders differ — node reads the `.wasm` off disk at
import, the web build takes a URL a bundler rewrites:

```js
import { chunkMarkdown } from "catena";        // resolves per environment
import { chunkMarkdown } from "catena/node";   // or pick one
import { chunkMarkdown } from "catena/web";
```

## C

```c
#include <catena.h>

char *chunks = catena_chunk_markdown(markdown, 512, 64, 50, 768);
if (!chunks) fprintf(stderr, "%s\n", catena_last_error());
/* … a JSON array of chunk records … */
catena_string_free(chunks);
```

Text in as NUL-terminated UTF-8, records out as NUL-terminated JSON — the same
document the command line writes. Nothing here is an opaque handle with
accessors to keep in step: the records already have a documented shape, and
every language with an FFI has a JSON parser.

Every returned string is yours and must go back through `catena_string_free`.
A call that fails returns NULL and leaves a message in `catena_last_error`,
which is thread-local. `catena_abi_version` is bumped when anything here
changes meaning; additions leave it alone.

A budget of zero means that budget's default, so a caller that cares about one
of them can pass zero for the rest.

## Building

```sh
cd crates/bindings
maturin build --profile wheel      # the Python wheel
npm/build.sh                       # the npm package
cargo build --features capi        # the C library and its header
```

Three things worth knowing, each of which fails confusingly otherwise:

**`cargo build --features python` does not work and is not supposed to.**
pyo3's `extension-module` leaves the Python symbols to be resolved by the
interpreter at load time, so there is nothing to link against until an
interpreter loads it. `cargo clippy --features python` type-checks it without
linking, which is what the gate runs.

**The wheel needs `--profile wheel`, not `--release`.** The release profile
strips, and a stripped extension module is one macOS refuses to load at all —
"mis-aligned LINKEDIT string pool". The `wheel` profile is the release profile
without it. `[tool.maturin] profile` is ignored by maturin 1.15, so it has to
be the flag.

**The WebAssembly build is only compiled for wasm32.** On any other target the
module compiles to nothing, so `--all-features` on a host build does not reach
it; the gate checks it separately against `wasm32-unknown-unknown`.

## Swapping a stage over

A binding that returns *nearly* the same answer as the code it replaces is
worse than no binding: the pipeline keeps running and the corpus quietly
changes underneath it.

So before swapping anything, run both implementations over a directory of real
files and compare the answers field by field — not a sample, and not the test
fixtures, which are the inputs somebody already thought of. The things that
find divergences are the documents nobody chose.

Two details worth the trouble:

- **Compare against the code you are actually replacing.** A checkout that has
  moved on since the port measures the drift rather than the port, and reports
  failures that are nobody's bug.
- **Check that the comparison can fail.** Feed one side a deliberately wrong
  setting and confirm it is reported. A check that cannot fail proves nothing,
  and one that silently compares nothing looks exactly like success.

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
