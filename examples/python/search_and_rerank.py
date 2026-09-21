"""Passages into pgvector, then a search that asks twice and reconciles.

Loads the passages of a book into Postgres with both a vector column and a
text index, searches both ways, and fuses the two rankings into one.

    DATABASE_URL=postgresql://user@host/db \
    EMB_MODEL=nomic-embed-text \
        python examples/python/search_and_rerank.py "why did the empire decline"

Needs psycopg, a Postgres with pgvector, an embedding server with Ollama's
API, and a book — the sample is fetched if none is given.
"""

import json
import os
import pathlib
import subprocess
import sys
import urllib.request

import catena
import psycopg

EMB_HOST = os.environ.get("EMB_HOST", "http://localhost:11434")
EMB_MODEL = os.environ.get("EMB_MODEL", "nomic-embed-text")
GUTENBERG = "https://www.gutenberg.org/cache/epub/731/pg731.epub"


def embed(texts: list[str]) -> list[list[float]]:
    """Ask the embedding server, in batches it will accept.

    The one thing here that touches the network. No model is loaded in this
    process: it belongs behind a server, where it is shared and where its
    GPU is somebody else's problem.
    """
    out: list[list[float]] = []
    for at in range(0, len(texts), 32):
        batch = texts[at : at + 32]
        body = json.dumps({"model": EMB_MODEL, "input": batch}).encode()
        request = urllib.request.Request(
            f"{EMB_HOST}/api/embed", data=body, headers={"content-type": "application/json"}
        )
        with urllib.request.urlopen(request, timeout=300) as reply:  # noqa: S310
            out.extend(json.load(reply)["embeddings"])
        print(f"   embedded {len(out)}/{len(texts)}", end="\r", file=sys.stderr)
    return out


def passages(book: pathlib.Path) -> list[dict]:
    """The book, cut into passages worth embedding."""
    markdown = subprocess.run(
        ["pandoc", "-f", "epub", "-t", "commonmark", "--wrap=none", str(book)],
        capture_output=True, text=True, check=True,
    ).stdout
    chunks = catena.chunk_markdown(catena.scrub(markdown))
    # A table of contents embeds as well as anything and retrieves for every
    # query, which is worse than not having it.
    return [c for c in chunks if c["role"] == "body" and c["tokens"] > 40]


def load(conn, rows: list[dict], vectors: list[list[float]]) -> None:
    width = len(vectors[0])
    with conn.cursor() as cur:
        cur.execute("CREATE EXTENSION IF NOT EXISTS vector")
        cur.execute("DROP TABLE IF EXISTS catena_passages")
        # The tsvector is generated from the body rather than written
        # alongside it, so the two cannot drift apart.
        cur.execute(f"""
            CREATE TABLE catena_passages (
                idx     integer PRIMARY KEY,
                chapter text,
                body    text NOT NULL,
                fts     tsvector GENERATED ALWAYS AS (to_tsvector('english', body)) STORED,
                vec     vector({width})
            )""")
        with cur.copy("COPY catena_passages (idx, chapter, body, vec) FROM STDIN") as copy:
            for chunk, vector in zip(rows, vectors):
                copy.write_row((chunk["idx"], chunk["chapter"], chunk["text"], str(vector)))
        cur.execute("CREATE INDEX ON catena_passages USING gin (fts)")
        cur.execute("CREATE INDEX ON catena_passages USING hnsw (vec vector_cosine_ops)")
    conn.commit()


def by_meaning(conn, vector: list[float], limit: int = 10) -> list[dict]:
    """Nearest in the embedding space.

    `<=>` is cosine distance, so smaller is nearer. This finds a passage
    about the fall of Rome for a query about decline, sharing no words.
    """
    with conn.cursor() as cur:
        cur.execute(
            """SELECT idx, regexp_replace(left(body, 68), '\\s+', ' ', 'g')
                 FROM catena_passages ORDER BY vec <=> %s::vector LIMIT %s""",
            (str(vector), limit),
        )
        return [
            {"v": 1, "id": str(idx), "channel": "vec", "rank": at, "score": 0.0, "text": body}
            for at, (idx, body) in enumerate(cur.fetchall(), start=1)
        ]


