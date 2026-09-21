//! Repairs that can be made without looking anything up.

use std::sync::LazyLock;

use regex::Regex;
use textkit::pattern::SPACE;

/// A format name stuck on the end of a title taken from a filename.
static FORMAT_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)[_\-{SPACE}]*(txt|pdf|epub|mobi|azw3?|htmlz?|djvu){SPACE}*$"
    ))
    .expect("a valid pattern")
});

/// An underscore standing in for the colon before a subtitle.
static SUBTITLE_UNDERSCORE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"_{SPACE}+")).expect("a valid pattern"));

/// Two or more spaces where one belongs.
static SPACE_RUN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"{SPACE}{{2,}}")).expect("a valid pattern"));

/// A run of letters, by Python's definition of a letter.
static LETTERS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\p{L}+").expect("a valid pattern"));

/// Words that stay lowercase inside a title.
const SMALL_WORDS: &[&str] = &[
    "a", "an", "and", "as", "at", "but", "by", "for", "from", "in", "into", "nor", "of", "on",
    "or", "over", "per", "the", "to", "upon", "via", "with",
];

/// Whether a title is in capitals throughout.
///
/// A space is required, so a title that is only an acronym is left alone.
pub fn is_shouting(title: &str) -> bool {
    let title = title.trim_matches(textkit::is_space);
    !title.is_empty()
        && title == title.to_uppercase()
        && title.contains(' ')
        && title
            .chars()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|pair| pair[0].is_ascii_uppercase() && pair[1].is_ascii_uppercase())
}

