//! A deterministic sample of awkward text.
//!
//! Generated rather than committed, so the fixture costs the clone nothing,
//! and generated the same way on every machine, so Rust and Python are
//! demonstrably reading the same bytes. Change anything in here and every
//! golden file derived from it has to be regenerated.
//!
//! The lines are chosen to be hostile to a tokeniser rather than
//! representative of prose: scripts that are not Latin, combining marks,
//! zero-width characters, the literal spelling of a control token, numbers,
//! URLs, markdown, and runs of punctuation. Ordinary English is the easy case
//! and is well covered by the corpus itself.

/// Lines in the standard sample.
pub const LINES: usize = 10_000;

/// The seed the standard sample is generated from.
pub const SEED: u64 = 0x636174656e610001;

/// The standard sample: [`LINES`] lines, newline-separated, no trailing
/// newline, no carriage returns.
///
/// Carriage returns are excluded deliberately: Python's universal-newline
/// translation would turn a lone `\r` into a line break and the two sides
/// would then disagree about how many lines there even are.
pub fn standard() -> String {
    generate(SEED, LINES)
}

/// Generate `lines` lines from `seed`.
pub fn generate(seed: u64, lines: usize) -> String {
    let mut rng = Rng::new(seed);
    let mut out = String::new();
    for i in 0..lines {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&line(&mut rng));
    }
    out
}

fn line(rng: &mut Rng) -> String {
    match rng.below(10) {
        0 => words(rng, 3, 14),
        1 => format!("{} {}", pick(rng, SCRIPTS), words(rng, 1, 6)),
        2 => format!("# {}", words(rng, 2, 7)),
        3 => format!(
            "- [{}]({}) {}",
            words(rng, 1, 4),
            pick(rng, URLS),
            words(rng, 0, 5)
        ),
        4 => format!("{} {} {}", numbers(rng), pick(rng, PUNCT), numbers(rng)),
        5 => format!("{}{}", pick(rng, AWKWARD), words(rng, 0, 6)),
        6 => format!("    {}", pick(rng, CODE)),
        7 => format!(
            "{}{}",
            words(rng, 1, 5),
            pick(rng, PUNCT).repeat(rng.below(4) + 1)
        ),
        8 => words(rng, 20, 60),
        _ => String::new(),
    }
}

fn words(rng: &mut Rng, least: usize, most: usize) -> String {
    let count = least + rng.below(most.saturating_sub(least) + 1);
    let mut out = String::new();
    for i in 0..count {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(pick(rng, WORDS));
    }
    out
}

fn numbers(rng: &mut Rng) -> String {
    match rng.below(4) {
        0 => format!("{}", rng.below(10)),
        1 => format!("{}", rng.below(100_000)),
        2 => format!("{}.{}", rng.below(1000), rng.below(1000)),
        _ => format!("{}-{}", rng.below(2100), rng.below(13)),
    }
}

fn pick<'a>(rng: &mut Rng, from: &[&'a str]) -> &'a str {
    from[rng.below(from.len())]
}

const WORDS: &[&str] = &[
    "the",
    "second",
    "estate",
    "manuscript",
    "marginalia",
    "colophon",
    "quire",
    "recto",
    "verso",
    "palimpsest",
    "incunabulum",
    "folio",
    "gloss",
    "catena",
    "scholion",
    "lacuna",
    "obelus",
    "supralinear",
    "hapax",
    "stemma",
    "apparatus",
    "witness",
    "conjecture",
    "lemma",
];

const SCRIPTS: &[&str] = &[
    "café",
    "naïve",
    "Москва",
    "東京",
    "مرحبا",
    "Ἀθῆναι",
    "עברית",
    "ॐ",
    "Ægypt",
    "ﬁrst",
    "Straße",
];

const URLS: &[&str] = &[
    "https://example.org/a/b?c=d&e=f",
    "http://localhost:8080/x",
    "#anchor-name",
    "mailto:someone@example.invalid",
];

const PUNCT: &[&str] = &[".", "!", "?", "…", "—", ";", ":", ",", ")", "\"", "'"];

const CODE: &[&str] = &[
    "fn main() { println!(\"{}\", 1 + 1); }",
    "SELECT * FROM t WHERE id = $1;",
    "{\"v\":1,\"idx\":0}",
    "<!-- Page 41 -->",
];

/// Characters that have broken something before: a zero-width joiner, a
/// non-breaking space, a combining acute, the literal spelling of a control
/// token, and a tab.
const AWKWARD: &[&str] = &[
    "\u{200d}",
    "\u{00a0}",
    "e\u{0301}",
    "<|endoftext|>",
    "\t",
    "\u{feff}",
    "🙂",
    "\u{1c}",
];

/// xorshift64*, so the sample is the same on every machine and every run.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 { 1 } else { seed })
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sample_is_the_same_every_time() {
        assert_eq!(generate(SEED, 50), generate(SEED, 50));
    }

    #[test]
    fn a_shorter_sample_is_a_prefix_of_a_longer_one() {
        let long = generate(SEED, 50);
        let short = generate(SEED, 10);
        assert!(long.starts_with(&short));
    }

    #[test]
    fn the_standard_sample_has_the_advertised_shape() {
        let text = standard();
        assert_eq!(text.split('\n').count(), LINES);
        assert!(!text.contains('\r'));
        assert!(!text.ends_with('\n'));
    }
}
