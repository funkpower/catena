//! What real front matter looks like, and what has to come off it.
//!
//! Every case here is a shape taken from a book that was catalogued wrongly
//! without it.

use colophon::{PAGES, Source, read};

/// A paperback series prints two hundred other titles before its own, in the
/// same shape a title page uses. Catalogued off that page, this book is by
/// whichever author the reader's eye landed on.
#[test]
fn a_series_advertisement_is_not_the_title_page() {
    let front = "\
<!-- Page 2 -->

The First World War: A Very Short Introduction

<!-- Page 3 -->

Very Short Introductions available now:
ANARCHISM Colin Ward
ANCIENT EGYPT Ian Shaw
ARISTOTLE Jonathan Barnes
THE ELEMENTS Philip Ball
CONSCIOUSNESS Susan Blackmore
COSMOLOGY Peter Coles

<!-- Page 4 -->

CLASSICS Mary Beard
DADA AND SURREALISM David Hopkins
DARWIN Jonathan Howard
THE DEAD SEA SCROLLS Timothy Lim
RUSSIAN LITERATURE Catriona Kelly

<!-- Page 6 -->

Michael Howard

THE FIRST
WORLD WAR

A Very Short Introduction

<!-- Page 7 -->

Great Clarendon Street, Oxford ox2 6dp
Oxford University Press is a department of the University of Oxford.
First published 2002
All rights reserved
ISBN 978-0-19-280353-5
";
    let found = read(front, PAGES);
    // The title page breaks the title across two lines and sets it in
    // capitals; the half title prints the whole of it, cased as the
    // publisher meant. Both are this book, and the unbroken one is kept.
    assert_eq!(found.title.as_deref(), Some("The First World War"));
    assert_eq!(found.subtitle.as_deref(), Some("A Very Short Introduction"));
    assert_eq!(found.authors, vec!["Michael Howard"]);
    assert_eq!(found.isbns, vec!["9780192803535"]);
    assert_eq!(found.year, Some(2002));
    assert_eq!(found.read_from.get("title"), Some(&Source::TitlePage));
}

/// The cataloguing block is a catalogue record somebody else already made.
/// The solidus between the title and who wrote it is read as a capital I by
/// every scanner ever built.
#[test]
fn a_cataloguing_block_outranks_the_page_it_is_printed_on() {
    let front = "\
<!-- Page 4 -->

Consciousness Explained

Daniel C. Dennett

<!-- Page 5 -->

Copyright © 1991 by Daniel C. Dennett
All rights reserved
First published 1991
Library of Congress Cataloging-in-Publication Data
Dennett, Daniel C.
  Consciousness explained I Daniel C. Dennett. — 1st ed.
     p.   cm.
  Includes bibliographical references and index.
  ISBN 0-316-18065-3
  1. Consciousness. 2. Philosophy of mind. I. Title.
  B105.C477D46 1991
  126—dc20
";
    let found = read(front, PAGES);
    assert_eq!(found.title.as_deref(), Some("Consciousness explained"));
    assert_eq!(found.authors, vec!["Daniel C. Dennett"]);
    assert_eq!(found.read_from.get("title"), Some(&Source::Cip));
    assert_eq!(found.subjects, vec!["Consciousness", "Philosophy of mind"]);
    assert_eq!(found.isbns, vec!["0316180653"]);
}

/// Since 2013 the block comes in labelled fields, and the subject headings
/// with it — the one place a book states what it is about.
#[test]
fn the_labelled_block_gives_its_subjects() {
    let front = "\
<!-- Page 4 -->

Library of Congress Cataloging-in-Publication Data
Names: Howard, Michael, 1922- author.
Title: The First World War : a very short introduction / Michael Howard.
Description: Oxford : Oxford University Press, 2007.
Identifiers: ISBN 9780192803535
Subjects: LCSH: World War, 1914-1918. | Europe--History--1871-1918.
All rights reserved
";
    let found = read(front, PAGES);
    assert_eq!(found.title.as_deref(), Some("The First World War"));
    assert_eq!(found.subtitle.as_deref(), Some("a very short introduction"));
    assert_eq!(found.authors, vec!["Michael Howard"]);
    assert_eq!(found.publisher.as_deref(), Some("Oxford University Press"));
    assert_eq!(found.place.as_deref(), Some("Oxford"));
    assert_eq!(found.year, Some(2007));
    assert_eq!(found.subjects.len(), 2);
}

/// A translation's block prints the original title in brackets above the
/// real one. It is a heading, not the book in your hand.
#[test]
fn a_uniform_title_is_not_the_title() {
    let front = "\
<!-- Page 4 -->

All rights reserved
Library of Congress Cataloging-in-Publication Data
Ricoeur, Paul.
  [Sur la traduction. English]
  On translation / Paul Ricoeur ; translated by Eileen Brennan.
  ISBN 0-415-35777-X
";
    let found = read(front, PAGES);
    assert_eq!(found.title.as_deref(), Some("On translation"));
    assert_eq!(found.authors, vec!["Paul Ricoeur"]);
}

