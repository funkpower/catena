# bibfetch

What a catalogue says about an identifier, asked of the body that assigns it.

Part of [catena](../../README.md).

## What it does

[`idnorm`](../idnorm/README.md) can tell you a DOI is well formed. It cannot
tell you the paper exists — that is not a fact about the string, it is a fact
about the world, and the only way to learn it is to ask somebody who keeps a
register.

So this asks, and that is the only thing it does.

## When you would use it

- **Auditing a bibliography.** A well-formed DOI that no register has is a
  mistyped one, and a catalogue can carry it for years without noticing.
- **Filling in a reference** you have only the identifier for.
- **Before citing something**, to check the year and the authors are what you
  think they are.
- **Cataloguing a book that came with no identifier**, to find the editions a
  register has under its title, and an ISBN to file it by.

## Install

```sh
cargo install --git https://git.pumpin.online/funkpower/catena bibfetch
```

## Example

```
$ printf '10.1093/mind/LIX.236.433\narXiv:2401.01234\n978-0-306-40615-7\n' | idnorm | bibfetch --polite you@example.org
{"v":1,"id":"10.1093/mind/lix.236.433","kind":"doi","found":true,"source":"crossref","title":"I.—COMPUTING MACHINERY AND INTELLIGENCE","authors":["A. M. TURING"],"year":1950,"container":"Mind"}
{"v":1,"id":"2401.01234","kind":"arxiv","found":true,"source":"arxiv","title":"Mixture cure semiparametric additive hazard models under partly interval censoring -- a penalized likelihood approach","authors":["Jinqing Li","Jun Ma"],"year":2024,"container":"arXiv","summary":"Survival analysis can sometimes involve individuals who will not experience…"}
{"v":1,"id":"9780306406157","kind":"isbn","found":true,"source":"openlibrary","title":"Error-correction coding for digital communications","year":1981,"container":"Plenum Press"}
```

The ones nobody has:

```
$ printf '10.9999/definitely-not-a-real-doi\n' | idnorm | bibfetch
{"v":1,"id":"10.9999/definitely-not-a-real-doi","kind":"doi","found":false,"source":"crossref"}
$ echo $?
1
```

Absence is the useful answer here. It is what tells a catalogue that a DOI it
has been carrying for three years was mistyped on the way in.

Auditing a whole bibliography:

```sh
idnorm --quiet refs.txt | bibfetch --polite you@example.org \
  | jq -r 'select(.found | not) | "missing: \(.id)"'
```

## Options

| Option | Effect |
|---|---|
| `--polite <EMAIL>` | An address to put in the request. The registers ask for one and answer faster when they get it |
| `--timeout <SECONDS>` | How long to wait on a register. Default `20` |
| `--attempts <COUNT>` | How many times to ask before believing a failure. Default `3` |
| `--man` | Write this page as a manual page, in roff |

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Every identifier was found |
| `1` | At least one was not |
| `64` | The command line was wrong |
| `65` | The input was not identifier records |
| `69` | A register could not be reached |
| `74` | The file could not be read |

Exit `1` is an answer — the register replied and does not have it. Exit `69`
is a failure — nobody replied, and the identifier may be perfectly good.

## The registers

| Kind | Asked of | Why that one |
|---|---|---|
| `doi` | Crossref | It assigns most scholarly DOIs |
| `arxiv` | the arXiv | Its own register of preprints |
| `isbn` | OpenLibrary's edition record, then its index, then Google Books | Books, without a key |

One register is enough for a DOI or a preprint: the body that assigns the
identifier is the only one that has it. A book is different. An ISBN is
assigned by a publisher and recorded by whoever gets round to it, so an
answer that names the book but not who wrote it is common — OpenLibrary's
edition record names its authors by key rather than by name, and its index
names them. So the registers are asked in turn and the first complete answer
wins. An incomplete one is kept in case nothing better comes, and a register
being down is a failure only when every register is.

Google Books is asked last because it answers anonymous callers only until a
daily quota runs out, after which it replies `429` to everyone sharing the
address.

