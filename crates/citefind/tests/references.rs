//! Reading a block of references end to end.

use citefind::{Kind, read};

/// A page of endnotes as conversion leaves it: numbers, anchors, and a
/// discursive note in among the citations.
const NOTES: &str = "\
[1.](part0011.html#ke_00001_00003) John MacGowan, *The Imperial History of China* (London: Curzon, 1897), p. 154.
2. I am grateful to the seminar at Leiden for pressing me on this.
[3](text0007.xhtml#en3). Johanna Bockman, *Markets in the Name of Socialism* (Stanford, CA: Stanford University Press, 2011), 40-42.
4. Ibid., p. 44.
";

#[test]
fn a_page_of_notes_keeps_its_numbering_and_drops_its_asides() {
    let found = read(NOTES);
    assert_eq!(found.len(), 3, "{found:#?}");
    assert_eq!(
        found
            .iter()
            .map(|one| one.marker.as_deref())
            .collect::<Vec<_>>(),
        [Some("1"), Some("3"), Some("4")]
    );
    // Numbered from zero in the order they appeared, whatever the markers say.
    assert_eq!(
        found.iter().map(|one| one.idx).collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert_eq!(found[0].authors, ["John MacGowan"]);
    assert_eq!(found[0].container.as_deref(), Some("Curzon"));
    assert_eq!(found[1].year, Some(2011));
    assert_eq!(found[1].kind, Kind::Book);
    assert!(found[2].continuation);
}

#[test]
fn a_rule_of_dashes_means_the_author_above() {
    let block = "\
Smith, John. *The First Book*. Oxford: Clarendon Press, 1998.

———. *The Second Book*. Oxford: Clarendon Press, 2001.

——— *The Third Book*. Oxford: Clarendon Press, 2004.

Doe, Jane. *Something Else*. Cambridge: University Press, 2005.

———. *And Another*. Cambridge: University Press, 2007.";
    let found = read(block);
    assert_eq!(found.len(), 5, "{found:#?}");
    assert_eq!(
        found
            .iter()
            .map(|one| one.authors.clone())
            .collect::<Vec<_>>(),
        [
            vec!["Smith, John"],
            vec!["Smith, John"],
            vec!["Smith, John"],
            vec!["Doe, Jane"],
            vec!["Doe, Jane"],
        ]
    );
    assert_eq!(found[2].title.as_deref(), Some("The Third Book"));
}

#[test]
fn an_ibid_does_not_inherit_an_author_the_way_a_dash_does() {
    let block = "\
Smith, John. *The First Book*. Oxford: Clarendon Press, 1998.

Ibid., p. 44.";
    let found = read(block);
    assert_eq!(found.len(), 2);
    assert!(found[1].authors.is_empty(), "{:#?}", found[1]);
    assert!(found[1].continuation);
}

#[test]
fn every_record_says_what_it_could_not_settle() {
    let found = read(
        "Marron, Kevin Ritual Abuse, Seal Books, 1988\n\nAnother, Line. Of nothing much, 1990.",
    );
    for one in &found {
        assert_eq!(
            one.unsettled,
            one.missing()
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn the_same_block_reads_the_same_way_every_time() {
    assert_eq!(read(NOTES), read(NOTES));
}

/// A block mixing one entry that reads cleanly, one that gives up nothing
/// but a title, and a continuation.
const MIXED: &str = "\
1. John MacGowan, *The Imperial History of China* (London: Curzon, 1897), p. 154.
2. Marron, Kevin Ritual Abuse, Seal Books, 1988.
3. Ibid., p. 44.
";

#[test]
fn a_prompt_asks_about_the_entries_that_did_not_settle_and_no_others() {
    let found = read(MIXED);
    assert_eq!(found.len(), 3, "{found:#?}");
    let prompt = citefind::prompt::compose(&found).expect("something unsettled");

    // The entry that read cleanly is not in the question, and neither is the
    // continuation — it has nothing of its own to give.
    assert!(!prompt.contains("MacGowan"), "{prompt}");
    assert!(!prompt.contains("Ibid"), "{prompt}");
    assert!(prompt.contains("Ritual Abuse"), "{prompt}");
    assert!(prompt.contains("wanted:"), "{prompt}");
}

#[test]
fn a_prompt_is_only_composed_where_there_is_something_to_ask() {
    let settled = read(
        "1. John MacGowan, *The Imperial History of China* (London: Curzon, 1897), p. 154.\n2. Johanna Bockman, *Markets in the Name of Socialism* (Stanford, CA: Stanford University Press, 2011), 40.",
    );
    assert!(
        settled.iter().all(|one| one.unsettled.is_empty()),
        "{settled:#?}"
    );
    assert_eq!(citefind::prompt::compose(&settled), None);
}

#[test]
fn a_reply_about_an_entry_that_settled_does_not_overrule_the_page() {
    let mut found = read(MIXED);
    let before = found[0].clone();
    let filled = citefind::prompt::merge(
        &mut found,
        r#"{"0": {"title": "Invented", "container": "Invented"}, "2": {"title": "Invented"}}"#,
    )
    .expect("a reply");
    assert_eq!(filled, 0);
    assert_eq!(found[0], before);
    assert_eq!(found[2].title, None);
}

#[test]
fn a_reply_fills_in_only_what_was_asked_about() {
    let mut found = read("Marron, Kevin Ritual Abuse, Seal Books, 1988");
    assert_eq!(found.len(), 1);
    assert!(found[0].unsettled.contains(&"authors".to_string()));

    let filled = citefind::prompt::merge(
        &mut found,
        "Sure!\n```json\n{\"0\": {\"authors\": [\"Marron, Kevin\"], \"year\": 1066}}\n```",
    )
    .expect("a reply");
    assert_eq!(filled, 1);
    assert_eq!(found[0].authors, ["Marron, Kevin"]);
    // The year was read off the page, so the model's does not replace it.
    assert_eq!(found[0].year, Some(1988));
    assert!(!found[0].unsettled.contains(&"authors".to_string()));
}

#[test]
fn a_reply_that_is_not_json_is_refused_rather_than_half_applied() {
    let mut found = read(NOTES);
    let before = found.clone();
    assert!(citefind::prompt::merge(&mut found, "I could not tell.").is_err());
    assert_eq!(found, before);
}

#[test]
fn a_note_that_cites_two_works_gives_two_records() {
    let found = read(
        "12. Theodor W. Adorno, *Negative Dialectics*, trans. E. B. Ashton (London, 1973), 365; \
         Jeffrey C. Alexander, \"On the Social Construction of Moral Universals,\" *Cultural Trauma* 5 (2002), 196.\n\
         13. Smith, John. Thinking, Fast and Slow. New York: Farrar, Straus and Giroux, 2011.",
    );
    assert_eq!(found.len(), 3, "{found:#?}");
    // Both halves of note 12 are note 12.
    assert_eq!(
        found
            .iter()
            .map(|one| one.marker.as_deref())
            .collect::<Vec<_>>(),
        [Some("12"), Some("12"), Some("13")]
    );
    assert_eq!(found[0].title.as_deref(), Some("Negative Dialectics"));
    assert_eq!(
        found[1].title.as_deref(),
        Some("On the Social Construction of Moral Universals")
    );
    // The full stop between an author and a title is not a separator, or
    // every bibliography entry ever written would come apart at it.
    assert_eq!(found[2].title.as_deref(), Some("Thinking, Fast and Slow"));
}

#[test]
fn the_body_of_a_page_is_not_the_first_of_the_notes_at_its_foot() {
    let page = format!(
        "{}\n\n1. John MacGowan, *The Imperial History of China* (London: Curzon, 1897), p. 154.\n\
         2. Johanna Bockman, *Markets in the Name of Socialism* (Stanford, CA: Stanford University Press, 2011), 40.",
        "The obligation ran both ways, and contemporaries said so plainly whenever they were asked. ".repeat(8)
    );
    let found = read(&page);
    assert_eq!(found.len(), 2, "{found:#?}");
    assert!(
        found.iter().all(|one| !one.text.contains("obligation")),
        "{found:#?}"
    );
}

#[test]
fn a_short_note_citing_two_works_gives_two_records() {
    let found = read(
        "12 Unger, Dutch Shipbuilding, 33-4; Unger, 'Dutch herring', 256-9.\n\
         13 Van Gelder, The Two Reformations, 213, 312; Herwaarden, 'Geloof en geloofsuitingen', 403-4.",
    );
    assert_eq!(found.len(), 4, "{found:#?}");
    assert_eq!(
        found
            .iter()
            .map(|one| one.title.as_deref())
            .collect::<Vec<_>>(),
        [
            Some("Dutch Shipbuilding"),
            Some("Dutch herring"),
            Some("The Two Reformations"),
            Some("Geloof en geloofsuitingen"),
        ]
    );
    assert_eq!(
        found
            .iter()
            .map(|one| one.marker.as_deref())
            .collect::<Vec<_>>(),
        [Some("12"), Some("12"), Some("13"), Some("13")]
    );
}

#[test]
fn nothing_citation_shaped_gives_nothing() {
    assert!(read("Bibliography\n\nSee below.\n\n233\n").is_empty());
}
