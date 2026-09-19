# bibclean

Catalogue records audited and repaired, with a reason for every change.

Two commands: `bibclean` for the records, and
[`vocabmerge`](#vocabmerge--the-other-command) for the headings they are filed
under.

Part of [catena](../../README.md).

## What it does

Reads catalogue records, names everything wrong with each one, weighs it, and
makes the repairs that need no lookup.

Nothing is rewritten silently. Every change comes back with the fault that
prompted it, so a run can be read before it is agreed to.

## When you would use it

- **Before trusting a catalogue you inherited.** Metadata rots in recognisable
  ways, and the ways are worth naming: a title that is really a filename, an
  author field holding a publisher, a scanner's name where the work's name
  should be.
- **Finding the worst records first.** Every fault carries a weight, so a
  thousand records sort by how much attention they deserve and a limited pass
  spends its time where it matters.
- **Recovering identifiers you already have.** A file is often named exactly
  its ISBN, which is a free and certain identifier for a record that has none
  — certain because the check digit says so.
- **As a gate in a pipeline.** It exits `1` when anything needed attention,
  so a nightly job can stop and ask.

## Install

```sh
cargo install --git https://git.pumpin.online/funkpower/catena bibclean
```

## Example

Five records in, four of them wanting attention:

```
$ bibclean --flagged books.ndjson
{"v":1,"title":"The Human Condition","author":"Arendt, Hannah","isbn":"9780226586601","path":"/library/9780226586601.pdf","reasons":["allcaps_title","missing_isbn"],"severity":2,"changes":[{"field":"title","from":"THE HUMAN CONDITION","to":"The Human Condition","why":"allcaps_title"},{"field":"isbn","from":"","to":"9780226586601","why":"missing_isbn"}]}
{"v":1,"title":"Untitled","author":"Arguing About War","isbn":null,"path":null,"reasons":["placeholder_title","title_in_author","missing_isbn"],"severity":7,"changes":[]}
{"v":1,"title":"Microsoft Word - ch3.doc","author":"Oxford University Press","isbn":null,"path":null,"reasons":["filename_title","junk_title","publisher_as_author","missing_isbn"],"severity":9,"changes":[]}
{"v":1,"title":"The Fall: And the Rise","author":"Camus, Albert","isbn":null,"path":null,"shelf":"upstairs","reasons":["underscore_title","missing_isbn"],"severity":1,"changes":[{"field":"title","from":"The_Fall_ And the Rise_txt","to":"The Fall: And the Rise","why":"underscore_title"}]}
```

Three things to notice: the ISBN was recovered from the filename and the
check digit confirmed it; the second record was not touched, because what it
needs is a lookup and guessing would be worse; and `"shelf": "upstairs"` came
through untouched, because fields this does not know about are carried.

The worst ten records, in order:

```sh
bibclean --flagged books.ndjson | jq -s 'sort_by(-.severity) | .[:10] | .[]' -c
```

Just the changes, for review:

```sh
bibclean books.ndjson | jq -r '.changes[] | "\(.why)\t\(.from)\t→\t\(.to)"'
```

## Options

| Option | Effect |
|---|---|
| `--flagged` | Write only the records that need attention |

A missing ISBN alone does not count as needing attention: nearly every book
lacks one.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Nothing needed attention |
| `1` | Something did — a fault worth fixing, or a slug worth replacing |
| `64` | The command line was wrong |
| `65` | A record could not be read |
| `74` | The file could not be read |

## Input

One JSON record per line. `title`, `author`, `isbn`, `path`, `slug` and `id`
are read; everything else is carried through untouched, so a record can go
through this and back into whatever it came from.

```json
{"title":"THE HUMAN CONDITION","author":"Arendt, Hannah","path":"/library/9780226586601.pdf"}
```

## Recovering a missing author

A book says who wrote it on its own copyright page. Hand in the opening pages
as `front_matter` and a record whose author is missing gets a name proposed,
with the evidence and how far it is worth trusting:

```
$ bibclean authors.ndjson
{"v":1,"title":"The Icon and the Axe","author":"Unknown",…,"reasons":["unknown_author","missing_isbn"],"severity":3,"changes":[],"author_proposal":{"author":"James H. Billington","method":"front_matter_copyright","confidence":0.9,"evidence":"…by James H. Billington","isbn":"0394708461"}}
{"v":1,"title":"Tragedy and Hope by Carroll Quigley","author":"",…,"reasons":["unknown_author"],"severity":3,"changes":[],"author_proposal":{"author":"Carroll Quigley","method":"title_by","confidence":0.7,"evidence":"by Carroll Quigley","isbn":null}}
```

Four signals, cheapest and most trustworthy first:

| Method | Confidence | Read from |
|---|---|---|
| `front_matter_copyright` | 0.9 | "Copyright © 1966 by …" |
| `front_matter_loc` | 0.8 | The cataloguing block's main entry, written surname first |
| `title_by` | 0.7 | "… by Author Name" at the end of the title |
| `title_camel` | 0.5 | A run-together name at the front of the title |

An ISBN found on the same page rides along with the name, because the two are
facts about the same book.

A candidate that reads as a publisher rather than a person is refused — an
imprint holds the copyright on a great many books and is the author of none of
them — as is one with a digit in it, one that opens with an article, and one
too long to be a name. Shouting is fixed, bare initials get their full stops,
and co-authors are kept: `John Schuster and John Sutton` becomes
`John Schuster & John Sutton`.

Nothing is applied. A record that already has an author is not second-guessed;
the proposal is only ever made where there is a gap.

The front matter is read and never written back — it is evidence handed in,
not part of the record.

## Slugs

A slug is a public identifier — it reaches citations and notes, and outlives
whatever produced it. When a record carries one, along with the `id` that
makes it unique, its slug is judged too:

```
$ bibclean slugs.ndjson
{"v":1,"title":"Weaponizing Anthropology","author":"Laura Nader",…,"slug":"william-zinsser-on-writing-well-3451","id":3451,"reasons":[],"severity":0,"changes":[],"slug_proposal":{"current":"william-zinsser-on-writing-well-3451","proposed":"laura-nader-weaponizing-anthropology-3451","tier":"wrong-book","reasons":["only 0% of slug tokens appear in the current title/author — slug and metadata disagree"]}}
{"v":1,"title":"Mind and Brain","author":"Zinsser, William",…,"slug":"unknown-mind-and-brain-1234","id":1234,"reasons":[],"severity":0,"changes":[],"slug_proposal":{"current":"unknown-mind-and-brain-1234","proposed":"zinsser-william-mind-and-brain-1234","tier":"placeholder-author","reasons":["slug opens with placeholder author 'unknown'"]}}
```

Nothing is renamed. A proposal comes back beside the slug the record has, and
the **tier** says how much the evidence is worth:

| Tier | Evidence | Safe to apply in bulk? |
|---|---|---|
| `placeholder-author` | The slug opens with a placeholder, so it was minted without an author | yes |
| `filename-artefact` | The slug carries a mirror suffix, a wrapper format or a path | yes |
| `wrong-book` | Too few of the slug's words appear in the record at all | no |
| `noop-metadata-junk` | Regenerating gives the same slug: the record is the problem | no |
| `defer-new-slug-junk` | The new slug would be no better than the old one | no |
| `review-loses-volume` | The current slug names a volume the new one would lose | no |
| `review-low-recall` | The new slug recovers too little of the title | no |

The first two rest on evidence inside the slug itself and cannot be wrong
about it. The rest are *disagreements* between the slug and the record, and a
disagreement does not say which side is at fault — which is exactly the case
where renaming automatically makes things worse.

Only the safe tiers:

```sh
bibclean books.ndjson | jq -c 'select(.slug_proposal.tier | IN("placeholder-author","filename-artefact"))'
```

## The faults it names

| Reason | Weight | What it means |
|---|---|---|
| `placeholder_title` | 4 | The whole title is "Untitled", "Document", "N/A" |
| `placeholder_author` | 4 | The same, in the author field |
| `path_title` | 4 | A file path sitting in the title |
| `numeric_author` | 4 | An author field opening with a bare year or series number |
| `unknown_author` | 3 | "Unknown", "Anonymous", empty |
| `filename_title` | 3 | A bare filename, extension and all |
| `junk_title` | 3 | The name of the tool that made the file, not the work |
| `publisher_as_author` | 3 | An imprint in the author field |
| `title_in_author` | 3 | The title is a placeholder and the real title is in the author field |
| `allcaps_title` | 2 | Shouting |
| `lowercase_title` | 2 | A filename stub: "heidegger areopagite" |
| `missing_apostrophe` | 2 | "Lossky s Argument" |
| `possible_swap` | 2 | Title and author look to be the wrong way round |
| `underscore_title` | 1 | Underscores standing in for spaces and colons |
| `missing_isbn` | 0 | Reported, but not a reason to do anything |

Each test is deliberately conservative. A real book called "The Document" or
"A Test of Faith" must not be swept up, so the placeholder tests only fire
when the *entire* field is the placeholder; and "Oxford University Press" in
an author field is wrong while "Oxford, Michael" is a person.

## The repairs it makes

| Fault | Repair |
|---|---|
| `underscore_title` | A trailing format name comes off, an underscore before a subtitle becomes a colon, the rest become spaces |
| `allcaps_title` | Title-cased, keeping small words small, hyphenated parts cased on both sides, and accented first letters capitalised rather than skipped |
| `missing_isbn` | Filled from the filename, if a checksum-valid ISBN is in it |

Everything else is named and left alone, because the fix needs a lookup and a
wrong guess in a catalogue is harder to find than a missing value.

## vocabmerge — the other command

A catalogue's headings accumulate spellings. The same person is entered
surname-first once and given-name-first twice; a publisher appears with and
without its legal suffix; a subject grows a stray bracket. Each spelling is a
separate heading, so the works under one are invisible from the others.

`vocabmerge` reads a list of headings and says what should happen to them:

```
$ vocabmerge --kind author authors.ndjson
{"kind":"author","action":"merge","keep":"Fyodor Dostoevsky","drop":"Dostoevsky, Fyodor","keep_id":3,"drop_id":1,"keep_count":11,"drop_count":3,"method":"last_first","confidence":0.9,"evidence":"3 books as 'Dostoevsky, Fyodor' → 'Fyodor Dostoevsky' (11)"}
{"kind":"author","action":"merge","keep":"Fyodor Dostoevsky","drop":"Dostoevsky| Fyodor","keep_id":3,"drop_id":2,"keep_count":11,"drop_count":1,"method":"last_first","confidence":0.9,"evidence":"1 books as 'Dostoevsky| Fyodor' → 'Fyodor Dostoevsky' (11)"}
```

Subjects get the same treatment, plus the headings that are not subjects at
all:

```
$ vocabmerge --kind tag tags.ndjson
{"kind":"tag","action":"merge","keep":"Philosophy","drop":"[Philosophy]",…,"method":"tag_norm","confidence":0.9,…}
{"kind":"tag","action":"merge","keep":"Philosophy","drop":"Philosophy.",…,"method":"tag_norm","confidence":0.9,…}
{"kind":"tag","action":"delete","keep":null,"drop":"z-lib.org",…,"method":"junk_tag:format-or-watermark","confidence":0.92,"evidence":"format-or-watermark: 'z-lib.org' on 300 books"}
{"kind":"tag","action":"delete","keep":null,"drop":"9780198237907",…,"method":"junk_tag:isbn","confidence":0.92,"evidence":"isbn: '9780198237907' on 1 books"}
```

`18th Century` and `Philosophy` are left alone: the deletion rules are
high-precision on purpose, because a real subject swept up by one is a shelf
nobody can find again.

### Input

One heading per line — an `id`, a `name`, and the `count` of works under it:

```json
{"id":1,"name":"Dostoevsky, Fyodor","count":3}
```

### Options

| Option | Effect |
|---|---|
| `--kind <author\|publisher\|tag>` | What kind of heading the list holds |
| `--rules <FILE>` | Vocabulary specific to this collection |

The rules file is JSON, and today holds one thing:

```json
{"device_prefixes": ["alexskobo"]}
```

An e-reader stamps its own name onto every book it touches. The generic device
names are built in; one named after its owner is a fact about a single
collection, so it is supplied rather than compiled in.

### Exit codes

| Code | Meaning |
|---|---|
| `0` | Nothing to propose |
| `1` | Something to propose |
| `64` | The command line was wrong |
| `65` | A heading could not be read |
| `74` | The input could not be read |

### How headings are grouped

| Kind | Joined when |
|---|---|
| `author` | They normalise to the same string, **or** they are the same words in a different order and both read as a person's name |
| `publisher` | They are the same once the legal suffixes and the words `press`, `books`, `university` and the like are removed |
| `tag` | They normalise to the same string, brackets and trailing stops ignored |

Author groups join both rules at once, deliberately. Otherwise two
surname-first spellings merge onto each other and neither ever reaches the
given-name-first heading that most of the works are actually under.

The heading kept is the one with the most works, then the cleaner spelling of
it — shouting, a trailing separator, a legal suffix and a leading bracket all
count against a heading, and spaced initials count for one.

A respelling is trusted further than an inversion: `strict_norm` at 0.95,
`last_first` at 0.9, a publisher family at 0.8, a junk subject at 0.92.

## As a library

```rust
let cleaned = bibclean::clean(&book);
if bibclean::reasons::worth_fixing(&cleaned.reasons) {
    for change in &cleaned.changes {
        println!("{}: {} → {} ({})", change.field, change.from, change.to, change.why);
    }
}
```

The pieces are usable on their own: `bibclean::isbn::valid`,
`bibclean::isbn::from_filename`, `bibclean::fix::title_case`,
`bibclean::matching::same_work`, `bibclean::slug::slug_for`,
`bibclean::vocab::cluster`.

`same_work` is worth knowing about separately. It compares vocabulary rather
than characters, because character similarity is not usable for titles: "The
Cambridge Companion to Aquinas" and "The Cambridge Companion to Hume" are
eighty-six per cent the same string, and believing that attaches one book's
description to another.

## See also

- [`booksniff`](../booksniff/README.md) — what the file behind the record actually is.
- [The records](../../docs/records.md) — the shapes the rest of the suite uses.
