//! The budget sets the chunker is pinned at.
//!
//! Two of them: the defaults, and a deliberately small set that forces the
//! flush, overlap and hard-maximum paths to run on a fixture small enough to
//! read. Most of the interesting behaviour only appears at a boundary, and
//! at the default budgets a fixture would have to be pages long to have one.

use chunkmd::Budgets;

/// Named budget sets, in the order goldens are generated.
pub const SETS: &[(&str, Budgets)] = &[
    (
        "default",
        Budgets {
            target: 512,
            overlap: 64,
            min_tokens: 50,
            hard_max: 768,
        },
    ),
    (
        "small",
        Budgets {
            target: 60,
            overlap: 12,
            min_tokens: 5,
            hard_max: 90,
        },
    ),
];

/// The arguments the oracle takes for a budget set.
pub fn as_args(budgets: &Budgets) -> Vec<String> {
    [
        budgets.target,
        budgets.overlap,
        budgets.min_tokens,
        budgets.hard_max,
    ]
    .iter()
    .map(usize::to_string)
    .collect()
}
