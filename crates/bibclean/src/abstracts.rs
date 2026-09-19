//! An abstract that arrived inside out.
//!
//! Some catalogues publish an abstract as an *inverted index* — a map of each
//! word to the positions it appears at — rather than as text. The reason is
//! copyright: an index is a set of facts about a document rather than a copy
//! of it, and may be redistributed where the paragraph may not.
//!
//! Turning one back into prose is sorting by position and joining. What comes
//! out is the original with its punctuation and spacing intact, because those
//! travelled attached to the words.

/// An abstract as its catalogue publishes it: word to the positions it holds.
///
/// A list of pairs rather than a map, so that the order the catalogue listed
/// the words in survives. It matters exactly once — when two words claim the
/// same position — and a map would settle that tie by hash order, which is a
/// different answer on every run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Inverted(pub Vec<(String, Vec<i64>)>);

impl Inverted {
    /// The pairs, in the order they were written.
    pub fn pairs(&self) -> &[(String, Vec<i64>)] {
        &self.0
    }
}

impl FromIterator<(String, Vec<i64>)> for Inverted {
    fn from_iter<T: IntoIterator<Item = (String, Vec<i64>)>>(pairs: T) -> Self {
        Inverted(pairs.into_iter().collect())
    }
}

impl<'de> serde::Deserialize<'de> for Inverted {
    /// Read the object entry by entry rather than through a map, which is
    /// what keeps the document's own order.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct InOrder;

        impl<'de> serde::de::Visitor<'de> for InOrder {
            type Value = Inverted;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("an object of words to the positions they hold")
            }

            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<Inverted, M::Error> {
                let mut pairs = Vec::with_capacity(map.size_hint().unwrap_or(0));
                while let Some((word, at)) = map.next_entry::<String, Vec<i64>>()? {
                    pairs.push((word, at));
                }
                Ok(Inverted(pairs))
            }
        }

        deserializer.deserialize_map(InOrder)
    }
}

/// Put an inverted abstract back in order.
///
/// `None` for an index with nothing usable in it, which is how a record with
/// no abstract at all is usually spelled.
///
/// A word at two positions appears at both. Positions with no word, and gaps
/// in the numbering, are simply not represented — the order is what is known,
/// not the exact offsets.
pub fn from_inverted(index: &Inverted) -> Option<String> {
    let mut positions: Vec<(i64, &str)> = index
        .pairs()
        .iter()
        .flat_map(|(word, at)| at.iter().map(move |at| (*at, word.as_str())))
        .collect();
    if positions.is_empty() {
        return None;
    }

    // By position only, and stably, so two words claiming one position come
    // out in the order the index listed them rather than an arbitrary one.
    positions.sort_by_key(|(at, _)| *at);
    Some(
        positions
            .into_iter()
            .map(|(_, word)| word)
            .collect::<Vec<_>>()
            .join(" "),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inverted(pairs: &[(&str, &[i64])]) -> Inverted {
        pairs
            .iter()
            .map(|(word, at)| (word.to_string(), at.to_vec()))
            .collect()
    }

    #[test]
    fn two_words_at_one_position_keep_the_order_they_were_listed_in() {
        // The only place the catalogue's own ordering matters, and the only
        // reason this is a list of pairs rather than a map.
        let index = inverted(&[("alpha", &[1]), ("beta", &[1])]);
        assert_eq!(from_inverted(&index).as_deref(), Some("alpha beta"));

        let other_way = inverted(&[("beta", &[1]), ("alpha", &[1])]);
        assert_eq!(from_inverted(&other_way).as_deref(), Some("beta alpha"));
    }

    #[test]
    fn words_come_back_in_the_order_their_positions_give() {
        let index = inverted(&[("obligation", &[1]), ("The", &[0]), ("ran", &[2])]);
        assert_eq!(from_inverted(&index).as_deref(), Some("The obligation ran"));
    }

    #[test]
    fn a_word_at_two_positions_appears_at_both() {
        let index = inverted(&[("the", &[0, 2]), ("cat", &[1]), ("mat", &[3])]);
        assert_eq!(from_inverted(&index).as_deref(), Some("the cat the mat"));
    }

    #[test]
    fn punctuation_rides_along_with_the_word_it_was_attached_to() {
        let index = inverted(&[("ways,", &[1]), ("Both", &[0]), ("plainly.", &[2])]);
        assert_eq!(
            from_inverted(&index).as_deref(),
            Some("Both ways, plainly.")
        );
    }

    #[test]
    fn an_index_with_nothing_in_it_is_no_abstract() {
        assert_eq!(from_inverted(&Inverted::default()), None);
        assert_eq!(from_inverted(&inverted(&[("word", &[])])), None);
    }

    #[test]
    fn gaps_and_odd_positions_are_an_order_rather_than_an_offset() {
        // Catalogues skip numbers and occasionally start below zero; what is
        // known is which word comes first, not where it sat.
        let index = inverted(&[("second", &[40]), ("first", &[-3]), ("third", &[900])]);
        assert_eq!(from_inverted(&index).as_deref(), Some("first second third"));
    }

    #[test]
    fn a_one_word_abstract_is_still_an_abstract() {
        assert_eq!(
            from_inverted(&inverted(&[("Abstract", &[0])])).as_deref(),
            Some("Abstract")
        );
    }
}
