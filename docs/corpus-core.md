# corpus-core

The record types and the stream plumbing everything shares.

Back to [the index](README.md).

## What it is

A library, not a command — though it carries a small one for printing
schemas. It holds the three [record types](records.md), the NDJSON reader and
writer, and the exit-code conventions.

Everything else in the workspace depends on it, which is what makes the tools
composable: they agree about the records before they agree about anything
else.

## When you would use it

- **Reading or writing these records in your own program**, without shelling
  out to any of the tools.
- **Generating bindings in another language**, from the JSON Schema.
- **Writing a new filter** that joins the pipeline.

## The records

[`Doc`](records.md#doc--what-a-file-is),
[`Chunk`](records.md#chunk--a-retrievable-passage) and
[`Hit`](records.md#hit--one-scored-candidate). Each derives `Serialize`,
`Deserialize` and `JsonSchema`, and implements `Record`, which is how a reader
knows what version it is looking at.

```rust
use corpus_core::{Chunk, Kind, Doc};

let mut chunk = Chunk::new(0, "the mill stood at the northern end", 8);
chunk.page = Some(41);
chunk.chapter = Some("III. The Second Estate".into());
chunk.flag(corpus_core::chunk::flags::FOOTNOTE_CITATIONS);

let doc = Doc::new("scan.pdf", Kind::Pdf, 9_261_184);
```

Every field is emitted, including the empty ones, so a consumer can rely on
the shape rather than on what a particular producer happened to fill in.

## Reading and writing a stream

```rust
use corpus_core::ndjson;

for record in ndjson::read::<Chunk, _>(std::io::stdin().lock()) {
    let chunk = record?;
    println!("{} tokens", chunk.tokens);
}

ndjson::write_line(&mut std::io::stdout(), &chunk)?;
```

The reader is strict, and each strictness is there because the alternative is
a silent failure:

- **A record from a version it does not know is refused.** A reader that
  parses a newer record into whatever lines up will process truncated records
  rather than failing.
- **It stops at the first bad line.** A bad line means the producer and the
  consumer disagree about the format, and reading on buries that.
- **Lines are bounded.** A filter pointed at a binary file should fail, not
  allocate the file. The limit is sixteen megabytes and can be changed with
  `read_with_limit`.
- **Blank lines are skipped and a byte-order mark is ignored**, so a stream
  that was concatenated or written on Windows still reads.
- **Every record is flushed as it is written**, so a slow stage downstream
  sees a record as soon as it exists.

An error knows whether it was a closed pipe:

```rust
match ndjson::write_line(&mut out, &chunk) {
    Err(e) if e.is_broken_pipe() => return Ok(()),  // `| head` is normal
    other => other?,
}
```

## Floats JSON cannot carry

JSON has no spelling for NaN or infinity, and the usual behaviour is to write
them as `null` — which is a silent corruption in both directions: a score
written as `null` fails to read back and kills the stream at the consumer, and
a timestamp written as `null` reads back as "no timestamp" with nothing to say
it was ever anything else.

So a non-finite number is refused where it is produced, which is the only
place anyone can still fix it. Use the same guard on your own records:

```rust
#[derive(Serialize, Deserialize)]
struct Mine {
    #[serde(with = "corpus_core::finite")]
    score: f64,
    #[serde(default, with = "corpus_core::finite::option")]
    when: Option<f64>,
}
```

## Exit codes

```rust
use corpus_core::exit;

std::process::ExitCode::from(exit::OK)             // 0
std::process::ExitCode::from(exit::NO)             // 1
std::process::ExitCode::from(exit::INDETERMINATE)  // 2
std::process::ExitCode::from(exit::USAGE)          // 64
```

`Verdict` is the three-answer form for a predicate:

```rust
use corpus_core::exit::Verdict;

let answer = Verdict::from(needs_ocr);
std::process::ExitCode::from(answer)
```

See [the contract](contract.md#predicates-answer-with-exit-codes) for the
whole table.

## The schemas

```
$ cargo run -p corpus-core -- schema chunk > chunk.schema.json
$ cargo run -p corpus-core -- schema all
```

`doc`, `chunk`, `hit` or `all`. The output is byte-identical across runs, so
it can be committed and diffed.

## See also

- [The records](records.md) — the shapes, described.
- [`catena-cli`](catena-cli.md) — the rest of what a filter needs.
- [`textkit`](textkit.md) — counting and splitting text.
