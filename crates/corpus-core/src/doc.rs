//! What a file is.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A source file, as identified by inspection rather than by its name.
///
/// Fields a given tool cannot know are `None`: a format sniffer fills `kind`
/// and leaves `text_layer` empty, a triage tool does the reverse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Doc {
    /// Record version.
    #[serde(default = "crate::v")]
    pub v: u32,
    /// Path the file was read from, as given.
    pub path: String,
    /// What the bytes say the file is.
    pub kind: Kind,
    /// Extension the name claimed, lowercased, without the dot.
    pub declared_ext: Option<String>,
    /// Size on disk. Signed, because this is a database column as often as
    /// it is a number, and every database that will hold it counts in `i64`.
    pub bytes: i64,
    /// Page count, for paginated formats.
    pub pages: Option<u32>,
    /// ISO 639-2/B language code, when one was detected.
    pub language: Option<String>,
    /// Verdict on the embedded text layer.
    pub text_layer: Option<TextLayer>,
    /// Whether the file has to be OCRed to be searchable.
    pub needs_ocr: Option<bool>,
    /// Human-readable observations, in the order they were made.
    #[serde(default)]
    pub notes: Vec<String>,
}

impl Doc {
    /// A doc with only what inspection of the bytes always yields.
    pub fn new(path: impl Into<String>, kind: Kind, bytes: i64) -> Self {
        Self {
            v: crate::V,
            path: path.into(),
            kind,
            declared_ext: None,
            bytes,
            pages: None,
            language: None,
            text_layer: None,
            needs_ocr: None,
            notes: Vec::new(),
        }
    }

    /// Append an observation.
    pub fn note(&mut self, note: impl Into<String>) {
        self.notes.push(note.into());
    }
}

impl crate::Record for Doc {
    fn version(&self) -> u32 {
        self.v
    }
}

/// The format a file actually is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// Portable Document Format.
    Pdf,
    /// EPUB, any version.
    Epub,
    /// DjVu.
    Djvu,
    /// Amazon KF8.
    Azw3,
    /// Amazon MOBI.
    Mobi,
    /// Microsoft Reader.
    Lit,
    /// FictionBook, the XML ebook format.
    Fb2,
    /// Rich Text Format.
    Rtf,
    /// Plain text.
    Txt,
    /// HTML or XHTML.
    Html,
    /// An OLE2 compound document — a `.doc`, whatever the name says.
    Ole2,
    /// A zip archive whose only real content is a single PDF.
    ZipWrappedPdf,
    /// A zip archive whose only real content is a single FictionBook.
    ZipWrappedFb2,
    /// A comic book archive: a zip of page images.
    Cbz,
    /// Recognised as none of the above.
    Unknown,
}

/// The state of a PDF's embedded text layer, and therefore whether OCR is
/// worth its cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TextLayer {
    /// Extractable and readable as it stands.
    Healthy,
    /// Present but mojibake — extraction yields characters, not words.
    Garbled,
    /// Present but too thin to be the whole page; a scan with a caption layer.
    Sparse,
    /// Present and permuted by a non-standard font encoding, recoverable
    /// without OCR once the permutation is undone.
    ReflectedFont,
    /// No text at all; a scan.
    Absent,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_spellings_are_the_wire_spellings() {
        let cases = [
            (Kind::Pdf, "\"pdf\""),
            (Kind::Ole2, "\"ole2\""),
            (Kind::Azw3, "\"azw3\""),
            (Kind::ZipWrappedPdf, "\"zip-wrapped-pdf\""),
            (Kind::ZipWrappedFb2, "\"zip-wrapped-fb2\""),
            (Kind::Fb2, "\"fb2\""),
            (Kind::Lit, "\"lit\""),
            (Kind::Rtf, "\"rtf\""),
            (Kind::Cbz, "\"cbz\""),
            (Kind::Unknown, "\"unknown\""),
        ];
        for (kind, wire) in cases {
            assert_eq!(serde_json::to_string(&kind).unwrap(), wire);
            assert_eq!(serde_json::from_str::<Kind>(wire).unwrap(), kind);
        }
    }

    #[test]
    fn text_layer_spellings_are_the_wire_spellings() {
        assert_eq!(
            serde_json::to_string(&TextLayer::ReflectedFont).unwrap(),
            "\"reflected-font\""
        );
    }

    #[test]
    fn every_field_is_emitted_even_when_empty() {
        let doc = Doc::new("/tmp/a.pdf", Kind::Pdf, 12);
        assert_eq!(
            serde_json::to_string(&doc).unwrap(),
            r#"{"v":1,"path":"/tmp/a.pdf","kind":"pdf","declared_ext":null,"bytes":12,"pages":null,"language":null,"text_layer":null,"needs_ocr":null,"notes":[]}"#
        );
    }

    #[test]
    fn a_record_written_without_a_version_reads_as_version_one() {
        let doc: Doc = serde_json::from_str(r#"{"path":"a","kind":"txt","bytes":1}"#).unwrap();
        assert_eq!(doc.v, 1);
    }

    #[test]
    fn an_omitted_empty_list_reads_as_an_empty_list() {
        let doc: Doc = serde_json::from_str(r#"{"path":"a","kind":"txt","bytes":1}"#).unwrap();
        assert!(doc.notes.is_empty());
    }
}
