#!/bin/sh
# Passages into pgvector, then a search that asks twice and reconciles.
#
# Takes the passages example 01 wrote, embeds them, loads them into Postgres
# with a vector column and a text index, then searches both ways and fuses
# the two lists into one.
#
#     examples/shell/01-book-to-passages.sh ./work
#     examples/shell/03-search-and-rerank.sh "why did the empire decline" ./work
#
# Needs psql and jq, a Postgres with pgvector, an embedding server, and the
# catena commands on PATH:
#
#     DATABASE_URL=postgresql://user@host/db
#     EMB_HOST=http://localhost:11434     # ollama, or anything with its API
#     EMB_MODEL=nomic-embed-text          # 768 wide; bge-m3 is 1024
set -eu

query="${1:-why did the roman empire decline}"
work="${2:-./work}"
: "${DATABASE_URL:?set DATABASE_URL}"
host="${EMB_HOST:-http://localhost:11434}"
model="${EMB_MODEL:-nomic-embed-text}"

[ -s "$work/chunks.ndjson" ] || { echo "run 01-book-to-passages.sh first" >&2; exit 1; }
psql="psql -X -q -v ON_ERROR_STOP=1 $DATABASE_URL"
tab=$(printf '\t')

# How wide the model's vectors are. Asked rather than assumed, because the
# column is typed and a mismatch is a wall of errors at insert time.
dim=$(printf 'how wide is this\n' | emb --host "$host" --model "$model" | jq 'length')
echo "== $model returns $dim numbers per passage"

# One table. The vector is what the model said; the tsvector is Postgres's
# own, generated from the text so it cannot drift out of step with it.
$psql <<SQL
CREATE EXTENSION IF NOT EXISTS vector;
DROP TABLE IF EXISTS catena_passages;
CREATE TABLE catena_passages (
    idx      integer PRIMARY KEY,
    chapter  text,
    body     text NOT NULL,
    tokens   integer NOT NULL,
    fts      tsvector GENERATED ALWAYS AS (to_tsvector('english', body)) STORED,
    vec      vector($dim)
);
CREATE INDEX ON catena_passages USING gin (fts);
SQL

# Body passages only: a table of contents embeds as well as anything and
# retrieves for every query, which is worse than not having it.
jq -c 'select(.role == "body" and .tokens > 40)' "$work/chunks.ndjson" > "$work/body.ndjson"
echo "== embedding $(wc -l < "$work/body.ndjson" | tr -d ' ') passages"

# The newlines have to go before the text is a line: emb is line-oriented,
# so a passage containing one would arrive as two.
jq -r '.text | gsub("\\s+"; " ")' "$work/body.ndjson" > "$work/lines.txt"
emb --host "$host" --model "$model" --dim "$dim" < "$work/lines.txt" > "$work/vectors.ndjson"

# Line for line, which is emb's promise, so paste is a safe join.
jq -c '[.idx, .chapter, (.text | gsub("\\s+"; " ")), .tokens]' "$work/body.ndjson" > "$work/meta.ndjson"
paste -d '\t' "$work/meta.ndjson" "$work/vectors.ndjson" |
	python3 -c '
import csv, json, sys
out = csv.writer(sys.stdout, delimiter="\t", quoting=csv.QUOTE_NONE, escapechar="\\")
for line in sys.stdin:
    meta, vector = line.rstrip("\n").split("\t")
    idx, chapter, body, tokens = json.loads(meta)
    out.writerow([idx, chapter or "", body, tokens, vector])
' > "$work/rows.tsv"

$psql -c "\copy catena_passages (idx, chapter, body, tokens, vec) FROM '$work/rows.tsv'"
$psql -c "CREATE INDEX ON catena_passages USING hnsw (vec vector_cosine_ops);"
$psql -tAc "SELECT '   ' || count(*) || ' passages loaded' FROM catena_passages"

# psql's rows into hit records. `rrf` wants an id, which channel found it,
# and what rank that channel gave it; the text is carried along so the
# output is readable. Written in Python because the passages contain
# quotation marks and a shell printf would not escape them.
hits() {
	python3 -c '
import json, sys
channel = sys.argv[1]
for line in sys.stdin:
    line = line.rstrip("\n")
    if not line.strip():
        continue
    ident, rank, text = line.split("\t", 2)
    print(json.dumps({"v": 1, "id": ident, "channel": channel,
                      "rank": int(rank), "score": 0.0, "text": text}))
' "$1"
}

