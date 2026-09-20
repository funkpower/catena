# citefind

The references in a bibliography or a page of notes.

Part of [catena](../../README.md).

## What it does

A reference list is the one place in a book where the text is already
structured. Every entry names a work, and most name it in one of a handful of
shapes that have not changed in a century. What is missing is not information
but punctuation: the entries run together, and nothing in the file says where
one stops.

So this does two jobs. It cuts the block into entries — at the note numbers
where there are any, at the layout where there are not. Then it reads each
entry as far as its shape allows, and **names the fields it could not settle**
rather than guessing at them.

That last part is the point. A made-up year and a real one are
indistinguishable three stages downstream; a missing one is not.

## When you would use it

- **Building a citation graph** — which works a book leans on, and how often.
- **Finding the identifiers** in a reference list, to hand to
  [`idnorm`](../idnorm/README.md) and [`bibfetch`](../bibfetch/README.md).
- **Filling a catalogue** from the back of a book rather than from a register.
- **Costing a model run**: ask one about the entries that did not parse, and
  nothing else.

## Install

```sh
cargo install --git https://git.pumpin.online/funkpower/catena citefind
```

## Example

```
$ citefind bibliography.md
{"v":1,"idx":0,"text":"Smith, John. *The Making of the Second Estate*. Oxford: Clarendon Press, 1998.","marker":null,"authors":["Smith, John"],"year":1998,"title":"The Making of the Second Estate","container":"Clarendon Press","pages":null,"ids":[],"kind":"book","continuation":false,"unsettled":[]}
{"v":1,"idx":1,"text":"Doe, Jane, and Peter Brown. \"Obligation and Its Limits.\" *Journal of Medieval Studies* 12, no. 3 (2001): 45-67.","marker":null,"authors":["Doe, Jane","Peter Brown"],"year":2001,"title":"Obligation and Its Limits","container":"Journal of Medieval Studies","pages":"45-67","ids":[],"kind":"article","continuation":false,"unsettled":[]}
```

What a book cites, by year:

```sh
citefind bibliography.md | jq -r 'select(.year) | .year' | sort | uniq -c
```

The identifiers, checked and resolved:

```sh
citefind refs.md | jq -r '.ids[] | select(.kind == "doi") | .id' | idnorm | bibfetch
```

The works it leans on hardest, from a page of endnotes:

```sh
citefind notes.md | jq -r 'select(.title) | .title' | sort | uniq -c | sort -rn | head
```

## Asking a model about the rest

`--prompt` writes a question about the entries that did not parse, and only
those. Nothing is sent from here: the prompt goes to stdout and what happens
to it is yours to decide.

```
$ citefind --prompt refs.md
Below are entries from the bibliography or notes of a book. Each was read as
far as its punctuation allowed, and the fields that could not be read off it
are listed with it.

--- 0
text: Marron, Kevin Ritual Abuse, Seal Books, 1988
year: 1988
wanted: authors, title, container
...
```

```sh
citefind --prompt refs.md | ollama run qwen3:8b > reply.json
citefind --merge reply.json refs.md
```

A reply fills in only the fields that were asked about. A model that
volunteers a better title for an entry whose title was read off the page is
overruled — the page is evidence and the model is a guess — and a reply that
is not one JSON object is refused rather than half-applied.

One prompt covers every unsettled entry rather than one prompt each. A
reference list is long and mostly ordinary; the entries worth asking about are
the mangled few, and they are cheaper to ask about together.

## Options

| Option | Effect |
|---|---|
| `--prompt` | Write the question about what did not parse, instead of the records |
| `--merge FILE` | Fill in what did not parse from a model's reply |
| `--quiet`, `-q` | Say nothing; the exit code is the answer |
| `--man` | Write this page as a manual page, in roff |

## Exit codes

| Code | Meaning |
|---|---|
| `0` | It found references |
| `1` | It found none |
| `64` | The command line was wrong |
| `65` | The input was not readable as text, or a reply was not JSON |
| `74` | The file could not be read |

## The record