/// A title broken across two lines by the width of the page, with the second
/// half looking exactly like a person's name.
#[test]
fn a_wrapped_title_is_one_title() {
    let front = "\
<!-- Page 4 -->

Esotericism in
Western Culture

Counter-Normativity and
Rejected Knowledge

Wouter J. Hanegraaff

<!-- Page 5 -->

BLOOMSBURY ACADEMIC
First published 2012
All rights reserved
";
    let found = read(front, PAGES);
    assert_eq!(
        found.title.as_deref(),
        Some("Esotericism in Western Culture")
    );
    assert_eq!(
        found.subtitle.as_deref(),
        Some("Counter-Normativity and Rejected Knowledge")
    );
    assert_eq!(found.authors, vec!["Wouter J. Hanegraaff"]);
}

/// Two capitalised words are a name or they are half a title, and English
/// morphology is the only thing on the page that says which.
#[test]
fn an_abstract_noun_is_not_a_person() {
    let front = "\
<!-- Page 1 -->

Political Philosophy
From Plato to Mao
Martin Cohen
Pluto Press
LONDON

<!-- Page 2 -->

First published 2001 by Pluto Press
All rights reserved
";
    let found = read(front, PAGES);
    assert_eq!(found.title.as_deref(), Some("Political Philosophy"));
    assert_eq!(found.subtitle.as_deref(), Some("From Plato to Mao"));
    assert_eq!(found.authors, vec!["Martin Cohen"]);
    assert_eq!(found.publisher.as_deref(), Some("Pluto Press"));
}

/// A scan that starts at the preface prints no title page, and the honest
/// record of that is an empty one.
#[test]
fn a_book_that_says_nothing_about_itself_is_reported_as_saying_nothing() {
    let front = "\
<!-- Page 2 -->

Preface

Some mental states have qualitative phenomenal characters, and since the
dawn of Behaviorism some philosophers have doubted the ability of any
physicalist account of the mental to accommodate this fact.
";
    let found = read(front, PAGES);
    assert!(found.is_empty());
    assert!(found.title.is_none());
    for field in ["title", "authors", "isbns", "publisher", "year", "subjects"] {
        assert!(
            found.unsettled.iter().any(|f| f == field),
            "{field} should be unsettled"
        );
    }
}

/// An ISBN whose check digit disagrees is a mistyped one, and reporting it
/// would hand the next stage a key that opens nothing.
#[test]
fn an_isbn_that_fails_its_check_digit_is_not_reported() {
    let front = "\
<!-- Page 3 -->

All rights reserved
First published 1999
ISBN 978-0-19-280353-9
";
    let found = read(front, PAGES);
    assert!(found.isbns.is_empty());
}

/// What a model answers is filled in only where the pages said nothing, and
/// is marked as having come from a model.
#[test]
fn a_reply_fills_only_what_the_pages_left() {
    let front = "\
<!-- Page 4 -->

Esotericism in
Western Culture

Wouter J. Hanegraaff

<!-- Page 5 -->

All rights reserved
First published 2012
";
    let mut found = read(front, PAGES);
    assert!(found.subjects.is_empty());
    let filled = colophon::prompt::merge(
        &mut found,
        "{\"title\": \"Something Else\", \"subjects\": [\"Occultism\", \"Europe\"]}",
    )
    .expect("a readable reply");
    assert_eq!(filled, 1);
    assert_eq!(
        found.title.as_deref(),
        Some("Esotericism in Western Culture")
    );
    assert_eq!(found.subjects, vec!["Occultism", "Europe"]);
    assert_eq!(found.read_from.get("subjects"), Some(&Source::Model));
    assert!(!found.unsettled.iter().any(|f| f == "subjects"));
}

/// The question a model is asked never includes the pages that would
/// mislead it.
#[test]
fn the_question_leaves_out_the_advertising() {
    let front = "\
<!-- Page 3 -->

Very Short Introductions available now:
ANARCHISM Colin Ward
THE ELEMENTS Philip Ball
CONSCIOUSNESS Susan Blackmore
ARISTOTLE Jonathan Barnes
COSMOLOGY Peter Coles

<!-- Page 6 -->

Some pages of a preface that names no title at all, running on for long
enough that the classifier takes it for text rather than a title page.
";
    let found = read(front, PAGES);
    let question = colophon::prompt::compose(&found, front, PAGES).expect("a question");
    assert!(!question.contains("THE ELEMENTS"));
    assert!(!question.contains("Philip Ball"));
    assert!(question.contains("wanted: title"));
}
