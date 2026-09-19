# catena-cli

The parts of a filter every command does the same way.

Back to [the index](README.md).

## What it is

A library. Not a framework — a handful of functions, each of which exists
because getting it wrong is easy and the consequence is a pipeline that
misreports why it stopped.

Every command in this workspace is built on it, which is why they all behave
the same at the edges.

## When you would use it

- **Writing a filter that joins this pipeline**, and wanting it to behave like
  the others without rereading [the contract](contract.md).
- **Anywhere a small Unix filter is wanted**, in or out of this workspace.

## Parsing a command line

```rust
fn main() -> std::process::ExitCode {
    let args: Args = match catena_cli::parse() {
        Ok(args) => args,
        Err(code) => return code,
    };
    // …
}
```

`--help` and `--version` are not errors and leave with `0`; anything else
wrong with the arguments leaves with `64`, which is what the rest of the suite
means by "the command line was wrong". Argument parsers tend to pick their own
number for this, and a pipeline that routes on exit codes then has two
different meanings for the same one.

## Reading the input

```rust
let text = match catena_cli::read_text(args.path.as_deref()) {
    Ok(text) => text,
    Err((code, why)) => {
        eprintln!("mytool: {why}");
        return ExitCode::from(code);
    }
};
```

`read_text` tells a file that cannot be read from one that is not text: the
first is an I/O failure (`74`) and the second is bad input (`65`). A caller
routing on the exit code needs to tell those apart — one is worth retrying and
the other never is.

`read_lossy` is the same but replaces anything that is not UTF-8, for formats
that routinely arrive in another encoding. A subtitle file downloaded from
anywhere is as likely to be Latin-1, and a cue with one wrong character still
has its timing and most of its words.

Both read stdin when given no path.

## Writing the output

```rust
catena_cli::stream("mytool", &records)   // NDJSON, one per line, flushed
catena_cli::text("mytool", &cleaned)     // plain text
```

Both return the exit code to leave with. Both treat a consumer that closed the
pipe as the end of the job rather than as a failure — `… | head -1` exits `0`,
like every other filter — and both flush as they go, so a slow stage
downstream sees a record as soon as it exists.

For a loop that needs to do something between records:

```rust
let mut out = std::io::BufWriter::new(std::io::stdout().lock());
for record in records {
    // …
}
catena_cli::finish("mytool", out)
```

## A whole filter

```rust
use std::process::ExitCode;
use clap::Parser;

#[derive(Parser)]
#[command(version, about)]
struct Args {
    path: Option<std::path::PathBuf>,
}

fn main() -> ExitCode {
    let args: Args = match catena_cli::parse() {
        Ok(args) => args,
        Err(code) => return code,
    };
    let text = match catena_cli::read_text(args.path.as_deref()) {
        Ok(text) => text,
        Err((code, why)) => {
            eprintln!("mytool: {why}");
            return ExitCode::from(code);
        }
    };
    catena_cli::stream("mytool", &do_the_work(&text))
}
```

That is the whole shape. Everything a tool here actually *does* lives in its
library crate; the command is argument parsing, input, output and an exit
code, and should stay under a hundred and fifty lines.

## See also

- [The contract](contract.md) — what these functions are enforcing.
- [`corpus-core`](corpus-core.md) — the records and the exit-code constants.
