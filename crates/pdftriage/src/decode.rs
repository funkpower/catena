//! Reading what a PDF text extractor wrote.

/// Decode the bytes a PDF text extractor produced.
///
/// Poppler defaults to UTF-8, but a broken character map or a legacy Latin
/// text layer often emits bytes that are not valid UTF-8 at all — a lone
/// `0xed` is the usual one. Strict UTF-8 first, then Windows-1252, which
/// covers the Western European bytes without failing. Bytes that Windows-1252
/// does not define become the replacement character, exactly as the Python
/// this was ported from does.
pub fn decode(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => bytes.iter().map(|byte| windows_1252(*byte)).collect(),
    }
}

fn windows_1252(byte: u8) -> char {
    if byte < 0x80 {
        return byte as char;
    }
    HIGH[byte as usize - 0x80]
}

/// Windows-1252 for the bytes above ASCII. Generated from the codec itself.
static HIGH: [char; 128] = [
    '\u{20ac}', '\u{fffd}', '\u{201a}', '\u{192}', '\u{201e}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{2c6}', '\u{2030}', '\u{160}', '\u{2039}', '\u{152}', '\u{fffd}', '\u{17d}', '\u{fffd}',
    '\u{fffd}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{2dc}', '\u{2122}', '\u{161}', '\u{203a}', '\u{153}', '\u{fffd}', '\u{17e}', '\u{178}',
    '\u{a0}', '\u{a1}', '\u{a2}', '\u{a3}', '\u{a4}', '\u{a5}', '\u{a6}', '\u{a7}', '\u{a8}',
    '\u{a9}', '\u{aa}', '\u{ab}', '\u{ac}', '\u{ad}', '\u{ae}', '\u{af}', '\u{b0}', '\u{b1}',
    '\u{b2}', '\u{b3}', '\u{b4}', '\u{b5}', '\u{b6}', '\u{b7}', '\u{b8}', '\u{b9}', '\u{ba}',
    '\u{bb}', '\u{bc}', '\u{bd}', '\u{be}', '\u{bf}', '\u{c0}', '\u{c1}', '\u{c2}', '\u{c3}',
    '\u{c4}', '\u{c5}', '\u{c6}', '\u{c7}', '\u{c8}', '\u{c9}', '\u{ca}', '\u{cb}', '\u{cc}',
    '\u{cd}', '\u{ce}', '\u{cf}', '\u{d0}', '\u{d1}', '\u{d2}', '\u{d3}', '\u{d4}', '\u{d5}',
    '\u{d6}', '\u{d7}', '\u{d8}', '\u{d9}', '\u{da}', '\u{db}', '\u{dc}', '\u{dd}', '\u{de}',
    '\u{df}', '\u{e0}', '\u{e1}', '\u{e2}', '\u{e3}', '\u{e4}', '\u{e5}', '\u{e6}', '\u{e7}',
    '\u{e8}', '\u{e9}', '\u{ea}', '\u{eb}', '\u{ec}', '\u{ed}', '\u{ee}', '\u{ef}', '\u{f0}',
    '\u{f1}', '\u{f2}', '\u{f3}', '\u{f4}', '\u{f5}', '\u{f6}', '\u{f7}', '\u{f8}', '\u{f9}',
    '\u{fa}', '\u{fb}', '\u{fc}', '\u{fd}', '\u{fe}', '\u{ff}',
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_utf8_is_read_as_utf8() {
        assert_eq!(
            decode("caf\u{e9} \u{4e2d}".as_bytes()),
            "caf\u{e9} \u{4e2d}"
        );
    }

    #[test]
    fn a_lone_high_byte_falls_back_to_windows_1252() {
        assert_eq!(decode(&[b'a', 0xed, b'b']), "a\u{ed}b");
        assert_eq!(decode(&[0x93, b'x', 0x94]), "\u{201c}x\u{201d}");
    }

    #[test]
    fn an_undefined_byte_becomes_the_replacement_character() {
        assert_eq!(decode(&[0xff, 0x81]), "\u{ff}\u{fffd}");
    }

    #[test]
    fn the_table_covers_every_high_byte() {
        assert_eq!(HIGH.len(), 128);
    }
}
