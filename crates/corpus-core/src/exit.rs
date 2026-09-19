//! What a tool's exit code means.
//!
//! A predicate answers through its status, so it is usable in a shell `if`
//! without parsing anything. That only works if "no" and "broke" are
//! different numbers, which is why the failure codes below start where
//! `sysexits.h` puts them and leave 0, 1 and 2 to the answer.

/// Success, or a predicate answering yes.
pub const OK: i32 = 0;
/// A predicate answering no.
pub const NO: i32 = 1;
/// A predicate that could not tell — a damaged file, a format it does not
/// know. Distinct from no, because a caller may want to retry or escalate.
pub const INDETERMINATE: i32 = 2;
/// The command line was wrong.
pub const USAGE: i32 = 64;
/// The input was not what the tool can work on.
pub const DATA: i32 = 65;
/// An external program the tool needs is not on `PATH`.
pub const UNAVAILABLE: i32 = 69;
/// Reading or writing failed.
pub const IO: i32 = 74;

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
    pub const fn code(self) -> i32 {
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
        for code in [USAGE, DATA, UNAVAILABLE, IO] {
            assert!(code > INDETERMINATE);
        }
    }
}
