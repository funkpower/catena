//! Reading a rendered page as a column-by-column measure of how much ink is
//! on it.
//!
//! The page comes in as a greymap — the format `pdftoppm` writes and the
//! simplest image format there is: a header, then one byte per pixel. Only
//! the column sums are wanted, so nothing is kept but the profile.

use std::fmt;

/// What can be wrong with a greymap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// It does not begin `P5`.
    NotAGreymap,
    /// The header ended before the three numbers did.
    Truncated,
    /// A number in the header was not one.
    BadHeader,
    /// Sixteen-bit samples, which nothing here produces.
    SampleTooWide(u32),
    /// The header promises more pixels than the file holds.
    ShortImage {
        /// How many bytes the header calls for.
        expected: usize,
        /// How many are there.
        found: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotAGreymap => write!(f, "not a binary greymap"),
            Error::Truncated => write!(f, "the greymap header is cut short"),
            Error::BadHeader => write!(f, "the greymap header is not three numbers"),
            Error::SampleTooWide(max) => write!(f, "{max} levels of grey, which is more than 255"),
            Error::ShortImage { expected, found } => {
                write!(f, "the greymap promises {expected} bytes and holds {found}")
            }
        }
    }
}

impl std::error::Error for Error {}

/// The fraction of each column that is ink.
///
/// One number per pixel column, left to right, each between zero and one.
pub fn profile(greymap: &[u8], threshold: u8) -> Result<Vec<f64>, Error> {
    let (width, height, levels, pixels) = header(greymap)?;
    if levels > 255 {
        return Err(Error::SampleTooWide(levels));
    }
    let wanted = width.checked_mul(height).ok_or(Error::BadHeader)?;
    if pixels.len() < wanted {
        return Err(Error::ShortImage {
            expected: wanted,
            found: pixels.len(),
        });
    }
    if width == 0 || height == 0 {
        return Ok(Vec::new());
    }

    // A greymap need not go all the way to white, and the threshold is on the
    // usual nought-to-255 scale, so compare in that scale rather than the
    // file's.
    let dark = |value: u8| (u32::from(value) * 255 / levels.max(1)) < u32::from(threshold);

    let mut counts = vec![0usize; width];
    for row in pixels[..wanted].chunks_exact(width) {
        for (column, value) in row.iter().enumerate() {
            if dark(*value) {
                counts[column] += 1;
            }
        }
    }
    Ok(counts
        .into_iter()
        .map(|count| count as f64 / height as f64)
        .collect())
}

/// Width, height, levels of grey, and where the pixels start.
fn header(greymap: &[u8]) -> Result<(usize, usize, u32, &[u8]), Error> {
    let rest = greymap.strip_prefix(b"P5").ok_or(Error::NotAGreymap)?;

    let mut at = 0;
    let mut numbers = [0u32; 3];
    for number in &mut numbers {
        at += skip(&rest[at..]);
        let digits = rest[at..]
            .iter()
            .position(|byte| !byte.is_ascii_digit())
            .ok_or(Error::Truncated)?;
        if digits == 0 {
            return Err(Error::BadHeader);
        }
        *number = std::str::from_utf8(&rest[at..at + digits])
            .map_err(|_| Error::BadHeader)?
            .parse()
            .map_err(|_| Error::BadHeader)?;
        at += digits;
    }

    // Exactly one byte of whitespace separates the header from the pixels,
    // and a pixel may itself be a whitespace byte.
    let pixels = rest.get(at + 1..).ok_or(Error::Truncated)?;
    Ok((numbers[0] as usize, numbers[1] as usize, numbers[2], pixels))
}

/// Whitespace and comments, which a greymap header may carry anywhere.
fn skip(bytes: &[u8]) -> usize {
    let mut at = 0;
    loop {
        let was = at;
        while bytes.get(at).is_some_and(u8::is_ascii_whitespace) {
            at += 1;
        }
        if bytes.get(at) == Some(&b'#') {
            at += bytes[at..]
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(bytes.len() - at, |end| end + 1);
        }
        if at == was {
            return at;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Four columns, two rows: black, white, black, white.
    const STRIPES: &[u8] = b"P5\n4 2\n255\n\x00\xff\x00\xff\x00\xff\x00\xff";

    #[test]
    fn a_column_of_black_is_all_ink_and_a_column_of_white_is_none() {
        assert_eq!(
            profile(STRIPES, 160).expect("a greymap"),
            [1.0, 0.0, 1.0, 0.0]
        );
    }

    #[test]
    fn grey_falls_on_the_side_of_the_threshold_it_belongs_to() {
        let greys = b"P5\n3 1\n255\n\x9e\x9f\xa0"; // 158, 159, 160
        assert_eq!(profile(greys, 160).expect("a greymap"), [1.0, 1.0, 0.0]);
    }

    #[test]
    fn comments_and_extra_whitespace_in_the_header_are_skipped() {
        let commented = b"P5\n# written by something\n4  2\n255\n\x00\xff\x00\xff\x00\xff\x00\xff";
        assert_eq!(
            profile(commented, 160).expect("a greymap"),
            [1.0, 0.0, 1.0, 0.0]
        );
    }

    #[test]
    fn a_pixel_that_is_a_whitespace_byte_is_still_a_pixel() {
        // 0x20 is a space and also a very dark grey. Eating it as header
        // whitespace would shift every row by one and smear the profile.
        let spaces = b"P5\n2 1\n255\n\x20\xff";
        assert_eq!(profile(spaces, 160).expect("a greymap"), [1.0, 0.0]);
    }

    #[test]
    fn fewer_levels_of_grey_are_scaled_to_the_usual_range() {
        // Maximum 1, so 0 is black and 1 is white however it is written.
        let bilevel = b"P5\n2 1\n1\n\x00\x01";
        assert_eq!(profile(bilevel, 160).expect("a greymap"), [1.0, 0.0]);
    }

    #[test]
    fn what_is_not_a_greymap_says_so() {
        assert_eq!(
            profile(b"P6\n1 1\n255\n\x00\x00\x00", 160),
            Err(Error::NotAGreymap)
        );
        assert_eq!(profile(b"", 160), Err(Error::NotAGreymap));
        assert_eq!(profile(b"P5\n4 2\n", 160), Err(Error::Truncated));
        assert_eq!(profile(b"P5\nwide 2\n255\n", 160), Err(Error::BadHeader));
        assert_eq!(
            profile(b"P5\n1 1\n65535\n\x00\x00", 160),
            Err(Error::SampleTooWide(65535))
        );
    }

    #[test]
    fn a_greymap_cut_short_is_not_padded_with_white() {
        assert_eq!(
            profile(b"P5\n4 2\n255\n\x00\xff\x00", 160),
            Err(Error::ShortImage {
                expected: 8,
                found: 3
            })
        );
    }

    #[test]
    fn a_greymap_with_no_pixels_has_no_profile() {
        assert_eq!(
            profile(b"P5\n0 0\n255\n", 160).expect("a greymap"),
            Vec::<f64>::new()
        );
    }
}
