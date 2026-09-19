//! Cleaning the markup out of a cue's text.

use std::sync::LazyLock;

use regex::Regex;

/// Styling and speaker tags: `<i>`, `<v Speaker>`, `<c.colour>`.
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").expect("a valid pattern"));

/// Bracketed sound cues: `[Music]`, `[Applause]`, `[inaudible]`.
///
/// Not speech, and a search that matches them matches every recording with a
/// theme tune.
static BRACKETED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[.*?\]").expect("a valid pattern"));

/// Strip markup from a cue's text.
pub fn clean(text: &str) -> String {
    let text = TAG.replace_all(text, "");
    let text = BRACKETED.replace_all(&text, "");
    text.trim_matches(is_js_space).to_string()
}

/// What JavaScript's `String.trim` removes: Unicode whitespace and the
/// byte-order mark.
fn is_js_space(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styling_tags_go() {
        assert_eq!(clean("<i>The mill</i> stood"), "The mill stood");
        assert_eq!(clean("<v Speaker>Hello"), "Hello");
    }

    #[test]
    fn bracketed_sound_cues_go() {
        assert_eq!(clean("[Music] The mill stood [Applause]"), "The mill stood");
        assert_eq!(clean("[Music]"), "");
    }

    #[test]
    fn the_bracket_match_is_not_greedy() {
        assert_eq!(clean("[a] kept [b]"), "kept");
    }

    #[test]
    fn a_stray_pair_of_angle_brackets_takes_what_is_between_them() {
        // Faithful to the original, which uses the same pattern. A spoken
        // line with a less-than and a greater-than in it is vanishingly rare
        // next to the styling tags this exists to remove.
        assert_eq!(clean("a < b and c > d"), "a  d");
    }

    #[test]
    fn a_lone_angle_bracket_is_left_alone() {
        assert_eq!(clean("a < b"), "a < b");
    }
}
