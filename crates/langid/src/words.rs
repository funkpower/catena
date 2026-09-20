//! Telling apart the languages that share an alphabet, by their commonest
//! words.
//!
//! Function words — articles, prepositions, conjunctions, pronouns — make up
//! a third of any running text and differ sharply between languages even
//! closely related ones. Counting them needs no model, no training and no
//! network, and on a paragraph or more it is decisive.
//!
//! It is weak on a single line, which is why what comes out carries a margin
//! and a caller can refuse a close call.

use crate::script::Script;

/// One language, and the words that give it away.
pub struct Language {
    /// ISO 639-1, the two-letter code everything else uses.
    pub code: &'static str,
    /// ISO 639-2/B, for the `doc` record's language field.
    pub bibliographic: &'static str,
    /// What it is called in English.
    pub name: &'static str,
    /// The script it is written in.
    pub script: Script,
    /// Its commonest function words, lowercased.
    pub common: &'static [&'static str],
}

/// The languages this can tell apart.
///
/// General reference data, of the same kind as the HTML character references
/// and the ISBN check digit: true everywhere, not a fact about one
/// collection.
pub const LANGUAGES: &[Language] = &[
    Language {
        code: "en",
        bibliographic: "eng",
        name: "English",
        script: Script::Latin,
        common: &[
            "the", "of", "and", "to", "in", "that", "is", "was", "it", "for", "with", "as", "his",
            "on", "be", "at", "by", "this", "had", "not", "are", "but", "from", "or", "have", "an",
            "they", "which", "were", "their",
        ],
    },
    Language {
        code: "fr",
        bibliographic: "fre",
        name: "French",
        script: Script::Latin,
        common: &[
            "le", "de", "un", "et", "être", "à", "il", "avoir", "ne", "je", "son", "que", "se",
            "qui", "ce", "dans", "en", "du", "elle", "au", "pour", "pas", "que", "vous", "par",
            "sur", "les", "des", "une", "plus",
        ],
    },
    Language {
        code: "de",
        bibliographic: "ger",
        name: "German",
        script: Script::Latin,
        common: &[
            "der", "die", "und", "in", "den", "von", "zu", "das", "mit", "sich", "des", "auf",
            "für", "ist", "im", "dem", "nicht", "ein", "eine", "als", "auch", "es", "an", "werden",
            "aus", "er", "hat", "dass", "sie", "nach",
        ],
    },
    Language {
        code: "es",
        bibliographic: "spa",
        name: "Spanish",
        script: Script::Latin,
        common: &[
            "de", "la", "que", "el", "en", "y", "a", "los", "del", "se", "las", "por", "un",
            "para", "con", "no", "una", "su", "al", "es", "lo", "como", "más", "pero", "sus", "le",
            "ya", "o", "este", "sí",
        ],
    },
    Language {
        code: "it",
        bibliographic: "ita",
        name: "Italian",
        script: Script::Latin,
        common: &[
            "di", "che", "e", "il", "la", "in", "un", "a", "per", "non", "una", "è", "con", "si",
            "da", "le", "del", "i", "sono", "come", "al", "della", "nel", "più", "lo", "ma", "se",
            "gli", "alla", "anche",
        ],
    },
    Language {
        code: "pt",
        bibliographic: "por",
        name: "Portuguese",
        script: Script::Latin,
        common: &[
            "de", "a", "o", "que", "e", "do", "da", "em", "um", "para", "com", "não", "uma", "os",
            "no", "se", "na", "por", "mais", "as", "dos", "como", "mas", "ao", "ele", "das", "seu",
            "sua", "ou", "quando",
        ],
    },
    Language {
        code: "nl",
        bibliographic: "dut",
        name: "Dutch",
        script: Script::Latin,
        common: &[
            "de", "van", "het", "een", "en", "in", "is", "dat", "op", "te", "zijn", "met", "voor",
            "niet", "aan", "er", "om", "maar", "dan", "of", "door", "over", "ze", "bij", "ook",
            "tot", "uit", "naar", "heeft", "werd",
        ],
    },
    Language {
        code: "la",
        bibliographic: "lat",
        name: "Latin",
        script: Script::Latin,
        common: &[
            "et", "in", "est", "non", "ad", "cum", "qui", "quod", "ut", "sed", "de", "si", "ex",
            "quae", "per", "hoc", "esse", "enim", "autem", "nec", "atque", "sunt", "eius", "ab",
            "quam", "tamen", "ita", "etiam", "ipse", "sive",
        ],
    },
    Language {
        code: "pl",
        bibliographic: "pol",
        name: "Polish",
        script: Script::Latin,
        common: &[
            "w", "i", "z", "na", "się", "do", "nie", "to", "że", "jest", "o", "a", "od", "po",
            "za", "jak", "ale", "tym", "tego", "przez", "przy", "dla", "co", "który", "była",
            "były", "być", "ma", "już", "tylko",
        ],
    },
    Language {
        code: "sv",
        bibliographic: "swe",
        name: "Swedish",
        script: Script::Latin,
        common: &[
            "och", "att", "det", "som", "en", "på", "är", "av", "för", "med", "till", "den", "har",
            "de", "inte", "om", "ett", "han", "men", "var", "från", "vi", "så", "kan", "man",
            "när", "hon", "sig", "eller", "efter",
        ],
    },
    Language {
        code: "ru",
        bibliographic: "rus",
        name: "Russian",
        script: Script::Cyrillic,
        common: &[
            "и", "в", "не", "на", "что", "он", "с", "как", "это", "по", "но", "они", "к", "у",
            "же", "за", "из", "от", "то", "все", "был", "была", "было", "так", "его", "её", "для",
            "бы", "или", "мы",
        ],
    },
    Language {
        code: "uk",
        bibliographic: "ukr",
        name: "Ukrainian",
        script: Script::Cyrillic,
        common: &[
            "і", "в", "не", "на", "що", "з", "як", "до", "це", "за", "від", "але", "той", "його",
            "вона", "там", "бути", "має", "для", "та", "він", "ми", "ви", "чи", "або", "щоб",
            "коли", "де", "так", "ще",
        ],
    },
    Language {
        code: "el",
        bibliographic: "gre",
        name: "Greek",
        script: Script::Greek,
        common: &[
            "και",
            "το",
            "της",
            "των",
            "του",
            "στο",
            "με",
            "για",
            "που",
            "στη",
            "είναι",
            "από",
            "σε",
            "τα",
            "ο",
            "η",
            "δεν",
            "να",
            "ως",
            "κατά",
            "μια",
            "ένα",
            "αλλά",
            "αυτό",
            "όπως",
            "θα",
            "τον",
            "την",
            "οι",
            "στον",
        ],
    },
];

