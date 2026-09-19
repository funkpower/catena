# bibclean

Catalogue records audited and repaired, with a reason for every change.

Back to [the index](README.md).

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

Five records, four of which have something wrong with them:

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
| `1` | Something did |
| `64` | The command line was wrong |
| `65` | A record could not be read |
| `74` | The file could not be read |

## Input

One JSON record per line. `title`, `author`, `isbn` and `path` are read;
everything else is carried through untouched, so a record can go through this
and back into whatever it came from.

```json
{"title":"THE HUMAN CONDITION","author":"Arendt, Hannah","path":"/library/9780226586601.pdf"}
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
`bibclean::matching::same_work`.

`same_work` is worth knowing about separately. It compares vocabulary rather
than characters, because character similarity is not usable for titles: "The
Cambridge Companion to Aquinas" and "The Cambridge Companion to Hume" are
eighty-six per cent the same string, and believing that attaches one book's
description to another.

## See also

- [`booksniff`](booksniff.md) — what the file behind the record actually is.
- [The records](records.md) — the shapes the rest of the suite uses.