| Field | What it holds |
|---|---|
| `text` | The entry as it appeared, whitespace collapsed |
| `marker` | The note number it carried, where it carried one |
| `authors` | The names, in the order and the form the entry gave them |
| `year` | The year of publication |
| `title` | The work cited — for an article the article, not the journal |
| `container` | The journal it appeared in, or the publisher of a book |
| `pages` | The pages, as written |
| `ids` | Any DOI, ISBN, ISSN, arXiv id or URL, normalised |
| `kind` | `book`, `article`, `chapter`, `web` or `other` |
| `continuation` | Whether it only points back at an earlier entry |
| `unsettled` | The fields it could not read off the entry |

Every field is written on every record, `null` where there is nothing. A key
that comes and goes is a key every consumer has to guard, and the point of
`unsettled` is that an absence be legible.

`text` is kept whatever else was read, because every other field is an
interpretation of it and somebody will want to check.

## How it cuts

**A page of notes** is cut at its note numbers, in any of the shapes
conversion leaves them: `12.`, `[12]`, `fn39.`, a superscript, a number run
straight into the text, or a back-reference link. The pattern is the one
[`chunkmd`](../chunkmd/README.md) uses to recognise a page of notes, so the
two cannot drift apart, with two forms added — a link pointing at another
file rather than at a fragment, and an author-date entry carrying none of the
punctuation `chunkmd` looks for. `chunkmd` is deciding whether a chunk is a
page of notes and can afford to miss one marker in thirty; here a missed
marker runs two references together.

A note that cites **several works** becomes several records, cut at the
semicolons that separate them, each keeping the note's number. A full stop
separates works in a note too, and is not used: it also separates the author
from the title in every bibliography entry ever written.

Where the first note number sits a long way into the block, what precedes it
is the body of the page rather than a note that lost its number, and it is
left out. The copyright line a digitised book prints at the foot of every
page is cut off the end of whatever note it landed on.

A page of references whose lines happen to begin on numbers is still a
bibliography: `Mellor, Hugh (1974) "In Defense of Dispositions," Philosophical
Review 83:157-` continues `81. Reprinted in Mellor (1991):104-22.`, and that
`81.` is half a page range rather than a note number. More lines opening on a
surname than there are markers decides it.

**A bibliography** numbers nothing, so the signal is layout: a blank line
between entries, a hanging indent under each, or the surname-comma opening
that nearly every author-first entry begins with. Whichever the block carries
is the one used — and each block a blank line produces is looked at again,
because the blank line is often after the running head and not between the
entries under it. Inside a block, one line opening on a surname and a comma
is enough: a block is a single entry or a list of them, not a mixture.

## The forms it reads

