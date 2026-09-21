# Recipes

Each tool here does one thing, which is only useful because they compose. This
is what the joins look like in practice — including the parts that are not in
this suite, because a real pipeline has a database at the end of it.

- [A shelf of files, sorted before anything else touches it](#a-shelf-of-files-sorted-before-anything-else-touches-it)
- [A catalogue out of what the files already say](#a-catalogue-out-of-what-the-files-already-say)
- [Shelving a library by what its books printed](#shelving-a-library-by-what-its-books-printed)
- [Asking a model, without this suite calling one](#asking-a-model-without-this-suite-calling-one)
- [A scanned book into searchable passages](#a-scanned-book-into-searchable-passages)
- [A collected edition into one record per work](#a-collected-edition-into-one-record-per-work)
- [A podcast archive into passages you can cite to the second](#a-podcast-archive-into-passages-you-can-cite-to-the-second)
- [Chapter marks for a long recording](#chapter-marks-for-a-long-recording)
- [Passages into pgvector](#passages-into-pgvector)
- [Hybrid search, fused](#hybrid-search-fused)
- [A catalogue that has drifted](#a-catalogue-that-has-drifted)
- [A bibliography that has been copied between formats](#a-bibliography-that-has-been-copied-between-formats)
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

## A catalogue out of what the files already say

Before reaching for a network or a model, read what is already there. Every
ebook format has somewhere to put the title, and almost every file has
something written in it.

```
$ bookmeta library/*.epub | jq -r '[.title, .authors[0], (.identifiers.isbn // "—")] | @tsv'
A Collection of Essays          George Orwell     —
Adventures of Huckleberry Finn  Mark Twain        9781593081126
Bloodline (Star Wars)           Claudia Gray      9781101885253
The Pale-Faced Lie              David Crow        9780997487176
Finding Vigano                  Robert Moynihan   —
```

That is free and offline. Three things build on it.

**The ISBNs it found are worth confirming**, because a publisher's own file is
not evidence the book exists as catalogued:

```sh
bookmeta library/*.epub \
  | jq -r 'select(.identifiers.isbn) | .identifiers.isbn' \
  | idnorm --quiet | bibfetch --polite you@example.org \
  | jq -r 'select(.found | not) | "unknown to any register: \(.id)"'
```

**The books that said nothing** are the ones worth a person's attention, and
the only ones worth a model's time:

```sh
bookmeta library/* | jq -r 'select(.title == null) | .path' > silent.txt
```

**Real chapter titles** come from the book rather than from a converter's
guess:

```
$ tocfind enlightenment.epub | jq -r 'select(.depth == 0) | .title'
1. Introduction: Radical Enlightenment and “Modernity” (1650–1850)
Part I: The Origins of Democratic Modernity
Part II: Human Rights and Revolution (1770–1830)
Part III: Revolution and Competing Revolutionary Ideologies (1789–1830)
```

And the language, which decides which OCR to run and which model to embed
with:

```sh
langid converted/*.md | jq -r 'select(.language != "en") | [.path, .name] | @tsv'
```

## Shelving a library by what its books printed

A catalogue's subjects come from wherever the file came from, which is to say
from nowhere in particular. A book's own cataloguing block is the one place its
subjects are stated rather than guessed — and stated in Library of Congress
form, which is precise and useless for browsing:

```
$ colophon converted/hobbes.md | jq -r '.subjects[]'
Hobbes, Thomas, 1588–1679. Leviathan
Political science – Early works to 1800
```

Nobody has a shelf called that. Give `colophon` the list you actually file by
and it files them for you:

```
$ cat shelf.txt
Political Science = political science, politics
Byzantine History = byzantium, byzantine, constantinople
Theology = theology, christianity
Ethics

$ colophon --shelf shelf.txt converted/*.md | jq -c '{title, shelved, unshelved}'
{"title":"Leviathan","shelved":["Political Science"],"unshelved":["Hobbes, Thomas, 1588–1679. Leviathan"]}
{"title":"Christianity and Classical Culture","shelved":["Theology"],"unshelved":[]}
```

`unshelved` is the point of the exercise. A subject quietly dropped reads
exactly like a book that printed none, and the difference decides whether you
go looking for a heading your shelf is missing.

**The title page is not evidence on its own.** A scan that begins mid-frontmatter
gives up a running head, a copyright line, or the author's name where the title
should be. What makes a printed record trustworthy is a second one agreeing,
and the ISBN in the same block is how you get one:

```sh
colophon --shelf shelf.txt converted/*.md > printed.ndjson

jq -r 'select(.isbns != null) | .isbns[0]' printed.ndjson \
  | idnorm --quiet \
  | bibfetch --polite you@example.org > register.ndjson
```

Now two sources that cannot have copied each other have spoken, and you can
say which books are settled and which need a person:

```sh
jq -s -r '
  (.[1] | map({key: .id, value: .}) | from_entries) as $reg
  | .[0][]
  | . as $printed
  | ($reg[$printed.isbns[0] // ""] // {}) as $found
  | [ $printed.title,
      $found.title,
      (if $printed.title and $found.title then "corroborated" else "one source" end)
    ] | @tsv
' printed.ndjson register.ndjson
```

```
The First World War    The First World War: A Very Short Introduction   corroborated
Document5              The Problems of Philosophy                       corroborated
Born Fighting          —                                                one source
```

The second row is the case worth the whole recipe: a file named `Document5`
whose author reads `-ducati996`, which the book itself and OpenLibrary agree
is Russell's *The Problems of Philosophy*. The third is the honest failure —
one source said something, nothing confirmed it, and it goes to a human.

In process, where a pipeline actually lives:

```python
import catena

shelf = open("shelf.txt").read()
printed = catena.read_colophon(markdown, shelf=shelf)

found = None
if printed["isbns"]:
    found = catena.fetch_identifier(printed["isbns"][0], polite="you@example.org")

settled = bool(printed.get("title") and found and found.get("title"))
```

`catena.shelve(shelf, subjects)` files headings a catalogue already holds, by
the same rules, so a record's existing subjects and its book's printed ones are
judged alike.

## Asking a model, without this suite calling one

A tool here may compose a prompt. It does not send one. That keeps every
answer the same on every run and every machine, and leaves the model, the
machine and the GPU yours.

```sh
bookmeta --prompt --excerpt opening.txt scan.pdf \
  | ollama run qwen3.5:9b \
  | bookmeta --merge scan.pdf
```

Over the shelf, asking only about the files that said nothing:

```sh
while read -r f; do
  pdftotext -l 3 "$f" - > /tmp/opening.txt
  bookmeta --prompt --excerpt /tmp/opening.txt "$f" \
    | ollama run qwen3.5:9b \
    | bookmeta --merge "$f"
done < silent.txt > filled.ndjson
```

Four things make that safe to depend on. Only the fields the file left blank
are filled — a model does not get to overrule what the publisher wrote. A
`null` is an absence, not a value. The prompt is versioned, and a record
filled this way says which version did it. And a reply that is not one JSON
object is refused rather than half-applied.

What comes back is still a claim, so treat it as one:

```sh
jq -r 'select(.notes[]? | test("from a model")) | .path' filled.ndjson
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

`pdftriage` exits `0` when the layer is fine, `1` when it needs work, and `2`
when it could not tell — treat `2` as needing a human, not as a `1`.

But "needs work" is four different situations, and OCR is the wrong answer to
one of them. `--json` says which:

```sh
for f in flat/*.pdf; do
  case "$(pdftriage --json "$f" | jq -r .text_layer)" in
    healthy)
      pdftotext -layout "$f" "text/$(basename "$f" .pdf).txt" ;;
    reflected_font)
      # The one verdict where OCR is the wrong answer: every character is
      # already in the file, permuted by a non-standard font encoding, and
      # undoing the permutation recovers the text exactly. `pdftriage`'s
      # `reflect::undo` does that in process; from a shell today it is still
      # an OCR, which is an hour of GPU time to recover text the file holds.
      ocrmypdf --force-ocr "$f" "ocr/$(basename "$f")" ;;
    absent)
      # A scan, with nothing to preserve. The default refuses a file that
      # already has text, which is what you want here.
      ocrmypdf "$f" "ocr/$(basename "$f")" ;;
    garbled|sparse)
      # --force-ocr, not --redo-ocr. The text layer looks fine to a machine
      # and is mojibake, so anything that "keeps the text it judges good"
      # keeps precisely the text that is wrong.
      ocrmypdf --force-ocr "$f" "ocr/$(basename "$f")" ;;
  esac
done
```

`ocrmypdf` and `tesseract` are not part of this suite and will not be. A
trained OCR model and a PDF rebuilt around an invisible text layer are not a
filter's work; deciding *whether* to call one, and with which flag, is.

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

## What a book leans on

The bibliography is the one part of a book that is already structured. Every
entry names a work, and most name it in one of a handful of shapes that have
not changed in a century — so `citefind` reads it without asking anybody
anything.

```
$ citefind bibliography.md | head -1
{"v":1,"idx":0,"text":"Abogunrin, S. O. “The Three Variant Accounts of Peter’s Call: A Critical and Theological Examination of the Texts.” *New Testament Studies* 31 (1985): 587–602.","marker":null,"authors":["Abogunrin, S. O."],"year":1985,"title":"The Three Variant Accounts of Peter’s Call: A Critical and Theological Examination of the Texts","container":"New Testament Studies","pages":"587–602","ids":[],"kind":"article","continuation":false,"unsettled":[]}
```

Whose work a book rests on, counted:

```sh
citefind bibliography.md \
  | jq -r 'select(.authors | length > 0) | .authors[0]' \
  | sort | uniq -c | sort -rn | head
```

```
  64 Yates
  26 Quigley
  23 Bulgakov, Sergius
  17 Lorenz, K.
```

What it reads, and what it reaches for:

```sh
citefind bibliography.md | jq -r .kind | sort | uniq -c | sort -rn
```

```
4200 other
3356 book
 808 chapter
 700 article
 300 web
```

The identifiers it finds, checked and resolved against a register:

```sh
citefind bibliography.md \
  | jq -r '.ids[]? | select(.kind == "doi") | .id' \
  | idnorm | bibfetch
```

### Across a whole shelf

Every book's bibliography, cut out of the converted markdown and read:

```sh
for f in converted/*.md; do
  awk '/^#+ *(Bibliography|References|Works Cited|Notes)/{on=1} on' "$f" \
    | citefind 2>/dev/null \
    | jq -c --arg from "$(basename "$f" .md)" '. + {citing: $from}'
done > citations.ndjson
```

Over thirty converted books that was 14,481 references, 10,536 of them naming
a work, in about a second. A book whose heading matched but whose section was
empty exits `1` and says so on stderr, which is why the loop sends it
nowhere.

Which works the shelf agrees on — cited by the most different books:

```sh
jq -r 'select(.title) | [.citing, (.title | ascii_downcase)] | @tsv' citations.ndjson \
  | sort -u | cut -f2 | uniq -c | sort -rn | head
```

Counted per citing book rather than per entry, so a work cited forty times in
one bibliography still counts once. On a shelf whose books share a subject
this is the spine of the field; on one that does not, it is mostly ones.

That is a citation graph, built out of the books themselves, with nothing
running but a parser.

### What it could not read

Every record names the fields it could not settle. A bibliography set in a
style the parser does not know, or a chunk that began part-way through an
entry, shows up here rather than as a confident wrong answer:

```sh
citefind bibliography.md | jq -r 'select(.unsettled | length > 0) | .text'
```

`--prompt` writes a question about exactly those entries and no others, for a
model of your choosing. Nothing is sent from here:

```sh
citefind --prompt bibliography.md | ollama run qwen3:8b > reply.json
citefind --merge reply.json bibliography.md
```

A reply fills in only the fields that were asked about; a model that
volunteers a better title for an entry whose title was read off the page is
overruled. Most bibliographies have nothing to ask about, and `--prompt`
exits `1` and writes nothing when that is so.

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

Series and volume, which most catalogues have on almost nothing:

```sh
seriesof catalogue.ndjson \
  | jq -r 'select(.volume) | "UPDATE book SET series = \(.series|@json), series_index = \(.volume) WHERE title = \(.title|@json);"' \
  | psql
```

Apply only what you believe:

```sh
jq -r 'select(.action == "merge" and .confidence >= 0.9)
       | "UPDATE book SET author_id = \(.keep_id) WHERE author_id = \(.drop_id);"' \
  merges.ndjson | psql
```

## A bibliography that has been copied between formats

Identifiers arrive wrapped in whatever the last tool left on them, and a
catalogue that stores them as they came cannot tell two spellings of one DOI
apart.

```sh
idnorm --quiet refs.txt > ids.ndjson
```

The ones that cannot be right — an ISBN or an ISSN whose check digit fails:

```sh
jq -r 'select(.checksum == false) | .input' ids.ndjson
```

The ones that are well formed and still wrong, which needs a register:

```sh
bibfetch --polite you@example.org ids.ndjson \
  | jq -r 'select(.found | not) | "missing: \(.id)"'
```

That second question is a different one, which is why it is a different tool:
`idnorm` never touches the network and `bibfetch` does nothing else.

Filling in what you only had the identifier for:

```sh
idnorm --quiet refs.txt | bibfetch \
  | jq -r 'select(.found) | "\(.year)\t\(.authors | join(", "))\t\(.title)"'
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
