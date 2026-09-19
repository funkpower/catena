//! The parts of a filter that every tool here does the same way.
//!
//! Not a framework — three functions, each of which exists because getting
//! it wrong is easy and the consequence is a pipeline that misreports why it
//! stopped:
//!
//! - a usage error exits 64, not whatever the argument parser felt like;
//! - a consumer that closed the pipe is the end of the job, not a failure;
//! - a record is flushed as it is written, so a slow stage downstream sees it
//!   immediately.
//!
//! It also gives every tool `--man`, so that the manual page and the help
//! text can never say different things.

use std::io::{BufWriter, Write};
use std::process::ExitCode;

use corpus_core::{exit, ndjson};
use serde::Serialize;

/// Parse the command line, or exit.
///
/// `--help` and `--version` are not errors and leave with 0; anything else
/// wrong with the arguments leaves with 64, which is what the rest of the
/// suite means by "the command line was wrong".
pub fn parse<T: clap::Parser>() -> Result<T, ExitCode> {
    let command = T::command().arg(
        clap::Arg::new("man")
            .long("man")
            .action(clap::ArgAction::SetTrue)
            .help("Write this page as a manual page, in roff"),
    );

    // Looked for before parsing, the way `--help` is: a mode, not an option,
    // and not blocked by an argument the mode does not need. Anything after a
    // bare `--` is a value and never a flag.
    if std::env::args()
        .skip(1)
        .take_while(|argument| argument != "--")
        .any(|argument| argument == "--man")
    {
        return Err(man(command));
    }

    let leave = |e: clap::Error| {
        let _ = e.print();
        match e.use_stderr() {
            true => ExitCode::from(exit::USAGE),
            false => ExitCode::from(exit::OK),
        }
    };
    let matches = command.try_get_matches().map_err(leave)?;
    T::from_arg_matches(&matches).map_err(leave)
}

/// Write the manual page for a command and leave.
///
/// Generated from the same definition `--help` is, because a manual page kept
/// by hand is a manual page that is wrong.
fn man(command: clap::Command) -> ExitCode {
    // A package holding two commands would otherwise put the package's name
    // at the head of both pages, and `man vocabmerge` would open bibclean.
    let command = match invoked_as() {
        // Leaked because clap holds names for the life of the program,
        // which from here is one page of roff and an exit.
        Some(name) => command.name(&*name.leak()),
        None => command,
    };

    let mut roff = Vec::new();
    if let Err(e) = clap_mangen::Man::new(command).render(&mut roff) {
        eprintln!("man: {e}");
        return ExitCode::from(exit::SOFTWARE);
    }
    match String::from_utf8(roff) {
        Ok(page) => text("man", &page),
        Err(_) => ExitCode::from(exit::SOFTWARE),
    }
}

/// What this program was called, without a path and without an extension.
fn invoked_as() -> Option<String> {
    let argv0 = std::env::args_os().next()?;
    let name = std::path::Path::new(&argv0).file_stem()?.to_str()?;
    match name.is_empty() {
        true => None,
        false => Some(name.to_string()),
    }
}

/// Write records to stdout as NDJSON, one per line, flushed as they go.
pub fn stream<'a, T: Serialize + 'a>(
    tool: &str,
    records: impl IntoIterator<Item = &'a T>,
) -> ExitCode {
    let mut out = BufWriter::new(std::io::stdout().lock());
    for record in records {
        match ndjson::write_line(&mut out, record) {
            Ok(()) => {}
            // A consumer that stopped reading is not a failure: `… | head` is
            // a normal way to look at a stream.
            Err(e) if e.is_broken_pipe() => return ExitCode::from(exit::OK),
            Err(e) => {
                eprintln!("{tool}: {e}");
                return ExitCode::from(exit::IO);
            }
        }
    }
    finish(tool, out)
}

/// Write text to stdout and leave.
pub fn text(tool: &str, text: &str) -> ExitCode {
    let mut out = BufWriter::new(std::io::stdout().lock());
    match out.write_all(text.as_bytes()) {
        Ok(()) => finish(tool, out),
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::from(exit::OK),
        Err(e) => {
            eprintln!("{tool}: {e}");
            ExitCode::from(exit::IO)
        }
    }
}