/// How far ahead the best language must be before the answer is worth giving.
///
/// A ratio rather than a difference, so it means the same on a paragraph and
/// on a book.
pub const MARGIN: f64 = 1.25;

/// Score every language whose script matches, best first.
///
/// The score is the share of a text's words that are among a language's
/// commonest. It is comparable across languages because each list is the same
/// length.
pub fn score(text: &str, script: Script) -> Vec<(&'static Language, f64)> {
    let words = tokens(text);
    if words.is_empty() {
        return Vec::new();
    }

    let mut scored: Vec<(&'static Language, f64)> = LANGUAGES
        .iter()
        .filter(|language| language.script == script)
        .map(|language| {
            let hits = words
                .iter()
                .filter(|word| language.common.contains(&word.as_str()))
                .count();
            (language, hits as f64 / words.len() as f64)
        })
        .collect();

    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    scored
}

/// A text's words, lowercased, with everything that is not a letter dropped.
fn tokens(text: &str) -> Vec<String> {
    text.split(|c: char| crate::script::of(c).is_none())
        .filter(|word| !word.is_empty())
        .map(|word| word.to_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn best(text: &str, script: Script) -> Option<&'static str> {
        score(text, script)
            .first()
            .map(|(language, _)| language.code)
    }

    #[test]
    fn each_language_is_recognised_from_a_sentence_of_it() {
        for (text, code) in [
            (
                "The obligation ran both ways, and the lord who could not protect was not a lord at all in that time",
                "en",
            ),
            (
                "Le seigneur qui ne pouvait pas protéger n'était pas un seigneur, et c'est ce que les rôles disent de lui",
                "fr",
            ),
            (
                "Der Herr, der nicht schützen konnte, war kein Herr, und das ist es, was die Urkunden von ihm sagen",
                "de",
            ),
            (
                "El señor que no podía proteger no era un señor, y eso es lo que dicen los registros de él en la época",
                "es",
            ),
            (
                "Il signore che non poteva proteggere non era un signore, e questo è ciò che dicono i documenti di lui",
                "it",
            ),
            (
                "O senhor que não podia proteger não era um senhor, e é isso que os registos dizem dele naquele tempo",
                "pt",
            ),
            (
                "De heer die niet kon beschermen was geen heer, en dat is wat de bronnen over hem zeggen in die tijd",
                "nl",
            ),
            (
                "Dominus qui non poterat protegere non erat dominus, et hoc est quod de eo dicunt scripta illius aetatis",
                "la",
            ),
        ] {
            assert_eq!(best(text, Script::Latin), Some(code), "{text}");
        }
    }

    #[test]
    fn the_cyrillic_pair_are_told_apart() {
        assert_eq!(
            best(
                "Молчание было единственным ответом на его вопрос, и это всё что он услышал",
                Script::Cyrillic
            ),
            Some("ru")
        );
        assert_eq!(
            best(
                "Мовчання було єдиною відповіддю на його запитання, і це все що він почув від них",
                Script::Cyrillic
            ),
            Some("uk")
        );
    }

    #[test]
    fn greek_is_read_as_greek() {
        assert_eq!(
            best(
                "Και το φως εν τη σκοτία φαίνει και η σκοτία αυτό δεν κατέλαβεν από την αρχή",
                Script::Greek
            ),
            Some("el")
        );
    }

    #[test]
    fn a_text_with_no_words_scores_nothing() {
        assert!(score("", Script::Latin).is_empty());
        assert!(score("123 456", Script::Latin).is_empty());
    }

    #[test]
    fn only_languages_in_the_right_script_are_considered() {
        let latin = score("the and of", Script::Latin);
        assert!(
            latin
                .iter()
                .all(|(language, _)| language.script == Script::Latin)
        );
        assert!(!latin.is_empty());
    }

    #[test]
    fn every_language_has_the_same_number_of_words_so_scores_compare() {
        let first = LANGUAGES[0].common.len();
        for language in LANGUAGES {
            assert_eq!(
                language.common.len(),
                first,
                "{} has a different list",
                language.code
            );
        }
    }
}