An **ISSN** has no register that answers without a key, so it is not looked up
on its own. A journal's ISSN comes back on the works published in it instead.

A failure is retried, waiting a second and then two: long enough for a
register that is rate-limiting, short enough that one which is simply down
does not hold up a pipeline. An answer this cannot read is not retried,
because it will not read any better next time.

## Covers

```sh
bibfetch cover 978-0-306-40615-7 -o cover.jpg
```

Fetches a book's cover from Open Library's covers store by its ISBN and
writes the image. The thirteen-digit form is asked first and the ten-digit
form second, because a book is often filed under only one of them; an ISBN
whose check digit is wrong is asked about as written, since converting it
would give another book's number. `--cover-id` names an Open Library cover id
to ask by last, which is what the `cover_id` field of a lookup record holds:
the first cover in Open Library's record of that edition, never the cover its
index picked for the whole work, which is often another edition's. The cover
id is asked only when every form of the ISBN asked was answered with no cover; if
asking by ISBN failed, that failure is the result.

What comes back is checked before it is written: a JPEG or PNG by its bytes
as well as by what the server says, no larger than ten megabytes, reached in at
most three redirects, each over HTTPS to `covers.openlibrary.org`,
`archive.org` or a host under `archive.org`, and never to an address by number.
One request is made at a time, a `429` or `503` is waited out for as long as
its `Retry-After` says, up to a minute, and the whole fetch, every attempt and
redirect included, stops after two minutes. The two minutes start once it is
the fetch's turn, so a fetch queued behind others in the same process can wait
that long again first.

Open Library allows a hundred covers by ISBN every five minutes from one
address, and refuses the rest with `403`. Covers asked by ISBN are kept three
seconds apart across the whole process (`Covers::pace`), which stays under that
limit however long a run goes. A refusal all the same is not mistaken for an
outage: the other form of the ISBN is not asked, the cover id is, because
Open Library does not limit those, and with no cover found that way the fetch
fails with `Error::Limited`, which means ask again later.

| Option | Effect |
|---|---|
| `-o`, `--output <FILE>` | Where to write the image |
| `--size <SIZE>` | `small`, `medium` or `large`. Default `large` |
| `--cover-id <ID>` | An Open Library cover id to ask by when the ISBN has none |

Exit `0` the cover was written, `1` Open Library has none, `65` the argument
was not an ISBN or the answer was not an image, `69` Open Library could not be
reached, `74` the file could not be written, `75` Open Library refused for
being asked too often and will answer later.

From Rust, `bibfetch::cover(&agent, isbn, cover_id, size)` returns
`Ok(None)` when there is no cover, and a [`Cover`](src/cover.rs) with the
bytes, the media type and the address asked when there is.

## Searching by title

```
$ bibfetch search --title 'The First World War' --author Howard --limit 2
{"v":1,"id":"9780192853622","kind":"isbn","found":true,"source":"openlibrary","title":"The First World War","authors":["Michael Eliot Howard"],"year":2002,"container":"Oxford University Press","subjects":["World War, 1914-1918"],"language":"eng","cover_id":1123124}
{"v":1,"id":"9780199205592","kind":"isbn","found":true,"source":"openlibrary","title":"The First World War: A Very Short Introduction","authors":["Michael Howard"],"year":2007,"container":"Oxford University Press","language":"eng","cover_id":14606475}
```

A book that arrives as a file often has no identifier to ask about, only a
title and perhaps a name. This asks which books a register has under those,
and prints one record per candidate: Open Library's index first, and Google
Books only when Open Library has nothing.

Open Library's title search is strict. Given an author as well, it finds
nothing for *The Ladder of Divine Ascent* and four editions of *Ladder of
Divine Ascent*. So when the title as given finds nothing, the title without a
leading "The", "A" or "An" is asked next, and then the title and author as one
free-text query. Each is asked only when the one before found nothing, so a
title that matches costs one request. The looser questions can bring back
books that only share words with the title, which is one more reason the
candidates are for the caller to judge.

