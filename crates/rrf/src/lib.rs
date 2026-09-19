//! Reciprocal rank fusion: several ranked lists in, one ranked list out.
//!
//! The useful half of hybrid search, with none of the database. Vector search
//! and full-text search disagree about everything except what matters, and
//! their scores are not comparable — a cosine distance and a `ts_rank` are
//! different quantities on different scales. Fusion sidesteps that by
//! throwing the scores away and keeping only the ranks: a result that both
//! channels put near the top beats one that either channel loves alone.
//!
//! A channel may be weighted, which is how a metadata match outranks a
//! body-text tie: when somebody searches for a book by its title, the book
//! rarely repeats its own title in its body, so the channel that matched the
//! title is the one telling the truth.

use std::collections::HashMap;

use corpus_core::{Channel, Hit};

/// The constant that flattens the difference between rank 1 and rank 2.
///
/// Sixty, from the paper the method comes from and from every implementation
/// since. It is large enough that the top of one list does not swamp the rest
/// of another, and it is the number every other system's fused scores are
/// comparable against, which is worth more than tuning it.
pub const K: f64 = 60.0;

/// How much a metadata match counts for, against a body-text match at the
/// same rank.
pub const META_WEIGHT: f64 = 1.5;

/// What each channel's rank is worth.
#[derive(Debug, Clone, PartialEq)]
pub struct Weights {
    /// The rank constant.
    pub k: f64,
    /// Per-channel weights.
    pub channels: HashMap<Channel, f64>,
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            k: K,
            channels: HashMap::from([
                (Channel::Vector, 1.0),
                (Channel::Fts, 1.0),
                (Channel::Meta, META_WEIGHT),
            ]),
        }
    }
}

impl Weights {
    /// What one channel's rank is worth. An unnamed channel counts for one.
    pub fn of(&self, channel: Channel) -> f64 {
        self.channels.get(&channel).copied().unwrap_or(1.0)
    }
}

/// Fuse ranked lists into one.
///
/// Every hit is grouped by its `id`, and each contributes `weight / (k +
/// rank)` — the rank the producer gave it, not its position in the stream, so
/// the lists may arrive interleaved. The record that comes back for an id is
/// the one that ranked it best, carrying its payload, with `rank` and `score`
/// replaced by its place and score in the fused list.
///
/// Ties keep the order the ids were first seen in, so the same input always
/// produces the same output.
pub fn fuse(hits: &[Hit], weights: &Weights) -> Vec<Hit> {
    let mut order: Vec<&str> = Vec::new();
    let mut fused: HashMap<&str, Fusing<'_>> = HashMap::new();

    for hit in hits {
        let entry = fused.entry(hit.id.as_str()).or_insert_with(|| {
            order.push(hit.id.as_str());
            Fusing {
                best: hit,
                score: 0.0,
                seen: HashMap::new(),
            }
        });
        // A channel that ranks the same id twice is a producer contradicting
        // itself; the later word is taken as the corrected one.
        if let Some(previous) = entry.seen.insert(hit.channel, hit.rank) {
            entry.score -= weights.of(hit.channel) / (weights.k + previous as f64);
        }
        entry.score += weights.of(hit.channel) / (weights.k + hit.rank as f64);
        if hit.rank < entry.best.rank {
            entry.best = hit;
        }
    }

    let mut ranked: Vec<(&str, f64)> = order
        .iter()
        .map(|id| (*id, fused[id].score))
        .collect::<Vec<_>>();
    // Stable, so equal scores stay in first-seen order.
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));

    ranked
        .into_iter()
        .enumerate()
        .map(|(at, (id, score))| {
            let mut hit = fused[id].best.clone();
            hit.rank = at as u32 + 1;
            hit.score = score;
            hit
        })
        .collect()
}

struct Fusing<'a> {
    best: &'a Hit,
    score: f64,
    seen: HashMap<Channel, u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(id: &str, channel: Channel, rank: u32) -> Hit {
        Hit::new(id, channel, rank, 0.0)
    }

    #[test]
    fn a_result_both_channels_like_beats_one_either_channel_loves() {
        let hits = [
            hit("both", Channel::Vector, 2),
            hit("vector-only", Channel::Vector, 1),
            hit("both", Channel::Fts, 2),
            hit("fts-only", Channel::Fts, 1),
        ];
        let fused = fuse(&hits, &Weights::default());
        assert_eq!(fused[0].id, "both");
        assert_eq!(fused[0].rank, 1);
    }

    #[test]
    fn a_metadata_match_outranks_a_body_tie() {
        let hits = [hit("title", Channel::Meta, 1), hit("body", Channel::Fts, 1)];
        let fused = fuse(&hits, &Weights::default());
        assert_eq!(fused[0].id, "title");
    }

    #[test]
    fn the_scores_are_the_ones_the_method_defines() {
        let hits = [hit("a", Channel::Vector, 1), hit("a", Channel::Fts, 3)];
        let fused = fuse(&hits, &Weights::default());
        assert_eq!(fused[0].score, 1.0 / 61.0 + 1.0 / 63.0);
    }

    #[test]
    fn the_record_that_comes_back_is_the_one_that_ranked_best() {
        let mut best = hit("a", Channel::Vector, 1);
        best.payload.insert("from".into(), "vector".into());
        let mut worse = hit("a", Channel::Fts, 9);
        worse.payload.insert("from".into(), "fts".into());

        let fused = fuse(&[worse, best], &Weights::default());
        assert_eq!(fused[0].payload["from"], "vector");
        assert_eq!(fused[0].channel, Channel::Vector);
    }

    #[test]
    fn ranks_come_out_consecutive_from_one() {
        let hits = [
            hit("a", Channel::Vector, 1),
            hit("b", Channel::Vector, 2),
            hit("c", Channel::Vector, 3),
        ];
        let fused = fuse(&hits, &Weights::default());
        assert_eq!(
            fused.iter().map(|hit| hit.rank).collect::<Vec<_>>(),
            [1, 2, 3]
        );
    }

    #[test]
    fn a_tie_keeps_the_order_the_ids_arrived_in() {
        let hits = [
            hit("second", Channel::Vector, 5),
            hit("first", Channel::Fts, 5),
        ];
        let fused = fuse(&hits, &Weights::default());
        assert_eq!(fused[0].id, "second");
        assert_eq!(fused[1].id, "first");
        assert_eq!(fused[0].score, fused[1].score);
    }

    #[test]
    fn a_channel_that_ranks_the_same_id_twice_counts_once() {
        let twice = fuse(
            &[hit("a", Channel::Vector, 9), hit("a", Channel::Vector, 1)],
            &Weights::default(),
        );
        let once = fuse(&[hit("a", Channel::Vector, 1)], &Weights::default());
        assert_eq!(twice[0].score, once[0].score);
    }

    #[test]
    fn weights_can_be_changed() {
        let weights = Weights {
            k: 1.0,
            channels: HashMap::from([(Channel::Fts, 10.0)]),
        };
        let fused = fuse(
            &[hit("meta", Channel::Meta, 1), hit("fts", Channel::Fts, 1)],
            &weights,
        );
        assert_eq!(fused[0].id, "fts");
        assert_eq!(fused[0].score, 10.0 / 2.0);
    }

    #[test]
    fn nothing_in_nothing_out() {
        assert!(fuse(&[], &Weights::default()).is_empty());
    }
}
