# idnorm

The identifiers a bibliography is held together by, read and checked.

Part of [catena](../../README.md).

## What it does

A DOI arrives bare, as a URL, with a `doi:` prefix, with the full stop the
sentence around it left behind, in whatever case the publisher felt like. All
of those are one identifier. A catalogue that stores them as they arrived
cannot tell, so the same paper is three records and none of them join up.

`idnorm` reads one out of whatever it is wrapped in and writes it in the form
it should be stored and compared in.

## When you would use it

- **Before storing a reference**, so that two spellings of one DOI are one row.
- **Auditing a bibliography** for identifiers that cannot be right — an ISBN
  and an ISSN carry a check digit, and a mistyped one fails it.
- **Pulling identifiers out of running text**, where the surrounding sentence
  is attached to them.

## Install

```sh
cargo install --git https://git.pumpin.online/funkpower/catena idnorm
```

## Example

```
$ printf '10.1000/182\ndoi:10.1093/mind/LIX.236.433\n0028-0836\n978-0-306-40615-7\narXiv:2401.01234\n' | idnorm
{"v":1,"input":"10.1000/182","kind":"doi","id":"10.1000/182","checksum":null,"url":"https://doi.org/10.1000/182"}
{"v":1,"input":"doi:10.1093/mind/LIX.236.433","kind":"doi","id":"10.1093/mind/lix.236.433","checksum":null,"url":"https://doi.org/10.1093/mind/lix.236.433"}
{"v":1,"input":"0028-0836","kind":"issn","id":"0028-0836","checksum":true,"url":"https://portal.issn.org/resource/ISSN/0028-0836"}
{"v":1,"input":"978-0-306-40615-7","kind":"isbn","id":"9780306406157","checksum":true,"url":"https://openlibrary.org/isbn/9780306406157"}
{"v":1,"input":"arXiv:2401.01234","kind":"arxiv","id":"2401.01234","checksum":null,"url":"https://arxiv.org/abs/2401.01234"}
```

The ones that cannot be right:

```sh
idnorm refs.txt | jq -r 'select(.checksum == false) | .input'
```

What a line was not:

```
$ printf 'hello\n' | idnorm
idnorm: not an identifier: hello
$ echo $?
1
```

## Options

| Option | Effect |
|---|---|
| `--kind <doi\|issn\|isbn\|arxiv>` | Read every line as this kind rather than whatever it looks like |
| `--quiet` | Say nothing about the lines that are not identifiers |
| `--man` | Write this page as a manual page, in roff |

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Every line was an identifier |
| `1` | At least one was not |
| `64` | The command line was wrong |
| `74` | The file could not be read |

## What it can and cannot tell you

**It can say a string is well formed.** An ISBN and an ISSN carry a check
digit, weighted by position, which catches one mistyped figure and two
transposed ones. A DOI and an arXiv identifier carry no checksum at all, so
for those `checksum` is `null` rather than `true` — there is nothing to check
against, and saying `true` would claim more than is known.

**It cannot say the work exists.** That is a fact about the world rather than
about the string, and the only way to learn it is to ask a register.
[`bibfetch`](../bibfetch/README.md) does that:

```sh
idnorm refs.txt | bibfetch | jq -r 'select(.found | not) | .id'
```

Those two together are what "validate against a catalogue" means, and they are
two tools because one of them touches the network and the other must not.

## The four kinds

**DOI.** A registrant prefix beginning `10.` and a suffix the registrant
chooses. Stored lowercased, because a DOI is case-insensitive and two
catalogues will disagree about the case of one. Read out of running text, with
the trailing full stop, comma or bracket the sentence left on it removed.

**ISSN.** Eight digits in two fours, the last a check digit that may be `X`
for ten. Stored hyphenated.

**ISBN.** Ten or thirteen digits. Stored as digits, without the hyphens, since
where a publisher puts them varies. Only where the line is the identifier and
nothing else — an ISBN has no prefix announcing it, so a sentence with a
thirteen-digit number in it would otherwise read as one.

**arXiv.** Both forms: `2401.01234` since 2007, and `math.GT/0309136` before
it. A version suffix is kept, because `v2` names a particular revision and
citing without one means the latest.

## As a library

```rust
let found = idnorm::parse("https://doi.org/10.1000/182").expect("an identifier");
assert_eq!(found.kind, idnorm::Kind::Doi);
assert_eq!(found.id, "10.1000/182");
```

`idnorm::doi`, `idnorm::issn` and `idnorm::arxiv` read one kind each, and
`idnorm::issn::valid` checks a serial number's check digit.

## See also

- [`bibfetch`](../bibfetch/README.md) — what a register says about one.
- [`bibclean`](../bibclean/README.md) — the rest of a catalogue record.
