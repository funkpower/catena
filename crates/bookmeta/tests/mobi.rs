//! MOBI and AZW3 files as a converter writes them, and as they break.

use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn read(name: &str) -> bookmeta::Metadata {
    bookmeta::read(&fixture(name))
}

/// What the two converted fixtures were given, and so must say.
fn says_what_it_was_given(meta: &bookmeta::Metadata) {
    assert_eq!(meta.source, "mobi");
    assert_eq!(meta.title.as_deref(), Some("A Lantern in the Orchard"));
    assert_eq!(meta.authors, ["Ada Quill", "Tomás Reed"]);
    assert_eq!(meta.publisher.as_deref(), Some("Orchard Press"));
    assert_eq!(
        meta.description.as_deref(),
        Some("A short story written for testing.")
    );
    assert_eq!(meta.date.as_deref(), Some("2001-04-05T04:00:00+00:00"));
    assert_eq!(meta.language.as_deref(), Some("en"));
    assert_eq!(meta.subjects, ["Fiction", "Villages"]);
    assert_eq!(meta.identifiers["isbn"], "9780306406157");
    assert_eq!(meta.identifiers.get("asin"), None);
    assert_eq!(meta.identifiers["uuid"].len(), 36);

    let cover = meta.cover.as_ref().expect("a cover");
    assert_eq!(cover.media_type, "image/jpeg");
    let original = std::fs::read(fixture("lantern-cover.jpg")).expect("the cover fixture");
    assert_eq!(cover.bytes, original);
}

#[test]
fn a_mobi_6_file_says_what_it_was_given() {
    let meta = read("lantern.mobi");
    says_what_it_was_given(&meta);
    assert!(meta.notes.is_empty(), "{:?}", meta.notes);
}

#[test]
fn an_azw3_file_says_what_it_was_given() {
    let meta = read("lantern.azw3");
    says_what_it_was_given(&meta);
    assert!(meta.notes.is_empty(), "{:?}", meta.notes);
}

#[test]
fn exth_113_uuid_is_not_an_asin() {
    for name in ["lantern.mobi", "lantern.azw3"] {
        let meta = read(name);
        assert_eq!(meta.identifiers.get("asin"), None, "{name}");
        assert!(meta.identifiers.contains_key("uuid"), "{name}");
    }
}

/// `lantern.mobi` with its cover and thumbnail offsets (EXTH 201 and 202)
/// retyped to an unused record type, so the file names no cover.
fn lantern_naming_no_cover() -> Vec<u8> {
    let mut bytes = std::fs::read(fixture("lantern.mobi")).expect("the fixture");
    let exth = bytes.windows(4).position(|w| w == b"EXTH").expect("EXTH");
    let count = u32::from_be_bytes(bytes[exth + 8..exth + 12].try_into().unwrap());
    let mut cursor = exth + 12;
    let mut retyped = 0;
    for _ in 0..count {
        let kind = u32::from_be_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
        let size = u32::from_be_bytes(bytes[cursor + 4..cursor + 8].try_into().unwrap());
        if kind == 201 || kind == 202 {
            bytes[cursor..cursor + 4].copy_from_slice(&999u32.to_be_bytes());
            retyped += 1;
        }
        cursor += size as usize;
    }
    assert!(retyped > 0, "the fixture names a cover");
    bytes
}

#[test]
fn a_file_naming_no_cover_gets_its_first_image_record() {
    let meta = bookmeta::from_bytes("x.mobi", &lantern_naming_no_cover());
    let cover = meta.cover.expect("the first image record");
    assert_eq!(cover.media_type, "image/jpeg");
    let original = std::fs::read(fixture("lantern-cover.jpg")).expect("the cover fixture");
    assert_eq!(cover.bytes, original);
}

#[test]
fn a_file_with_no_exth_still_has_its_full_name_and_locale() {
    let meta = read("lantern-no-exth.mobi");
    assert_eq!(meta.source, "mobi");
    assert_eq!(meta.title.as_deref(), Some("A Lantern in the Orchard"));
    assert_eq!(meta.language.as_deref(), Some("en"));
    assert!(meta.authors.is_empty());
    assert!(meta.identifiers.is_empty());
    // No EXTH names no cover, so the first image record stands in for one.
    let cover = meta.cover.expect("the first image record");
    assert_eq!(cover.media_type, "image/jpeg");
}

#[test]
fn a_truncated_file_is_an_empty_record_with_a_reason() {
    let meta = read("lantern-truncated.mobi");
    assert_eq!(meta.source, "none");
    assert!(meta.is_empty());
    assert_eq!(meta.notes, ["the first record lies outside the file"]);
}

#[test]
fn a_drm_flagged_file_has_its_metadata_and_says_drm() {
    let meta = read("lantern-drm.mobi");
    says_what_it_was_given(&meta);
    assert_eq!(meta.notes, ["drm"]);
}

#[test]
fn every_truncation_of_the_mobi_fixture_returns() {
    let bytes = std::fs::read(fixture("lantern.mobi")).expect("the fixture");
    let first_record_end = u32::from_be_bytes(bytes[86..90].try_into().unwrap()) as usize;
    for len in 0..=bytes.len() {
        let meta = bookmeta::from_bytes("cut.mobi", &bytes[..len]);
        if len < first_record_end {
            assert!(meta.is_empty(), "cut at {len}: {meta:?}");
        }
    }
}

#[test]
fn every_single_byte_corruption_of_the_header_returns() {
    let bytes = std::fs::read(fixture("lantern.mobi")).expect("the fixture");
    // The Palm header, the record list, and the whole of the first record:
    // every offset the reader follows is in there.
    let first_record_end = u32::from_be_bytes(bytes[86..90].try_into().unwrap()) as usize;
    for at in 0..first_record_end {
        for value in [0x00, 0x7f, 0xff] {
            let mut broken = bytes.clone();
            broken[at] = value;
            let _ = bookmeta::from_bytes("broken.mobi", &broken);
        }
    }
}