Each candidate is one edition. Open Library's index answers with works, and
names the edition of each that best matches the search. The ISBN, publisher,
language, year and cover are that edition's and no other's: a field the
edition does not give is left out rather than filled from the work, whose
publishers, years and ISBNs belong to all its editions at once. Only the
title, the authors and the subjects, which describe the work, are the work's.
Open Library's index gives no length for an edition, so its candidates carry
no `pages`. Google Books answers with volumes, which are editions already.

| Field | When the candidate has an ISBN | When it has none |
|---|---|---|
| `id` | The first ISBN of the edition whose check digit agrees, in thirteen digits; one written in thirteen is taken over one written in ten | The register's own key: an Open Library id such as `OL37863121M` for the edition, or `OL267171W` for the work when no edition was named; a Google Books volume id |
| `kind` | `isbn` | `olid` for Open Library, `googlebooks` for Google Books |

A work the index names no edition of, or names one it gives neither an ISBN
nor a key for, is a candidate named by the work's key, with its title,
authors and subjects and nothing an edition would say.

An ISBN whose check digit is wrong is passed over rather than corrected, and
Google's shelfmarks, typed `OTHER`, are not read as ISBNs even when their
digits would pass for one. A record of kind `isbn` can be handed straight back
to `bibfetch` for everything the registers say about that edition; `olid` and
`googlebooks` are names no lookup here answers about.

`cover_id` is the same thing it is in a lookup record: the first cover in
Open Library's record of the edition the record names, by that edition's ISBN
or its key, so `bibfetch cover --cover-id` fetches that edition's cover and no
other. The cover the index picked for the whole work is never asked for,
because it is often another edition's. Google Books knows nothing of Open
Library's covers and gives none.

The candidates are what the register said, in the order it said them. This
makes no judgement about which one is the book in hand, and keeps one that is
plainly another book, because a title is not an identifier: *The Hamlet* comes
back for *Hamlet*. Deciding whether a candidate is the same work is a
comparison of titles, and [`bibclean`](../bibclean/README.md)'s `same_work`
is one.

```sh
bibfetch search --title 'Anathem' --author Stephenson \
  | jq -r 'select(.kind == "isbn") | .id' | head -1
```

Open Library having nothing is an answer only once Google Books has been
asked and has nothing either. When Google Books cannot be reached, the search
fails rather than say no register has the title, and when it refuses for its
quota (`403`, or `429` on every attempt) the failure says so and the command
exits `75`. Open Library being down is a failure only when Google Books has
nothing to offer in its place. Each is asked the same way a lookup asks,
retried with the same waits, except that a `403` from Google Books is not
asked again. Open Library is sent the title and the author as written; Google Books
is sent each word as its own `intitle:` or `inauthor:` term, so nothing in a
title is read as search syntax.

| Option | Effect |
|---|---|
| `--title <TITLE>` | The title, or as much of it as you have |
| `--author <AUTHOR>` | An author's name, or part of one |
| `--limit <COUNT>` | The most candidates to print, `1` to `40`. Default `10` |

Exit `0` at least one candidate was printed, `1` no register has the title,
`65` the title was empty, `69` a register that had to answer could not be
reached, `74` the output could not be written, `75` Google Books refused for
its quota.

From Rust, `bibfetch::search(&agent, title, author, limit, attempts)` returns
the candidates, empty when no register has the title.

## Reaching the network

This and [`emb`](../emb/README.md) are the only two tools here that do. Both
say so in `--help`, and for both it is the whole of what they do rather than
something that happens on the way to something else — which is the condition
under which [the contract](../../docs/contract.md) allows it at all.

Nothing is cached, nothing is stored but a cover you asked to have written,
and nothing is sent but the identifier, or the title and author searched for,
and the address you pass to `--polite`.

## See also

- [`idnorm`](../idnorm/README.md) — reading the identifier in the first place.
- [`bibclean`](../bibclean/README.md) — the rest of a catalogue record, and
  whether a candidate is the same work.
