# seriesof

The series a book belongs to, and which volume of it this is.

Part of [catena](../../README.md).

## What it does

Reads a series and a volume off a title, and says nothing about the books it
cannot place.

A shelf groups by series and volume, and most catalogues have neither set on
most books. What that looks like from the reading chair: four records all
reading "A History of Philosophy" with nothing to say which volume any of them
is, and five volumes of one work sorting as five unrelated books.

## When you would use it

- **Before importing a shelf** into anything that browses by series.
- **Auditing a catalogue** that was built from filenames, where the series was
  in the title all along and nobody read it.
- **Finding the volumes of a work you already have**, which is a query you
  cannot ask until the series field exists.

## Install

```sh
cargo install --git https://git.pumpin.online/funkpower/catena seriesof
```

## Example

```
$ seriesof catalogue.ndjson | jq -r '"\(.series)\t#\(.volume // "—")\t\(.method)"'
The Philokalia	#3	volume
A History of Philosophy	#4	volume
The Art of Computer Programming	#3	volume
Cambridge Companions	#—	publisher
Very Short Introductions	#—	publisher
Blackwell Companions	#—	publisher
Raven Son	#1	volume
```

The whole record, for a title that carries a volume:

```
$ seriesof catalogue.ndjson | head -1
{"v":1,"title":"The Philokalia Vol 3","author":"Palmer","series":"The Philokalia","volume":3,"method":"volume"}
```

And for publisher shorthand, which proposes a title as well:

```
$ seriesof catalogue.ndjson | jq -c 'select(.method == "publisher") | {series, proposed_title}' | head -2
{"series":"Cambridge Companions","proposed_title":"The Cambridge Companion to Aquinas"}
{"series":"Very Short Introductions","proposed_title":"Logic: A Very Short Introduction"}
```

Plain titles, for a directory rather than a catalogue:

```
$ ls *.epub | sed 's/\.epub$//' | seriesof --titles
{"v":1,"title":"The Philokalia Vol 3","series":"The Philokalia","volume":3,"method":"volume"}
```

Nothing to propose is an answer:

```
$ printf '{"title":"Being and Time"}\n' | seriesof
$ echo $?
1
```

Applying only what you believe:

```sh
seriesof catalogue.ndjson \
  | jq -r 'select(.volume) | "UPDATE book SET series = \(.series|@json), series_index = \(.volume) WHERE title = \(.title|@json);"' \
  | psql
```

## Options

| Option | Effect |
|---|---|
| `--titles` | Read plain titles, one per line, rather than records |
| `--man` | Write this page as a manual page, in roff |

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Something was proposed |
| `1` | Nothing was |
| `64` | The command line was wrong |
| `65` | The input was not records |
| `74` | The file could not be read |

## How it decides

Two shapes are recoverable, and they are not the same kind of fact.

### A volume marker is a position

"The Philokalia Vol 3", "Collected Works, Volume 9", "(Raven Son Book 1)".
The unit word is required — `Vol`, `Volume`, `Book`, `Part`, `Pt`, `Tome`,
`Bk` — because a bare trailing number is far more often a year, an edition or
noise. Arabic and roman numerals both count, in any script, up to two hundred.

The series is **what comes before the marker**. Whatever follows is that
volume's own subtitle: "The Art of Computer Programming, Volume 3: Sorting and
Searching" is volume 3 *of* the former, and gluing the halves back together
gives a title nobody published.

Except where the convention puts the series inside a bracket — "The Song of
the Sirin (Raven Son Book 1)" is book one of *Raven Son*, not of "The Song of
the Sirin (Raven Son".

A marker with nothing usable before it is left alone. "Vol. 2. PLATO,
EPICTETUS, MARCUS AURELIUS" is a volume of something whose name is not in the
field at all, and filing it under its own contents is worse than not filing
it.

### Publisher shorthand is only membership

A title of "Aquinas (Companion)" against an author of "Cambridge" means the
publisher landed in the author column and the collection name was flattened
into a parenthetical. The work is *The Cambridge Companion to Aquinas*.

These get a series and **no number**, deliberately. Cambridge Companions are
not numbered, nobody knows what "#3" would mean, and cataloguing software
defaults a missing index to 1 — so filing them as a numbered series makes
every one display as volume one, asserting an order the data does not have.

A title is only reconstructed where it reliably can be. Cambridge and
Routledge front theirs with the imprint; Blackwell does not — its Companions
to Philosophy are "A Companion to Nietzsche", while a few of its religion
titles genuinely are "The Blackwell Companion to …". The shorthand cannot tell
which, so Blackwell gets its series and no title claim.

Recognised: Cambridge, Oxford, Blackwell, Wiley Blackwell, Routledge, SUNY,
Palgrave and Springer, across companions, concise companions, handbooks,
guidebooks, Very Short Introductions, Contemporary Debates, contemporary
introductions, anthologies and studies.

## As a library

```rust
let found = seriesof::series_of("The Philokalia Vol 3", None).expect("a series");
assert_eq!(found.series, "The Philokalia");
assert_eq!(found.volume, Some(3));
```

`parse_volume` and `publisher_series` answer each half on its own, and
`roman_or_int` reads a numeral in any script.

## See also

- [`bibclean`](../bibclean/README.md) — the rest of what a catalogue record
  needs before it is worth keeping.
- [`booksniff`](../booksniff/README.md) — what the file actually is.
