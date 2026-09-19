//! Splitting a real file, checked by reading the result back.
//!
//! The PDFs are built here rather than kept as fixtures, so that what each
//! test is about — an inherited box, a rotated sheet, a mixed document — is
//! visible in the test instead of hidden in a binary.

use std::path::{Path, PathBuf};

use pdfgutter::pdf::Pdf;

/// One page of a document under construction.
struct Sheet {
    width: f64,
    height: f64,
    rotate: i64,
}

/// Write a PDF whose pages carry their own boxes.
fn write(path: &Path, sheets: &[Sheet]) {
    write_with(path, sheets, false);
}

/// Write a PDF, optionally hoisting the box onto the page tree so that every
/// page has to inherit it.
fn write_with(path: &Path, sheets: &[Sheet], inherit: bool) {
    let first = 4;
    let kids: Vec<String> = (0..sheets.len())
        .map(|i| format!("{} 0 R", first + 2 * i))
        .collect();

    let mut objects: Vec<(usize, Vec<u8>)> = Vec::new();
    objects.push((1, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()));

    let shared = match (inherit, sheets.first()) {
        (true, Some(sheet)) => format!(" /MediaBox [0 0 {} {}]", sheet.width, sheet.height),
        _ => String::new(),
    };
    objects.push((
        2,
        format!(
            "<< /Type /Pages /Count {} /Kids [{}]{shared} >>",
            sheets.len(),
            kids.join(" ")
        )
        .into_bytes(),
    ));
    objects.push((
        3,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
    ));

    for (i, sheet) in sheets.iter().enumerate() {
        let content = format!("BT /F1 12 Tf 40 40 Td (page {}) Tj ET", i + 1);
        let own = match inherit {
            true => String::new(),
            false => format!(" /MediaBox [0 0 {} {}]", sheet.width, sheet.height),
        };
        let rotate = match sheet.rotate {
            0 => String::new(),
            turn => format!(" /Rotate {turn}"),
        };
        objects.push((
            first + 2 * i,
            format!(
                "<< /Type /Page /Parent 2 0 R{own}{rotate} /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
                first + 2 * i + 1
            )
            .into_bytes(),
        ));
        let mut stream = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
        stream.extend_from_slice(content.as_bytes());
        stream.extend_from_slice(b"\nendstream");
        objects.push((first + 2 * i + 1, stream));
    }

    let mut out: Vec<u8> = b"%PDF-1.4\n".to_vec();
    let mut offsets = vec![0usize; objects.len() + 5];
    for (number, body) in &objects {
        offsets[*number] = out.len();
        out.extend_from_slice(format!("{number} 0 obj\n").as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let start = out.len();
    let top = objects.iter().map(|(n, _)| *n).max().unwrap_or(0) + 1;
    out.extend_from_slice(format!("xref\n0 {top}\n0000000000 65535 f \n").as_bytes());
    for offset in offsets.iter().take(top).skip(1) {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size {top} /Root 1 0 R >>\nstartxref\n{start}\n%%EOF\n").as_bytes(),
    );
    std::fs::write(path, out).expect("a writable file");
}

/// A directory this test can write into, named for the test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pdfgutter-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a writable directory");
    dir
}

fn wide() -> Sheet {
    Sheet {
        width: 1008.0,
        height: 720.0,
        rotate: 0,
    }
}

fn tall() -> Sheet {
    Sheet {
        width: 480.0,
        height: 720.0,
        rotate: 0,
    }
}

#[test]
fn a_wide_sheet_becomes_two_pages_and_a_tall_one_is_left_alone() {
    let dir = scratch("mixed");
    let (source, destination) = (dir.join("in.pdf"), dir.join("out.pdf"));
    write(&source, &[tall(), wide(), wide(), tall()]);

    let mut pdf = Pdf::open(&source).expect("a readable PDF");
    assert_eq!(pdf.len(), 4);
    assert_eq!(pdf.split(pdfgutter::MIN_RATIO), 6);
    pdf.save(&destination).expect("a written PDF");

    let split = Pdf::open(&destination).expect("a readable PDF");
    assert_eq!(split.len(), 6);

    // The covers keep their shape; the halves are each half a sheet wide.
    let widths: Vec<f64> = (0..6).map(|i| split.geometry(i).displayed().0).collect();
    assert_eq!(widths, [480.0, 504.0, 504.0, 504.0, 504.0, 480.0]);
}

#[test]
fn the_halves_come_out_in_reading_order() {
    let dir = scratch("order");
    let (source, destination) = (dir.join("in.pdf"), dir.join("out.pdf"));
    write(&source, &[wide()]);

    let mut pdf = Pdf::open(&source).expect("a readable PDF");
    pdf.split(pdfgutter::MIN_RATIO);
    pdf.save(&destination).expect("a written PDF");

    let split = Pdf::open(&destination).expect("a readable PDF");
    assert_eq!(split.geometry(0).media, [0.0, 0.0, 504.0, 720.0]);
    assert_eq!(split.geometry(1).media, [504.0, 0.0, 1008.0, 720.0]);
}

#[test]
fn a_box_the_pages_inherit_is_found_and_kept() {
    // A page tree that puts one MediaBox at the top and none on any page is
    // ordinary, and reading the pages alone would see no box at all.
    let dir = scratch("inherited");
    let (source, destination) = (dir.join("in.pdf"), dir.join("out.pdf"));
    write_with(&source, &[wide(), wide()], true);

    let pdf = Pdf::open(&source).expect("a readable PDF");
    assert_eq!(pdf.geometry(0).media, [0.0, 0.0, 1008.0, 720.0]);

    let mut pdf = pdf;
    assert_eq!(pdf.split(pdfgutter::MIN_RATIO), 4);
    pdf.save(&destination).expect("a written PDF");

    let split = Pdf::open(&destination).expect("a readable PDF");
    assert_eq!(split.geometry(0).media, [0.0, 0.0, 504.0, 720.0]);
    assert_eq!(split.geometry(3).media, [504.0, 0.0, 1008.0, 720.0]);
}

#[test]
fn a_sheet_on_its_side_is_cut_the_way_it_is_read_and_not_the_way_it_is_stored() {
    let dir = scratch("rotated");
    let (source, destination) = (dir.join("in.pdf"), dir.join("out.pdf"));
    // Stored tall, displayed wide: a quarter turn makes this a spread.
    write(
        &source,
        &[Sheet {
            width: 720.0,
            height: 1008.0,
            rotate: 90,
        }],
    );

    let pdf = Pdf::open(&source).expect("a readable PDF");
    assert_eq!(pdf.geometry(0).displayed(), (1008.0, 720.0));

    let mut pdf = pdf;
    assert_eq!(pdf.split(pdfgutter::MIN_RATIO), 2);
    pdf.save(&destination).expect("a written PDF");

    let split = Pdf::open(&destination).expect("a readable PDF");
    // Cut across rather than down, and each half still displays upright.
    assert_eq!(split.geometry(0).media, [0.0, 0.0, 720.0, 504.0]);
    assert_eq!(split.geometry(1).media, [0.0, 504.0, 720.0, 1008.0]);
    assert_eq!(split.geometry(0).displayed(), (504.0, 720.0));
}

#[test]
fn a_document_of_ordinary_pages_comes_through_unchanged() {
    let dir = scratch("untouched");
    let (source, destination) = (dir.join("in.pdf"), dir.join("out.pdf"));
    write(&source, &[tall(), tall(), tall()]);

    let mut pdf = Pdf::open(&source).expect("a readable PDF");
    assert_eq!(pdf.split(pdfgutter::MIN_RATIO), 3);
    pdf.save(&destination).expect("a written PDF");

    let split = Pdf::open(&destination).expect("a readable PDF");
    assert_eq!(split.len(), 3);
    for index in 0..3 {
        assert_eq!(split.geometry(index).media, [0.0, 0.0, 480.0, 720.0]);
    }
}

#[test]
fn splitting_twice_does_not_split_twice() {
    // Halves are portrait, so a second pass has nothing to do. Anything that
    // runs over a directory will eventually run over its own output.
    let dir = scratch("idempotent");
    let (source, once, twice) = (dir.join("in.pdf"), dir.join("1.pdf"), dir.join("2.pdf"));
    write(&source, &[wide(), wide(), wide()]);

    let mut pdf = Pdf::open(&source).expect("a readable PDF");
    pdf.split(pdfgutter::MIN_RATIO);
    pdf.save(&once).expect("a written PDF");

    let mut again = Pdf::open(&once).expect("a readable PDF");
    assert_eq!(again.split(pdfgutter::MIN_RATIO), 6);
    again.save(&twice).expect("a written PDF");

    let split = Pdf::open(&twice).expect("a readable PDF");
    assert_eq!(split.len(), 6);
}

#[test]
fn what_is_not_a_pdf_is_refused_rather_than_guessed_at() {
    let dir = scratch("refused");
    let path = dir.join("not.pdf");
    std::fs::write(&path, b"this is not a PDF at all").expect("a writable file");
    assert!(Pdf::open(&path).is_err());
}
