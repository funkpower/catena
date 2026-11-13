//! What a tool's exit code means.
//!
//! A predicate answers through its status, so it is usable in a shell `if`
//! without parsing anything. That only works if "no" and "broke" are
//! different numbers, which is why the failure codes below start where
//! `sysexits.h` puts them and leave 0, 1 and 2 to the answer.
//!
//! One case has no code of its own on purpose: a consumer closing the pipe.
//! `… | head -1` is a normal way to use a filter, and a filter that reports
//! failure because someone stopped reading would make every such pipeline
//! look broken. A tool that sees a broken pipe stops and exits [`OK`].

use std::process::ExitCode;

/// Success, or a predicate answering yes.
pub const OK: u8 = 0;
/// A predicate answering no.
pub const NO: u8 = 1;
/// A predicate that could not tell — a damaged file, a format it does not
/// know. Distinct from no, because a caller may want to retry or escalate.
pub const INDETERMINATE: u8 = 2;
/// The command line was wrong.
pub const USAGE: u8 = 64;
/// The input was not what the tool can work on: bad JSON, a record from a
/// version this build does not implement, bytes that are not text.
pub const DATA: u8 = 65;
/// An external program the tool needs is not on `PATH`.
pub const UNAVAILABLE: u8 = 69;
/// The tool failed at something that is nobody's fault but its own.
pub const SOFTWARE: u8 = 70;
/// Reading or writing failed.
pub const IO: u8 = 74;
/// A service refused for now, and asking again later will work.
pub const TEMPFAIL: u8 = 75;

/// The answer a predicate tool gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Yes.
    Yes,
    /// No.
    No,
    /// Could not tell.
    Unknown,
}

impl Verdict {
    /// The process exit code for this answer.
    pub const fn code(self) -> u8 {
        match self {
            Verdict::Yes => OK,
            Verdict::No => NO,
            Verdict::Unknown => INDETERMINATE,
        }
    }
}

impl From<bool> for Verdict {
    fn from(yes: bool) -> Self {
        if yes { Verdict::Yes } else { Verdict::No }
    }
}

impl From<Verdict> for ExitCode {
    fn from(verdict: Verdict) -> Self {
        ExitCode::from(verdict.code())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_answers_are_zero_one_two() {
        assert_eq!(Verdict::Yes.code(), 0);
        assert_eq!(Verdict::No.code(), 1);
        assert_eq!(Verdict::Unknown.code(), 2);
    }

    #[test]
    fn failure_codes_never_collide_with_an_answer() {
        for code in [USAGE, DATA, UNAVAILABLE, SOFTWARE, IO] {
            assert!(code > INDETERMINATE);
        }
    }
}
