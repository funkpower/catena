# textkit

Token counting, sentence and paragraph splitting.

Back to [the index](README.md).

## What it is

A library. Two jobs, both of which have to give the same answer everywhere or
chunk boundaries move: counting tokens the way an embedding stack counts them,
and cutting text into sentences and paragraphs.

## When you would use it

- **Deciding how much text fits** in a model's context, before sending it.
- **Writing your own chunker** that has to agree with
  [`chunkmd`](chunkmd.md) about where a boundary is.
- **Porting anything from Python** where the exact behaviour of `\s`, `\w` or
  `\b` matters more than it looks like it should.

## Counting tokens

```rust
let tokens = textkit::count("the mill stood at the northern end");
let ids = textkit::encode("…");
let text = textkit::decode_lossy(&ids[..100]);
```

`cl100k_base`, loaded from data compiled into the binary — no download, no
cache directory, nothing to go stale. Counting against a different tokeniser
than the one an index was built with silently moves every chunk boundary,
which is why this is one function and not a parameter.

Special-token spellings are treated as ordinary text. Text out of a PDF can
contain anything, including the literal string `<|endoftext|>`, and encoding
it as a control token would be both wrong and a way for a document to reach
into a model's input.

Cutting a list of tokens at an arbitrary index can split a character in half,
so decoding a slice is lossy rather than fallible:

```rust
// The last resort of a chunker: guarantees a maximum, respects nothing else.
for piece in textkit::split_at_token_limit(&text, 512) {
    assert!(textkit::count(&piece) <= 512);
}
```

## Splitting text

```rust
for paragraph in textkit::paragraphs(&markdown) {
    for sentence in textkit::sentences(paragraph) {
        // …
    }
}
```

Both return borrowed slices, trimmed, with the empty pieces dropped.

A paragraph break is a newline, any run of whitespace, and a newline — so one
blank line and four of them separate equally, and trailing spaces on an
otherwise blank line do not stop it being blank.

A sentence break is whitespace following `.`, `!` or `?`. Crude on purpose: it
does not know that `Mr.` or `p. 41` are not sentence ends. It is the fallback
path for a paragraph too large to chunk whole, where a wrong cut costs a
little context and a clever cut that disagrees with an existing index costs a
re-embedding.

## Whitespace

```rust
let trimmed = text.trim_matches(textkit::is_space);
```

`is_space` is whitespace by Python's definition: the Unicode `White_Space`
property *and* the four ASCII information separators `U+001C`–`U+001F`.
Trimming with the narrower definition leaves a separator on the front of a
piece, which changes its token count, which moves a chunk boundary.

## Numbers

```rust
assert_eq!(textkit::decimal("<!-- Page 412 -->"), 412);
assert_eq!(textkit::decimal("٤٢"), 42);
```

Every decimal digit, not only the ASCII ones. Every Unicode decimal block runs
zero to nine in consecutive code points, which is what makes this possible
without a lookup table.

## Patterns

Character classes that mean in Rust what they mean in Python:

```rust
use textkit::pattern::{SPACE, WORD, WORD_START, WORD_END};

let year = fancy_regex::Regex::new(&format!(r"{WORD_START}(19|20)\d{{2}}{WORD_END}"))?;
let heading = regex::Regex::new(&format!(r"(?m)^(#{{1,3}}){SPACE}+(.+)$"))?;
```

The two regex engines disagree, and every disagreement is a heuristic that
fires on one side and not the other:

- Python's `\s` also matches `U+001C`–`U+001F`.
- Python's `\w` is the letters, the numbers and the underscore. Rust's adds
  every combining mark and every connector punctuation mark, and leaves out
  the letter-like and other numbers — so `Gödel` written in decomposed form is
  one word to Rust and two to Python, and `²` is a word character to Python
  and not to Rust.
- `\b` is defined in terms of `\w`, so it inherits all of that.

The boundary forms are lookarounds, so a pattern using them needs an engine
that has them.

## See also

- [`corpus-core`](corpus-core.md) — the records these help produce.
- [`chunkmd`](chunkmd.md) — the chunker built on this.
