# Examples

Working programs that put these tools together, in the four ways you would
actually reach for them: as filters in a shell script, and as libraries from
Rust, Python and JavaScript.

Every one of them runs. Where an example needs a book it fetches a public
domain one from Project Gutenberg; where it needs a recording it fetches a
NASA broadcast, which is a work of the US government.

| | |
|---|---|
| [`shell/01-book-to-passages.sh`](shell/01-book-to-passages.sh) | A book file to passages worth embedding, and what its notes cite |
| [`shell/02-video-to-chapters.sh`](shell/02-video-to-chapters.sh) | A recording to chapter marks, citable passages, and a copy that plays on a phone |
| [`shell/03-search-and-rerank.sh`](shell/03-search-and-rerank.sh) | Passages into pgvector, then a search that asks twice and reconciles |
| [`rust/`](rust/src/main.rs) | The same book walked through in one process, with no pipe anywhere |
| [`python/book_to_citations.py`](python/book_to_citations.py) | A book to rows a database can take |
| [`python/search_and_rerank.py`](python/search_and_rerank.py) | The pgvector search, in the language a pipeline is usually written in |
| [`typescript/src/episode-to-chapters.ts`](typescript/src/episode-to-chapters.ts) | Captions into chapter marks and deep links for a player |
| [`typescript/src/search-and-rerank.ts`](typescript/src/search-and-rerank.ts) | Fusing two rankings app-side, and what re-ranking means |

## Shell

The tools are filters first: bytes in, records out, and `jq` between them.
Nothing is installed but the commands themselves.

```sh
cargo install --git https://git.pumpin.online/funkpower/catena \
    booksniff bookmeta tocfind pdftriage mdscrub chunkmd citefind \
    cuenorm cueseg cuepack avplan emb rrf

examples/shell/01-book-to-passages.sh ./work
```

`01` fetches Gibbon's first volume, works out what the file is, reads the
metadata and the contents the publisher declared, converts and scrubs and
chunks it, and reads the footnotes. Needs `pandoc` and `jq`.

`02` takes a recording's captions and produces chapter marks, passages that
carry the second they were said at, and — since a recording that is indexed
but unplayable is half a job — the one `ffmpeg` call that makes it play on a
phone. Needs `jq`, `ffmpeg`, and yt-dlp with impersonation support, which
YouTube now requires before it will hand over captions:

```sh
pipx install "yt-dlp[default,curl-cffi]"
```

`03` loads the passages `01` wrote into Postgres with a vector column and a
text index, searches both ways, and fuses the two rankings. Needs `psql`, a
Postgres with pgvector, and an embedding server:

```sh
DATABASE_URL=postgresql://user@host/db \
EMB_MODEL=nomic-embed-text \
    examples/shell/03-search-and-rerank.sh "why did the empire decline" ./work
```

## Rust

Every tool is a library with a thin command on top, so anything a command
does can be done in process instead. This walks a file from "what even is it"
to passages and references without serialising anything.

```sh
cd examples/rust
cargo run                 # the built-in sample
cargo run -- book.md      # a real one
```

## Python

The shape a pipeline stage ends up as once the reading is a library call:
convert, then loop, and what is left is the part that is actually about your
schema.

```sh
pip install catena-corpus
python examples/python/book_to_citations.py
```

```sh
pip install catena-corpus psycopg
DATABASE_URL=postgresql://user@host/db \
    python examples/python/search_and_rerank.py "why did the empire decline"
```

## TypeScript

The bindings are WebAssembly, so they run in Node and in a browser with the
same API.

```sh
cd examples/typescript
npm install
npm run chapters          # captions to chapter marks
npm run search            # fusing two rankings
```

## On re-ranking

Three of these examples end with the same explanation, because it is the
thing most often got wrong.

Retrieval uses a **bi-encoder**: the passage and the query are embedded
separately. That independence is what lets passage vectors be computed once
and indexed, and it is also the limit — nothing in a passage's vector knows
what was asked.

**Fusion** takes several ranked lists and makes one. Reciprocal rank fusion
scores a hit by where it placed rather than by what it scored: `1/(k + rank)`
from each list that found it, added up. It is used because the scores are not
comparable — a cosine distance and a `ts_rank_cd` measure different things,
and no scaling makes them the same — so the ranks are kept and the scores
thrown away. A passage that both searches liked beats one that either loved.
That is [`rrf`](../crates/rrf/README.md), and it needs no model.

A **cross-encoder** reads the query and the passage together and scores the
pair. Far more accurate, and far too slow to run over a corpus, so it runs
over the twenty or fifty candidates retrieval already found. In a pipeline
the two stack: retrieve both ways, fuse the lists, then hand the top of the
fused list to a cross-encoder for the final order.

Both get called re-ranking. Nothing here loads a model, so the last step is
yours — the fused list is what you would hand it.
