# Recipes

Each tool here does one thing, which is only useful because they compose. This
is what the joins look like in practice — including the parts that are not in
this suite, because a real pipeline has a database at the end of it.

- [A shelf of files, sorted before anything else touches it](#a-shelf-of-files-sorted-before-anything-else-touches-it)
- [A scanned book into searchable passages](#a-scanned-book-into-searchable-passages)
- [A collected edition into one record per work](#a-collected-edition-into-one-record-per-work)
- [A podcast archive into passages you can cite to the second](#a-podcast-archive-into-passages-you-can-cite-to-the-second)
- [Chapter marks for a long recording](#chapter-marks-for-a-long-recording)
- [Passages into pgvector](#passages-into-pgvector)
- [Hybrid search, fused](#hybrid-search-fused)
- [A catalogue that has drifted](#a-catalogue-that-has-drifted)
- [A video library that plays on the thing you carry](#a-video-library-that-plays-on-the-thing-you-carry)
- [Notes on composing these](#notes-on-composing-these)

## A shelf of files, sorted before anything else touches it

Work out what you actually have. Extensions lie — a `.pdf` that is really a
DjVu will fail four steps later, in a way that looks like a bug in the OCR.

```sh
booksniff library/* > manifest.ndjson
```

What is not what it claims:

```sh
jq -r 'select(.kind != .declared_ext) | "\(.path): a \(.kind)"' manifest.ndjson
```

What is too small to be a book, and probably a download that failed:

```sh
jq -r 'select(.notes[]? | test("too small")) | .path' manifest.ndjson
```

Split the shelf by what each file needs:

```sh
jq -r 'select(.kind == "pdf")  | .path' manifest.ndjson > pdfs.txt
jq -r 'select(.kind == "epub") | .path' manifest.ndjson > epubs.txt
```

## A scanned book into searchable passages

The long one. Five decisions, each of which is expensive to get wrong.

**1. Is it two pages photographed as one?** If it is and you skip this, the OCR
interleaves the two columns line by line and produces text that reads like
prose and is not.

```sh
for f in $(cat pdfs.txt); do
  pdfgutter --quiet "$f" && pdfgutter --quiet --into "flat/$(basename "$f")" "$f" \
                         || cp "$f" "flat/$(basename "$f")"
done
```

**2. Is the text layer worth having?** OCR costs GPU hours. A PDF that already
carries good text needs none; one whose text layer is mojibake needs it badly,
and the mojibake is invisible until you read a retrieved passage.

```sh
for f in flat/*.pdf; do
  if pdftriage --quiet "$f"; then
    pdftotext -layout "$f" "text/$(basename "$f" .pdf).txt"
  else
    ocrmypdf --redo-ocr "$f" "ocr/$(basename "$f")"   # not part of this suite
  fi
done
```

`pdftriage` exits `0` when the layer is fine, `1` when it needs OCR, and `2`
when it could not tell — treat `2` as needing a human, not as a `1`.

**3. Convert to markdown.** Whatever you like: `pandoc`, `marker`, `pdftotext`
and a script. Nothing here cares, as long as what comes out is markdown with
`<!-- Page 41 -->` markers where the pages were.

**4. Clean it.** Conversion leaves residue — stray HTML, control characters,
headings that are not headings.

```sh
mdscrub --pages raw/book.md > clean/book.md
```

**5. Cut it into passages.**

```sh
chunkmd --target 512 --overlap 64 clean/book.md > chunks/book.ndjson
```

The whole of it, for one book:

```
$ pandoc -t commonmark chapter.xhtml | mdscrub | chunkmd --min-tokens 10
{"v":1,"idx":0,"text":"The obligation ran both ways, and contemporaries said so plainly whenever they were asked. A lord who could not protect was a lord who could not command, and the rolls record the consequences in the plainest possible terms across three generations of tenure disputes.","tokens":50,"page":41,"chapter":"Chapter One","role":"body","start_time":null,"end_time":null,"flags":[]}
```

Every chunk carries the page it came from and the chapter it sits under, which
is what lets a search result cite itself.

## A collected edition into one record per work

Forty books in one file, indexed whole, means a query for any of them returns a
passage that could be from any other — and the title on the result names the
collection rather than the work.

```sh
mdsplit --into sections/ complete-works.md
for f in sections/*.md; do
  mdscrub "$f" | chunkmd > "chunks/$(basename "$f" .md).ndjson"
done
```

`mdsplit` exits `1` when the document is one work rather than a collection,
which is the usual answer and not a failure:

```sh
mdsplit --into sections/ book.md || cp book.md sections/000-book.md
```

Scripture is recognised and grouped by book rather than by chapter, because
every chapter there is its own heading and splitting on those gives a thousand
fragments instead of sixty-six works.

## A podcast archive into passages you can cite to the second

```sh
for f in transcripts/*.srt; do
  cuenorm "$f" | cuepack --seconds 75 > "chunks/$(basename "$f" .srt).ndjson"
done
```

```
$ cuenorm episode.srt | cuepack --seconds 5
{"v":1,"idx":0,"text":"the mill stood at the northern end","tokens":7,"page":null,"chapter":null,"role":"body","start_time":1.0,"end_time":4.0,"flags":[]}
{"v":1,"idx":1,"text":"of the valley where the road turned","tokens":7,"page":null,"chapter":null,"role":"body","start_time":4.0,"end_time":8.0,"flags":[]}
```

The same `chunk` record a book produces, with times where a book has pages. So
everything downstream — embedding, storage, retrieval — is the same code.

`cuepack` also reads a subtitle file directly, so `cuenorm` is only needed when
the input is in a format `cuepack` would otherwise have to guess at, or when
you want the cue stream for something else.

## Chapter marks for a long recording

```sh
cuenorm episode.vtt | cueseg --duration 7200 > chapters.ndjson
```

One `segment` per topic, with a title and an excerpt — the marks a player
needs, and the spans to hand a summariser one at a time.

## Passages into pgvector

`emb` turns lines into vectors. It is the only tool here that touches the
network, and it never loads a model: the model lives behind a server where its
GPU is somebody else's problem.

```sh
jq -r '.text | @json' chunks/book.ndjson | jq -r 'fromjson' \
  | emb --model nomic-embed-text --dim 768 > vectors.ndjson
```

Output lines correspond to input lines one for one, so the two files zip
together by position:

```sh
paste -d'\t' \
  <(jq -c '{text, page, chapter, start_time, end_time}' chunks/book.ndjson) \
  vectors.ndjson > rows.tsv
```

The table:

```sql
CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE passage (
    id         bigserial PRIMARY KEY,
    source     text        NOT NULL,
    idx        int         NOT NULL,
    body       text        NOT NULL,
    page       int,
    chapter    text,
    start_time double precision,
    end_time   double precision,
    embedding  vector(768) NOT NULL,
    fts        tsvector GENERATED ALWAYS AS (to_tsvector('english', body)) STORED
);

CREATE INDEX ON passage USING hnsw (embedding vector_cosine_ops);
CREATE INDEX ON passage USING gin  (fts);
```

Loading it, with `psql`:

```sh
psql -c "\copy passage (source, idx, body, page, chapter, start_time, end_time, embedding)
         FROM 'rows.csv' WITH (FORMAT csv)"
```

`vector` accepts its text form — `[0.1,0.2,…]` — which is exactly what `emb`
writes, so the JSON array goes into the column as it stands.

Two indexes because two kinds of question: `hnsw` answers "what is this
about", `gin` answers "who said this exact phrase". Neither is good at the
other's job, which is what the next recipe is for.

## Hybrid search, fused

Ask both indexes, then fuse the rankings rather than the scores — a cosine
distance and a `ts_rank` are not on the same scale and never will be.

```sql
-- the vector channel
SELECT id, row_number() OVER (ORDER BY embedding <=> $1) AS rank,
       1 - (embedding <=> $1) AS score
FROM passage ORDER BY embedding <=> $1 LIMIT 50;

-- the text channel
SELECT id, row_number() OVER (ORDER BY ts_rank(fts, q) DESC) AS rank,
       ts_rank(fts, q) AS score
FROM passage, websearch_to_tsquery('english', $2) q
WHERE fts @@ q ORDER BY 2 LIMIT 50;
```

Turn each into `hit` records — `channel` of `vec` and `fts` — and fuse:

```
$ cat vec.ndjson fts.ndjson | rrf -n 2
{"v":1,"id":"c81a","channel":"vec","rank":1,"score":0.032266458495966696,"payload":{}}
{"v":1,"id":"d42f","channel":"fts","rank":2,"score":0.01639344262295082,"payload":{}}
```

The two streams can be interleaved and in any order — each hit carries the rank
its own channel gave it, so nothing has to be grouped first.

Weight a channel you trust more:

```sh
cat vec.ndjson fts.ndjson | rrf --weight vec=2 --weight fts=1 -n 20
```

Put anything you want carried through in `payload` and it comes out untouched,
which saves a second round trip to fetch the text of what you just ranked.

## A catalogue that has drifted

```sh
bibclean --flagged catalogue.ndjson > repairs.ndjson
```

Every change comes with a reason, so the output is reviewable rather than
something you have to trust.

Headings accumulate spellings — the same author entered three ways, a subject
that grew a bracket:

```sh
vocabmerge --kind author authors.ndjson > merges.ndjson
vocabmerge --kind tag    tags.ndjson    >> merges.ndjson
```

Apply only what you believe:

```sh
jq -r 'select(.action == "merge" and .confidence >= 0.9)
       | "UPDATE book SET author_id = \(.keep_id) WHERE author_id = \(.drop_id);"' \
  merges.ndjson | psql
```

## A video library that plays on the thing you carry

```sh
for f in library/*.webm library/*.mkv; do
  plan=$(avplan "$f") && eval "$plan"
done
```

`avplan` writes nothing for a file that already plays and exits non-zero, so
`&&` skips it — you convert the six files that need it rather than the ninety
in the directory. An already-H.264 picture is copied rather than re-encoded,
which is the difference between forty minutes and forty seconds.

To see why before committing to it:

```sh
avplan --json talk.mkv | jq -r '.reasons[]'
```

## Notes on composing these

**Exit codes are the control flow.** `0` yes, `1` no, `2` could not tell. That
is what makes `&&` and `||` work as the branching in every loop above. Anything
`64` and over is a failure rather than an answer, so a script that treats
non-zero as "no" will eventually treat a broken file as a healthy one — test
for the code you mean.

**NDJSON goes through pipes and into files equally well.** Every tool writes
one record per line and flushes as it goes, so a slow stage downstream starts
work on the first record rather than the last.

**Nothing here has state.** Run any of it twice, on any machine, in any order;
parallelise a loop with `xargs -P` without coordinating anything. The tools
that call another program say so in `--help` and exit `69` when it is missing,
rather than guessing.

**Keep the records.** The chunk that went into the database is the audit trail
for the passage that comes out of it. Storage is cheaper than re-running OCR.

## See also

- [What these programs promise](contract.md) — the rules all of them keep.
- [The records](records.md) — the shapes that travel between them.
