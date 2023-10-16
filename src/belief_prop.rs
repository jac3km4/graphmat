use std::collections::{BTreeSet, BinaryHeap};
use std::fmt;

use bumpalo::Bump;
use hashbrown::HashSet;

use crate::heuristics::EdgeDistanceHeuristic;
use crate::match_star::{match_star, MatchContext};
use crate::object::ObjectMetadata;

/// Performs call graph matching with the specified partial matching and heuristics.
/// The algorithm is based on
/// [Error-tolerant graph matching in linear computational cost using an initial small partial matching](https://www.sciencedirect.com/science/article/abs/pii/S0167865518301235).
pub fn belief_prop(
    lhs: &ObjectMetadata,
    rhs: &ObjectMetadata,
    seeds: impl IntoIterator<Item = (u64, u64)>,
    heuristics: &impl EdgeDistanceHeuristic,
) -> Mapping {
    let mut bump = Bump::new();

    let mut pending = BinaryHeap::new();
    let mut matching = BTreeSet::new();
    let mut matching_inv = BTreeSet::new();
    let mut computed = HashSet::new();
    let ctx = MatchContext::new(lhs, rhs);

    for pair in seeds {
        let star0 = lhs.call_graph().get_star(pair.0);
        let star1 = rhs.call_graph().get_star(pair.1);
        let (dist, map) = match_star(star0, star1, heuristics, ctx, &bump);
        bump.reset();
        computed.insert(pair);
        pending.push(PendingItem::new(pair, dist, map));
    }

    while let Some(item) = pending.pop() {
        let inverted = (item.pair.1, item.pair.0);

        matching.insert(item.pair);
        matching_inv.insert(inverted);
        pending.retain(|i| i.pair != item.pair && i.pair != inverted);

        for &mapping in &item.mappings {
            if !computed.contains(&mapping)
                && !matching
                    .range((mapping.0, u64::MIN)..=(mapping.0, u64::MAX))
                    .any(|_| true)
                && !matching_inv
                    .range((mapping.1, u64::MIN)..=(mapping.1, u64::MAX))
                    .any(|_| true)
            {
                let star0 = lhs.call_graph().get_star(mapping.0);
                let star1 = rhs.call_graph().get_star(mapping.1);
                let (dist, candidate_mappings) = match_star(star0, star1, heuristics, ctx, &bump);
                bump.reset();
                computed.insert(mapping);
                pending.push(PendingItem::new(mapping, dist, candidate_mappings));
            }
        }
    }

    Mapping {
        set: matching,
        lhs_base: lhs.text_segment_base(),
        rhs_base: rhs.text_segment_base(),
    }
}

/// A mapping between two call graphs.
#[derive(Debug)]
pub struct Mapping {
    set: BTreeSet<(u64, u64)>,
    lhs_base: u64,
    rhs_base: u64,
}

impl Mapping {
    pub fn mappings(&self) -> impl Iterator<Item = (u64, u64)> + '_ {
        self.set
            .iter()
            .copied()
            .map(move |(lhs, rhs)| (self.lhs_base + lhs, self.rhs_base + rhs))
    }
}

impl fmt::Display for Mapping {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (l, r) in self.mappings() {
            writeln!(f, "{:#X} -> {:#X}", l, r)?;
        }
        Ok(())
    }
}

#[derive(Debug, Eq)]
struct PendingItem {
    pair: (u64, u64),
    dist: usize,
    mappings: Vec<(u64, u64)>,
}

impl PendingItem {
    fn new(pair: (u64, u64), dist: usize, mappings: Vec<(u64, u64)>) -> Self {
        Self { pair, dist, mappings }
    }
}

impl PartialEq for PendingItem {
    fn eq(&self, other: &Self) -> bool {
        self.dist == other.dist
    }
}

impl PartialOrd for PendingItem {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PendingItem {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.dist.cmp(&other.dist).reverse()
    }
}
