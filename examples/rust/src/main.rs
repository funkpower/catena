//! A converted book, read the whole way through in one process.
//!
//! The commands are filters and a subprocess is usually the right answer, but
//! when the caller already holds the text there is nothing to serialise: the
//! same libraries the commands are built from take a `&str` and hand back the
//! records. This walks a file from "what even is it" to passages worth
//! embedding and the references at the back, without a pipe anywhere.
//!
//!     cargo run -- book.md
//!     cargo run          # reads the sample below
//!
//! For a real book, convert one first — Gibbon's first volume is public
//! domain and has a few hundred pages of footnotes:
//!
//!     curl -sL -o gibbon.epub https://www.gutenberg.org/cache/epub/731/pg731.epub
//!     pandoc -f epub -t commonmark --wrap=none gibbon.epub -o gibbon.md
//!     cargo run -- gibbon.md

use std::path::Path;

/// A page of a converted book: a heading, two passages and a short
/// bibliography, which is enough to show every stage doing something.
const SAMPLE: &str = "\
# III. The Second Estate

The obligation ran both ways, and contemporaries said so plainly whenever
they were asked.[^4] A lord who could not protect was a lord who could not
command, and the rolls record the consequences.

What the rolls do not say, and what has to be inferred, is how often the
obligation was actually tested. <!-- a converter's leftover -->

# Bibliography

Bloch, Marc. *Feudal Society*. Translated by L. A. Manyon. London: Routledge
and Kegan Paul, 1961.

Duby, Georges. *The Three Orders: Feudal Society Imagined*. Chicago:
University of Chicago Press, 1980.

Reynolds, Susan. *Fiefs and Vassals: The Medieval Evidence Reinterpreted*.
Oxford: Clarendon Press, 1994.
";

fn main() {
    let (name, raw) = match std::env::args().nth(1) {
        Some(path) => {
            let bytes = std::fs::read(&path).unwrap_or_else(|why| {
                eprintln!("catena-example: {path}: {why}");
                std::process::exit(74);
            });
            (path, bytes)
        }
        None => ("sample.md".to_string(), SAMPLE.as_bytes().to_vec()),
    };

    // What the file actually is, whatever it is called. Worth asking first:
    // a `.pdf` that is really a DjVu fails four stages later, in a way that
    // looks like a bug in the converter.
    let doc = booksniff::sniff(&raw.as_slice(), &name);
    let stem = Path::new(&name).file_name().unwrap_or_default();
    println!("{:?} is a {:?}, {} bytes", stem, doc.kind, doc.bytes);
    for note in &doc.notes {
        println!("  note: {note}");
    }

    // Converted markdown carries the converter's leftovers. Scrubbing before
    // chunking means the passages are the book's words and not the tool's.
    let markdown = String::from_utf8_lossy(&raw);
    let clean = mdscrub::scrub(&markdown);

    // The defaults are sized for a book: 512 tokens a chunk, which would put
    // this whole sample in one. A page needs a page's budget.
    let budgets = match std::env::args().nth(1) {
        Some(_) => chunkmd::Budgets::default(),
        None => chunkmd::Budgets {
            target: 120,
            overlap: 0,
            min_tokens: 10,
            ..chunkmd::Budgets::default()
        },
    };
    let chunks = chunkmd::chunk(&clean, &budgets);
    println!("\n{} passage(s), the first few:", chunks.len());
    for chunk in chunks.iter().take(6) {
        let chapter = chunk.chapter.as_deref().unwrap_or("—");
        let opening: String = chunk.text.chars().take(64).collect();
        println!(
            "  [{}] {:?} {} tokens, under {chapter:?}\n      {}…",
            chunk.idx,
            chunk.role,
            chunk.tokens,
            opening.replace('\n', " ")
        );
    }

    // The references. A passage whose role is `bibliography` is one place
    // they live; a page of notes at the foot of a chapter is the other, and
    // it has no heading over it to find it by — which is why `chunkmd`
    // flags those, so this does not have to guess.
    let carrying: Vec<&corpus_core::Chunk> = chunks
        .iter()
        .filter(|chunk| {
            chunk.role == corpus_core::Role::Bibliography
                || chunk
                    .flags
                    .iter()
                    .any(|flag| flag == corpus_core::chunk::flags::FOOTNOTE_CITATIONS)
        })
        .collect();

    let references: Vec<citefind::Reference> = carrying
        .iter()
        .flat_map(|chunk| citefind::read(&chunk.text))
        .filter(|found| !found.continuation)
        .collect();

    println!(
        "\n{} passage(s) carry citation material, giving {} reference(s):",
        carrying.len(),
        references.len()
    );
    // The ones that named something, which is not all of them: a book whose
    // notes cite in the classical short form — `Tacit. Annal. xiii. 5.` —
    // gives mostly passages rather than works, and the record says as much
    // rather than inventing a title to fill the field.
    let with_work = references
        .iter()
        .filter(|found| found.title.is_some())
        .count();
    let named: Vec<&citefind::Reference> = references
        .iter()
        .filter(|found| !found.authors.is_empty())
        .collect();
    println!(
        "{with_work} name a work and {} name an author; those:",
        named.len()
    );

    for found in named.iter().take(6) {
        let who = match found.authors.is_empty() {
            true => "—".to_string(),
            false => found.authors.join("; "),
        };
        println!(
            "  {who}\n    {} ({}) — {:?}",
            found.title.as_deref().unwrap_or("—"),
            found
                .year
                .map(|year| year.to_string())
                .unwrap_or_else(|| "no year".into()),
            found.kind
        );
        if !found.unsettled.is_empty() {
            println!("    could not read: {}", found.unsettled.join(", "));
        }
    }

    // Worth saying plainly, because the numbers above look poor on a book
    // like Gibbon and the reason is not the parser having failed quietly.
    // His notes cite in the classical short form — `Tacit. Annal. xiii. 5.`
    // — which names a passage rather than a work, so there is no title and
    // no year to read and the record says which fields it could not fill
    // rather than inventing them. `citation_prompt` and `merge_citation_reply`
    // are there for a caller that wants a model to finish the job.
    if references.len() > 20 && named.len() * 10 < references.len() {
        println!("\n    most of these are classical short cites — a passage");
        println!("    rather than a work — there is nothing more to read in them.");
    }
}
