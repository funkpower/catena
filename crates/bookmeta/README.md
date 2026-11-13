# bookmeta

The metadata a file already carries.

Part of [catena](../../README.md).

## What it does

Every ebook format has somewhere to put the title, and almost every file in a
library has something written there — by the publisher, by the converter, or
by whoever last touched it. Reading it costs nothing, needs no network, and is
the first thing a catalogue wants.

What comes back is **what the file says, not what is true**. A PDF titled
`Microsoft Word - ch3.doc` is reported with that title and a note saying where
it came from. Deciding that is rubbish is [`bibclean`](../bibclean/README.md)'s
job; finding out what is actually true is [`bibfetch`](../bibfetch/README.md)'s.

## When you would use it

- **Cataloguing a directory** you have just been given, before anything else.
- **Filling a library** without shelling out to calibre for every file.
- **Finding the books that say nothing about themselves**, which are the ones
  worth a person's attention.

## Install

```sh
cargo install --git https://git.pumpin.online/funkpower/catena bookmeta
```

## Example

```
$ bookmeta library/*.epub | jq -r '[.title, (.authors|join(", "))] | @tsv'
Hagakure - The Way of the Samurai	Yamamoto Tsunetomo
The Complete Stories	Flannery O'Connor
A Collection of Essays	George Orwell
```

The whole record:

```
$ bookmeta philokalia.epub
{"v":1,"path":"philokalia.epub","source":"opf","title":"The Philokalia, Volume 3","authors":["G. E. H. Palmer","Kallistos Ware"],"publisher":"Faber & Faber","date":"1984-09-17","language":"en","series":"The Philokalia","series_index":"3","identifiers":{"isbn":"9780571175256"},"subjects":["Orthodox Christianity"]}
```

The ones that said nothing:

```sh
bookmeta library/* | jq -r 'select(.title == null) | .path'
```

Feeding a catalogue:

```sh
bookmeta library/*.epub \
  | jq -c '{title, author: .authors[0], isbn: .identifiers.isbn}' \
  | bibclean
```

## Options

| Option | Effect |
|---|---|
| `--prompt` | Write a question about what the file did not say, instead of the record |
| `--merge` | Read a model's reply on stdin and fill in what was missing |
| `--excerpt <FILE>` | Text to show the model alongside the metadata |
| `--man` | Write this page as a manual page, in roff |

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Every file said something |
| `1` | At least one said nothing |
| `64` | The command line was wrong |
| `65` | A model's reply could not be read |
| `74` | A file could not be read |

## Where it reads from

| Format | Where |
|---|---|
| EPUB | The package document named by `META-INF/container.xml` — Dublin Core, plus the `<meta>` entries Calibre adds for a series |
| PDF | The trailer's information dictionary |
| MOBI, AZW, AZW3 | The MOBI header in the first record, and the EXTH records after it |

An EPUB whose container is malformed falls back to the first `.opf` in the
archive, because a converter that wrote a bad container often still wrote a
usable package.

Identifiers come back under the scheme they are: `urn:isbn:…` and a bare ISBN
both become `isbn`, a UUID becomes `uuid`, anything else `other`. PDF dates
are reduced to the day where they parse and kept verbatim where they do not,
because a date nobody can read is still evidence.

A Kindle book's title is EXTH 503 where it is there, then the MOBI header's
full name, then the Palm database name (cut to 31 bytes, with a note). Authors
(100) and subjects (105) are every record of their kind, in order; publisher
(101), description (103), date (106) and language (524) the first. EXTH 104 is
an `isbn` when it is shaped like one and `other` when not, and 113 is an
`asin`. A contributor (108) is not an author, the same as an EPUB's
`dc:contributor`, and is not reported. With no EXTH language, the language
comes from the header's locale. Strings are decoded as the header says,
UTF-8 or Windows-1252. A book whose text is encrypted still has its header
read, and carries the note `drm`.

Every offset in a Kindle book is checked before it is followed: headers that
are cut short or point outside the file give an empty record with a note, and
an EXTH block that breaks off keeps the records before the break.

## Covers

`Metadata::cover` is the cover image the file carries, with its media type
read off its first bytes: JPEG, PNG, GIF, WebP or BMP. It is for library
callers and is not part of the JSON record, which is a line of text.

A Kindle book's cover is EXTH 201, or the thumbnail, 202, when no cover is
named, and with neither the first image record.

An EPUB's is the first of these that reaches an image:

1. The manifest item whose `properties` include `cover-image` (EPUB 3).
2. The item `<meta name="cover" content="…">` names (EPUB 2). Some
   converters write the image's address there rather than its id, and that is
   followed too.
3. The guide's `<reference type="cover">`, a page, and the first `<img src>`
   or SVG `<image href>` on it. A guide that points straight at an image is
   taken at its word.
4. Any image in the manifest whose id or address says `cover`.

A route that names something unusable — not in the archive, not an image —
leaves a note, and the next route is asked. Addresses are resolved against the
document they are written in, with their fragment off and their
percent-escapes undone, and one that climbs out of the archive or names a
scheme (`http:`, `data:`) is not followed.

What the bytes say beats what the manifest says: a JPEG listed as
`image/png` comes back as `image/jpeg`, with a note. A cover larger than
`bookmeta::MAX_COVER` (16 MiB) is left out with a note, in either format, and
an EPUB's is never decompressed further than that, whatever size the archive
claims for it.

## Asking a model

Some files say nothing useful. `--prompt` writes the question and **sends
nothing** — where it goes is your business, your model, your machine:

```
$ bookmeta --prompt --excerpt opening.txt scan.pdf
You are cataloguing a book. Below is what its file says about itself, and the
opening of the text where one was available.

Known:
  path: scan.pdf
  title: Microsoft Word - ch3.doc

Missing: authors, publisher, date, language
…
Reply with one JSON object and nothing else. … Use null for anything the text
does not settle — a guess is worse than an absence, because nobody downstream
can tell them apart.
```

Then put it wherever you like and hand the answer back:

```sh
bookmeta --prompt scan.pdf | ollama run qwen3.5:9b | bookmeta --merge scan.pdf
```

Four things make this safe to depend on:

- **Only what was missing is filled.** A model that volunteers a better title
  for a book whose title was already known is overruled: what the file says is
  evidence, what a model says is a guess.
- **A null is an absence**, not a value. So is an empty string.
- **The prompt is versioned** and printed where you can read it. A record
  filled this way carries a note saying which prompt version did it.
- **A reply that is not one JSON object is refused**, not half-applied. Prose
  and code fences around the object are fine.

Nothing here reaches the network, so this tool keeps [the contract](../../docs/contract.md)
whole: same input, same output, every run and every machine.

## As a library

```rust
let meta = bookmeta::read(std::path::Path::new("philokalia.epub"));
assert_eq!(meta.title.as_deref(), Some("The Philokalia, Volume 3"));

if let Some(prompt) = bookmeta::prompt::compose(&meta, Some(opening)) {
    // send it wherever you like, then:
    // bookmeta::prompt::merge(&mut meta, &reply)?;
}
```

`bookmeta::epub`, `bookmeta::pdf` and `bookmeta::mobi` read one format each, and
`Metadata::missing` says which fields a record still wants.

## See also

- [`booksniff`](../booksniff/README.md) — what the file actually is.
- [`bibclean`](../bibclean/README.md) — whether what it says is any good.
- [`bibfetch`](../bibfetch/README.md) — what a register says instead.