def by_word(conn, query: str, limit: int = 10) -> list[dict]:
    """Nearest by the words themselves.

    Finds the passage that actually says the word where the vector search
    drifted to a paraphrase, and finds nothing at all where the words are
    not in the book — which is a useful thing to be told.
    """
    with conn.cursor() as cur:
        cur.execute(
            """SELECT idx, regexp_replace(left(body, 68), '\\s+', ' ', 'g')
                 FROM catena_passages
                WHERE fts @@ websearch_to_tsquery('english', %(q)s)
                ORDER BY ts_rank_cd(fts, websearch_to_tsquery('english', %(q)s)) DESC
                LIMIT %(n)s""",
            {"q": query, "n": limit},
        )
        return [
            {"v": 1, "id": str(idx), "channel": "fts", "rank": at, "score": 0.0, "text": body}
            for at, (idx, body) in enumerate(cur.fetchall(), start=1)
        ]


def main() -> int:
    query = sys.argv[1] if len(sys.argv) > 1 else "why did the roman empire decline"
    url = os.environ.get("DATABASE_URL")
    if not url:
        print("set DATABASE_URL", file=sys.stderr)
        return 64

    work = pathlib.Path("work")
    work.mkdir(exist_ok=True)
    book = work / "gibbon.epub"
    if not book.exists():
        print(f"fetching {GUTENBERG}")
        with urllib.request.urlopen(GUTENBERG, timeout=120) as reply:  # noqa: S310
            book.write_bytes(reply.read())

    rows = passages(book)
    print(f"== {len(rows)} passages")
    vectors = embed([" ".join(chunk["text"].split()) for chunk in rows])
    print(f"   {EMB_MODEL} returns {len(vectors[0])} numbers per passage")

    with psycopg.connect(url) as conn:
        load(conn, rows, vectors)
        print("   loaded into catena_passages")

        print(f"\n== query: {query}")
        hits = by_meaning(conn, embed([query])[0]) + by_word(conn, query)

    for channel, label in (("vec", "by meaning"), ("fts", "by word")):
        print(f"\n   {label}:")
        for hit in [h for h in hits if h["channel"] == channel][:4]:
            print(f"   {hit['rank']}. [{hit['id']}] {hit['text']}")

    # Fusion is a library call rather than a subprocess: by the time a hit
    # reaches here it is already yours, and serialising it to hand to a
    # program and back is work for nothing.
    print("\n== fused")
    text_of = {hit["id"]: hit["text"] for hit in hits}
    for hit in catena.fuse_hits(hits, limit=5):
        print(f"   {hit['rank']}. [{hit['id']}] score {hit['score']:.3f}  {text_of[hit['id']]}")

    print(RERANKING)
    return 0


RERANKING = """
   Reciprocal rank fusion scores a passage by where it placed, not by what
   it scored: 1/(k + rank) from each list that found it, added up. The two
   searches return numbers that mean different things — a cosine distance
   and a ts_rank_cd — and no scaling makes them comparable, so their ranks
   are used and their scores thrown away. A passage both searches liked
   beats one that either loved, which is the behaviour worth having.

   This is fusion, and it is often called re-ranking, but a second thing
   wears the same name. Retrieval uses a bi-encoder: passage and query are
   embedded separately, which is what lets the passage vectors be computed
   once and indexed, and is also what limits it — nothing in a passage's
   vector knows what was asked.

   A cross-encoder reads the query and the passage together and scores the
   pair. It is far more accurate and far too slow to run over a corpus, so
   it runs over the twenty or fifty candidates retrieval already found. The
   two stack: retrieve both ways, fuse the lists, then hand the top of the
   fused list to a cross-encoder for the final order. Nothing here loads a
   model, so that step is yours — the fused list is what you would hand it.
"""


if __name__ == "__main__":
    raise SystemExit(main())
