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

## Reaching the network

This and [`emb`](../emb/README.md) are the only two tools here that do. Both
say so in `--help`, and for both it is the whole of what they do rather than
something that happens on the way to something else — which is the condition
under which [the contract](../../docs/contract.md) allows it at all.

Nothing is cached, nothing is stored, and nothing is sent but the identifier
and the address you pass to `--polite`.

## See also

- [`idnorm`](../idnorm/README.md) — reading the identifier in the first place.
- [`bibclean`](../bibclean/README.md) — the rest of a catalogue record.