/// Flush what is buffered, treating a closed consumer as the end of the job.
pub fn finish(tool: &str, mut out: impl Write) -> ExitCode {
    match out.flush() {
        Ok(()) => ExitCode::from(exit::OK),
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::from(exit::OK),
        Err(e) => {
            eprintln!("{tool}: {e}");
            ExitCode::from(exit::IO)
        }
    }
}

/// Read the whole input as text, telling a file that cannot be read from one
/// that is not text.
///
/// The two are different answers: the first is an I/O failure and the second
/// is bad input, and a caller routing on the exit code needs to tell them
/// apart.
pub fn read_text(path: Option<&std::path::Path>) -> Result<String, (u8, String)> {
    let bytes = read_bytes(path)?;
    String::from_utf8(bytes).map_err(|e| (exit::DATA, format!("input is not UTF-8: {e}")))
}

/// Read the whole input, replacing anything that is not UTF-8.
///
/// For the formats that routinely arrive in another encoding — a subtitle
/// file downloaded from anywhere is as likely to be Latin-1 — where a cue
/// with one wrong character still has its timing and most of its words.
pub fn read_lossy(path: Option<&std::path::Path>) -> Result<String, (u8, String)> {
    Ok(String::from_utf8_lossy(&read_bytes(path)?).into_owned())
}

fn read_bytes(path: Option<&std::path::Path>) -> Result<Vec<u8>, (u8, String)> {
    use std::io::Read as _;

    match path {
        Some(path) => {
            std::fs::read(path).map_err(|e| (exit::IO, format!("{}: {e}", path.display())))
        }
        None => {
            let mut bytes = Vec::new();
            std::io::stdin()
                .read_to_end(&mut bytes)
                .map(|_| bytes)
                .map_err(|e| (exit::IO, e.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_a_file_that_is_not_there_is_an_io_failure() {
        let missing = std::path::Path::new("/definitely/not/here");
        let (code, _) = read_text(Some(missing)).expect_err("should fail");
        assert_eq!(code, exit::IO);
    }

    #[test]
    fn reading_bytes_that_are_not_text_is_a_data_failure() {
        let path = std::env::temp_dir().join("catena-cli-not-text");
        std::fs::write(&path, [0xff, 0xfe]).expect("a writable path");
        let (code, why) = read_text(Some(&path)).expect_err("should fail");
        assert_eq!(code, exit::DATA);
        assert!(why.contains("UTF-8"), "{why}");
    }

    #[test]
    fn the_lossy_read_takes_what_the_strict_one_refuses() {
        let path = std::env::temp_dir().join("catena-cli-lossy");
        std::fs::write(&path, [b'a', 0xff, b'b']).expect("a writable path");
        assert_eq!(read_lossy(Some(&path)).expect("should read"), "a\u{fffd}b");
    }

    #[test]
    fn a_closed_consumer_is_the_end_of_the_job_and_not_a_failure() {
        struct Closed;
        impl Write for Closed {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Ok(0)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "broken pipe",
                ))
            }
        }
        assert_eq!(
            format!("{:?}", finish("test", Closed)),
            format!("{:?}", ExitCode::from(exit::OK))
        );
    }

    /// A command shaped like the tools here: a required path and a flag.
    #[derive(clap::Parser)]
    #[command(name = "example", about = "An example.")]
    struct Example {
        /// The file to read.
        path: std::path::PathBuf,
        /// Emit a record.
        #[arg(long)]
        json: bool,
    }

    fn page() -> String {
        let command = <Example as clap::CommandFactory>::command();
        let mut roff = Vec::new();
        clap_mangen::Man::new(command)
            .render(&mut roff)
            .expect("a rendered page");
        String::from_utf8(roff).expect("text")
    }

    #[test]
    fn the_manual_page_says_what_the_help_says() {
        let page = page();
        assert!(page.contains(".TH example 1"), "{page}");
        assert!(page.contains("An example."), "{page}");
        assert!(page.contains("\\-\\-json"), "{page}");
    }

    #[test]
    fn the_manual_page_describes_arguments_the_command_requires() {
        // The page has to describe a command whose arguments are required,
        // which is why it cannot be reached by parsing them first.
        assert!(page().contains("PATH"), "{}", page());
    }
}