# ---------------------------------------------------------------------------
# Two searches. They disagree, and that is the point.
# ---------------------------------------------------------------------------
echo
echo "== query: $query"
printf '%s\n' "$query" | emb --host "$host" --model "$model" --dim "$dim" | tr -d '\n' > "$work/query.vec"
qvec=$(cat "$work/query.vec")

# Nearest by meaning. `<=>` is cosine distance, so smaller is closer, and a
# passage about "the fall of Rome" ranks even though it shares no words with
# the query.
$psql -tAF"$tab" -c "
  SELECT idx, row_number() OVER (ORDER BY vec <=> '$qvec'::vector),
         regexp_replace(left(body, 68), '\s+', ' ', 'g')
    FROM catena_passages
   ORDER BY vec <=> '$qvec'::vector
   LIMIT 10" | hits vec > "$work/hits.ndjson"

# Nearest by word. Finds the passage that says "decline" where the vector
# search drifted to a paraphrase, and finds nothing at all for a query whose
# words are not in the book.
$psql -tAF"$tab" -c "
  SELECT idx,
         row_number() OVER (ORDER BY ts_rank_cd(fts, websearch_to_tsquery('english', \$\$$query\$\$)) DESC),
         regexp_replace(left(body, 68), '\s+', ' ', 'g')
    FROM catena_passages
   WHERE fts @@ websearch_to_tsquery('english', \$\$$query\$\$)
   ORDER BY 2
   LIMIT 10" | hits fts >> "$work/hits.ndjson"

echo
echo "   by meaning:"
jq -r 'select(.channel == "vec") | "   \(.rank). [\(.id)] \(.text)"' "$work/hits.ndjson" | head -4
echo
echo "   by word:"
jq -r 'select(.channel == "fts") | "   \(.rank). [\(.id)] \(.text)"' "$work/hits.ndjson" | head -4

# ---------------------------------------------------------------------------
# Reconciling them.
# ---------------------------------------------------------------------------
echo
echo "== fused"
rrf --limit 5 < "$work/hits.ndjson" > "$work/fused.ndjson"
jq -s -r --slurpfile hits "$work/hits.ndjson" '
	($hits | INDEX(.id)) as $by_id
	| .[] | "   \(.rank). [\(.id)] score \(.score * 1000 | floor / 1000)  \($by_id[.id].text)"
' "$work/fused.ndjson"

cat <<'NOTE'

   Reciprocal rank fusion scores a passage by where it placed, not by what
   it scored: 1/(k + rank) from each list that found it, added up. The two
   searches return numbers that mean different things — a cosine distance
   and a ts_rank_cd — and no amount of scaling makes them comparable, so
   their ranks are used and their scores thrown away. A passage both
   searches liked beats one that either loved, which is the behaviour worth
   having: agreement between two different ways of looking is evidence, and
   a high score from one is only ever evidence about that one.

   The run above shows it happening. The word search's first hit is the
   front matter, which says "Roman Empire" more often than any chapter of
   the book does; the vector search never offers it. Fusion pushes it down
   and lifts the one passage both searches found.

   The constant k, 60 by default, is how much a first place is worth over a
   second. Raise it and the lists flatten toward equal weight; lower it and
   the top of each list dominates. `--weight vec=2` trusts one channel more
   than the other, which is the knob to reach for when one of them is
   consistently better on your corpus.

   This is fusion, and it is often called re-ranking, but there is a second
   thing wearing the same name. Retrieval uses a bi-encoder: the passage
   and the query are embedded separately, so the passage vectors can be
   computed once and indexed, and a search is one vector comparison against
   an index. That independence is what makes it fast and is also what
   limits it — nothing in the passage's vector knows what was asked.

   A cross-encoder reads the query and the passage together and scores the
   pair. It is far more accurate and far too slow to run over a corpus, so
   it runs over the twenty or fifty candidates retrieval already found. In
   a pipeline the two stack: retrieve both ways, fuse the lists as above,
   then hand the top of the fused list to a cross-encoder for the final
   order. Nothing here does that step — it needs a model, and these tools
   do not load models — but the fused list is what you would hand it.
NOTE