| Form | Example |
|---|---|
| Bibliography | `Smith, John. *The Making of the Second Estate*. Oxford: Clarendon Press, 1998.` |
| Author-date | `Peirce, C. S. (1878). How to make our ideas clear. *Popular Science Monthly*, 12, 286-302.` |
| Author-date, no brackets | `Altmann, M. 1951, A Study of Behaviour. Sociometry 14, 351-354.` |
| Chicago note | `John MacGowan, The Imperial History of China (1897), p. 154.` |
| Chicago short note | `Unger, Dutch Shipbuilding, 33-4` |
| Commas only | `Miller, David, On Nationality, Oxford: Clarendon Press, 1995` |
| Commas and a place | `Cheyne, T. K., Founders of Old Testament Criticism, London, 1893.` |
| Reading list, unmarked | `Block, N. 1981. Psychologism and behaviorism. Philosophical Review 90:5-43.` |
| Editor in the author's place | `Barratt, Alexandra, ed. Women's Writing in Middle English. London: Longman, 1992.` |
| A name that ends in an initial | `Hoyt, Edwin P. The Supersalesmen. New York: World Publishing, 1962.` |
| A reprint's two years | `Schleiermacher, Friedrich. 1813/1963. "Ueber die verschiedenen Methoden…"` |
| Chapter in a book | `Brown, Peter. "Sorcery…" In *Witchcraft Confessions*, ed. Mary Douglas, 17-45. London: Tavistock, 1970.` |

**Where the italics sit decides what they are.** In `Peirce (1878). Plain
title. *Popular Science Monthly*` they come after the title and name the
journal; in `Goffman (1963). *Stigma* Penguin` they open the tail and are the
title. The same goes for quotation marks.

A rule of dashes where the author should be means the author of the entry
above — the style says so, and the entry it refers to is the one immediately
before — so it is filled in.

## What it will not do

**Resolve `ibid.`** A continuation refers to whatever was cited before it,
which is a fact about the surrounding page and not about the entry. It is
flagged `continuation` and left alone, and left out of `--prompt` entirely.

The same goes for a **short cite**: `Spade 1980, 12.` and `Lowrie and
Fletcher, p. 133` point at a full entry elsewhere in the same bibliography.
The author and year they do give are kept; no reading of that line will
produce a title.

**Report a title it did not find.** An entry held together by commas alone,
with no names to anchor it, yields no title rather than its own text echoed
back. Nor does a run carrying a page reference or a semicolon count as one:
that is two entries run together. A quotation mark with no partner means the
cut landed inside a title — unless it is the last character, which is a
stray an opening mark was lost from.

**Take a keyed short cite for a reference.** `Mate selection: Buss, 1989;
Symons, 1979.` keys a note's own words to works given in full elsewhere. The
topic is not an author and `Buss, 1989` is not a work, so it is flagged
`continuation` like `ibid.`

**Take a translator's note for a reference.** An em dash and a role at the
end — `— Ed.`, `— Tr.` — is how a translator signs a note of their own.

**Take where a book was published for what it is called.** `Leiden: Brill,
2003` and `New York: 1928` are an imprint standing on its own, and a segment
that is only an imprint is skipped rather than read as the title. A title
with a colon in it is not one: `Russian Popular Culture: Entertainment and
Society since 1900` survives, which is why the imprint is asked for narrowly
— a place of up to three words, and then a year or a publisher and a year.

**Take a note's own remark for a work.** `The Commentator seems to have
based himself principally on the Bible` and `Dewey remarks that we experience
most things as temporal` are what a page of endnotes is mostly made of. A
title read off markup is taken at its word; a title guessed at by cutting the
entry at its sentence stops is not, and where the entry gave up nothing else
— no author, no identifier, no container — that guess has to be a noun
phrase. A finite verb is what separates the remark from `Quaestiones
Parisienses`.

**Take a siglum for a work.** `H, 272`, `FR,120-122`, `GNO`. The key that
expands the abbreviation is a page of its own that the entry does not carry,
and the heading the line sits under is not its title either.

**Take the reader's furniture for a reference.** `572 next page > < previous
page`, and the `file:///` path it points at, are what a PDF reader left in
the text.

**Take chapter and verse for a work.** `Matthew 4:17` and `Cf. Deuteronomy
18:15-19; Isaiah 53` are scripture references, which is most of what the
notes of a theological book are. So is `PG 4: 264` — Migne, and CSEL, NPNF,
SC and the rest are shelves, not works, and the work is not named there.

**Name a place of publication as the publisher.** `(London, 1973)` says where
a book was published and not by whom, so `container` stays empty. The place
is still in `text`.

**Mistake the back matter for the bibliography.** A contributor's biography
opens exactly as a Chicago note does — `S. Marc Cohen is Professor of…` — and
a note that opens by thanking people cites nothing however many names it
carries. Neither is a reference. Nor is a page footer a browser printed, nor
a note that runs on for six sentences with no imprint, no page reference and
no identifier.

## As a library

```rust
for one in citefind::read(&text) {
    println!("{} ({}) {:?}", one.authors.join("; "), one.year.unwrap_or(0), one.title);
}
```

`citefind::split::entries` cuts a block without reading the entries.
`citefind::parse::one` reads an entry already cut. `citefind::prompt::compose`
and `merge` are the two halves of the model-facing path, and
`prompt::PROMPT_VERSION` says which wording produced a given answer.

From Python, JavaScript or C, through
[`bindings`](../bindings/README.md):

```python
for one in catena.find_citations(text):
    print(one["year"], one["title"], one["unsettled"])
```

## See also

- [`idnorm`](../idnorm/README.md) — the identifiers it finds, checked.
- [`bibfetch`](../bibfetch/README.md) — what a register says about them.
- [`chunkmd`](../chunkmd/README.md) — which flags the chunks worth feeding it.
- [`bibclean`](../bibclean/README.md) — the same audit for a catalogue record.
