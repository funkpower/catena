# langid

What language a document is in.

Part of [catena](../../README.md).

## What it does

Two passes, because one will not do. The writing system settles most of the
world outright — a text in Hangul is Korean — and for the Latin and Cyrillic
alphabets it only narrows the field, so the commonest words decide between
what is left.

No model, no download, no network. Function words make up a third of any
running text, and counting them is decisive on a paragraph. On a single line
it is not, which is why a close call is refused rather than guessed at.

## When you would use it

- **Before OCR**, to know which language to hand it.
- **Before embedding**, where a multilingual shelf wants different models.
- **Filling the `language` field** on a `doc` record, which nothing else sets.
- **Finding the odd book out** in a library you assumed was all one language.

## Install

```sh
cargo install --git https://git.pumpin.online/funkpower/catena langid
```

## Example

```
$ langid converted/*.md | jq -r '[.name, (.confidence*100|round|tostring + "%"), .path] | @tsv'
English	77%	converted/enlightenment.md
English	76%	converted/hagakure.md
French	81%	converted/meditations.md
Latin	68%	converted/summa.md
```

Just the code:

```
$ langid --quiet book.md
en
```

The whole record, which carries the bibliographic code the `doc` record
wants:

```
$ langid book.md
{"v":1,"path":"book.md","language":"en","bibliographic":"eng","name":"English","script":"Latin","confidence":0.77}
```

The ones that are not what you assumed:

```sh
langid converted/*.md | jq -r 'select(.language != "en") | [.path, .name] | @tsv'
```

Reading from a pipe:

```sh
pdftotext -l 5 scan.pdf - | langid --quiet
```

## Options

| Option | Effect |
|---|---|
| `--quiet` | Print the code alone, one per line |
| `--man` | Write this page as a manual page, in roff |

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Every file was placed |
| `1` | At least one could not be |
| `64` | The command line was wrong |
| `74` | A file could not be read |

## How it decides

**The script first.** Kana, Hangul, Hebrew, Arabic and Devanagari each belong
to one language as far as a library is concerned, and no amount of word
counting improves on that. Kana is checked before Han, because Japanese is
mostly Han characters with kana between them and it is the kana that tell it
from Chinese.

A quarter of a text's letters agreeing is enough to call it a script: a book
carries page numbers, names and quotations in other alphabets, and an English
word in a Russian paragraph does not make the paragraph English.

**Then the words.** Thirteen languages across Latin, Cyrillic and Greek, each
with its thirty commonest function words. The score is the share of a text's
words found in each list, and the lists are the same length so the scores
compare.

**A close call is no answer.** The winner must be a quarter ahead of the
runner-up. `de` is among the commonest words of four languages at once, and on
its own settles none of them — so a text that short comes back with the script
named, no language, and a note saying which two it could not choose between.

Only the first twenty thousand characters are read. A book's language does not
change after the first few pages.

## As a library

```rust
let found = langid::detect("book.md", text);
assert_eq!(found.language.as_deref(), Some("en"));
```

`langid::script::detect` answers the writing system on its own, and
`langid::words::score` ranks every language of a given script.

## See also

- [`booksniff`](../booksniff/README.md) — what the file is.
- [`bookmeta`](../bookmeta/README.md) — what the file says its language is,
  which is worth more when it says anything.