/// Title-case a shouting title without mangling it.
///
/// The obvious library function is unusable here: it gives "Don'T" and
/// capitalises every preposition. Small words stay lowercase unless they open
/// or close the title, hyphenated parts are cased on both sides, and the
/// first *letter* is capitalised rather than the first character — an
/// ASCII-only rule skips an accented opening letter and capitalises the one
/// after it, turning "ÊTRE" into "êTre", and a third of a real library is not
/// in English.
pub fn title_case(title: &str) -> String {
    let words: Vec<&str> = title.split_whitespace().collect();
    let last = words.len().saturating_sub(1);

    words
        .iter()
        .enumerate()
        .map(|(at, word)| {
            let core: String = word
                .trim_start_matches(['(', '[', '"', '\''])
                .to_lowercase()
                .trim_end_matches(['.', ',', ':', ';', '!', '?', ')', ']', '"', '\''])
                .to_string();
            // A small word stays small in the middle of a title, but not
            // where a title actually starts. A colon starts a subtitle, and
            // "Aesthetic in Kant: a Critique" is the giveaway that the rule
            // was only ever counting from the beginning of the whole string.
            let opens_a_subtitle = at > 0
                && words[at - 1]
                    .trim_end_matches(['"', '\'', ')', ']'])
                    .ends_with([':', ';', '?', '!', '.']);
            if SMALL_WORDS.binary_search(&core.as_str()).is_ok()
                && at > 0
                && at < last
                && !opens_a_subtitle
            {
                return word.to_lowercase();
            }
            let lowered = word.to_lowercase();
            if lowered.contains('-') {
                lowered
                    .split('-')
                    .map(capitalise)
                    .collect::<Vec<_>>()
                    .join("-")
            } else {
                capitalise_first_letters(&lowered)
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Undo the underscore mangling a filename-derived title carries.
///
/// Three habits, in order: a format name on the end, an underscore standing
/// in for the colon before a subtitle, and underscores used as plain spaces.
pub fn clean_underscores(title: &str) -> String {
    let title = FORMAT_SUFFIX.replace(title, "");
    let title = title.trim_matches(textkit::is_space);
    let title = SUBTITLE_UNDERSCORE.replace_all(title, ": ");
    let title = title.replace('_', " ");
    SPACE_RUN
        .replace_all(&title, " ")
        .trim_matches(|c: char| textkit::is_space(c) || c == ':' || c == ',' || c == '-')
        .to_string()
}

/// Capitalise the first letter and lowercase the rest.
fn capitalise(word: &str) -> String {
    let lowered = word.to_lowercase();
    capitalise_first_letters(&lowered)
}

/// Capitalise the first run of letters, leaving everything before it alone.
fn capitalise_first_letters(lowered: &str) -> String {
    match LETTERS.find(lowered) {
        None => lowered.to_string(),
        Some(found) => {
            let mut out = String::with_capacity(lowered.len());
            out.push_str(&lowered[..found.start()]);
            let mut letters = found.as_str().chars();
            if let Some(first) = letters.next() {
                out.extend(first.to_uppercase());
                out.push_str(letters.as_str());
            }
            out.push_str(&lowered[found.end()..]);
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_small_word_list_is_sorted_so_it_can_be_searched() {
        assert!(SMALL_WORDS.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn shouting_needs_more_than_one_word() {
        assert!(is_shouting("THE HUMAN CONDITION"));
        assert!(!is_shouting("MIT"));
        assert!(!is_shouting("The Human Condition"));
        assert!(!is_shouting(""));
        assert!(!is_shouting("A B"));
    }

    #[test]
    fn a_shouting_title_comes_back_readable() {
        assert_eq!(title_case("THE HUMAN CONDITION"), "The Human Condition");
        assert_eq!(
            title_case("A HISTORY OF THE WORLD IN TEN CHAPTERS"),
            "A History of the World in Ten Chapters"
        );
    }

    #[test]
    fn a_small_word_at_either_end_is_capitalised() {
        assert_eq!(title_case("OF MICE AND MEN"), "Of Mice and Men");
        assert_eq!(title_case("WHAT IS IT ALL FOR"), "What Is It All For");
    }

    #[test]
    fn an_apostrophe_does_not_start_a_new_word() {
        assert_eq!(title_case("DON'T LOOK NOW"), "Don't Look Now");
    }

    #[test]
    fn a_hyphenated_word_is_cased_on_both_sides() {
        assert_eq!(title_case("THE WELL-BUILT WHEEL"), "The Well-Built Wheel");
    }

    #[test]
    fn an_accented_first_letter_is_the_one_capitalised() {
        // The small-word list is English, so a French preposition is cased
        // like any other word. That is what the original does, and a list per
        // language is a different tool.
        assert_eq!(title_case("ÊTRE ET TEMPS"), "Être Et Temps");
        assert_eq!(
            title_case("ÉTUDES SUR LE MOYEN ÂGE"),
            "Études Sur Le Moyen Âge"
        );
    }

    #[test]
    fn a_bracketed_word_keeps_its_bracket() {
        assert_eq!(title_case("THE (SECOND) ESTATE"), "The (Second) Estate");
    }

    #[test]
    fn a_format_suffix_comes_off() {
        assert_eq!(clean_underscores("The Fall_txt"), "The Fall");
        assert_eq!(clean_underscores("The Fall - pdf"), "The Fall");
        // A format name takes no separator to be one, so an extension goes
        // too and leaves its dot behind. Faithful to the original; a title
        // that is really a filename is being repaired either way.
        assert_eq!(clean_underscores("The Fall.epub"), "The Fall.");
    }

    #[test]
    fn an_underscore_before_a_subtitle_becomes_a_colon() {
        assert_eq!(
            clean_underscores("The 9/11 Mystery Plane_ And the Vanishing of America"),
            "The 9/11 Mystery Plane: And the Vanishing of America"
        );
    }

    #[test]
    fn underscores_used_as_spaces_become_spaces() {
        assert_eq!(
            clean_underscores("Board_Advisors_etc"),
            "Board Advisors etc"
        );
    }

    #[test]
    fn a_title_with_nothing_wrong_with_it_is_unchanged() {
        assert_eq!(
            clean_underscores("The Human Condition"),
            "The Human Condition"
        );
    }
}

#[cfg(test)]
mod subtitle_case_tests {
    use super::title_case;

    #[test]
    fn a_subtitle_starts_with_a_capital() {
        // The rule counted small words from the start of the string, so every
        // subtitle beginning "A", "The" or "Of" was set lower case.
        assert_eq!(
            title_case("AESTHETIC IN KANT: A CRITIQUE"),
            "Aesthetic in Kant: A Critique"
        );
        assert_eq!(
            title_case("BORN FIGHTING: THE SCOTS-IRISH"),
            "Born Fighting: The Scots-Irish"
        );
        assert_eq!(
            title_case("A HISTORY: AN INTRODUCTION TO IT"),
            "A History: An Introduction to It"
        );
    }

    #[test]
    fn a_small_word_in_the_middle_is_still_small() {
        assert_eq!(
            title_case("THE RISE AND FALL OF THE THIRD REICH"),
            "The Rise and Fall of the Third Reich"
        );
        assert_eq!(title_case("WAR OF THE WORLDS"), "War of the Worlds");
    }
}
