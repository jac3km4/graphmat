use bumpalo::collections::{CollectIn, Vec as BumpVec};
use bumpalo::Bump;

use crate::levenshtein::{levenshtein_matrix, LevenshteinMatrix};
use crate::match_star::MatchContext;
use crate::object::CodeMetadata;

/// A macro for creating a heuristic composed of multiple heuristics.
#[macro_export]
macro_rules! heuristics {
    ($expr:expr $(,)?) => {
        $expr
    };
    ($expr:expr, $($trail:tt)*) => {
        $crate::heuristics::Combined::new($expr, heuristics!($($trail)*))
    };
}

/// A heuristic for computing the distance between edges.
pub trait EdgeDistanceHeuristic {
    fn label<'bump>(
        &self,
        lhs: impl IntoIterator<Item = u64> + Clone,
        rhs: impl IntoIterator<Item = u64> + Clone,
        ctx: MatchContext<'_>,
        bump: &'bump Bump,
    ) -> LevenshteinMatrix<'bump>;
}

/// A heuristic that combines two heuristics.
#[derive(Debug)]
pub struct Combined<H, T>(H, T);

impl<H, T> Combined<H, T> {
    /// Creates a new combined heuristic.
    pub fn new(lhs: H, rhs: T) -> Self {
        Self(lhs, rhs)
    }
}

impl<H: EdgeDistanceHeuristic, T: EdgeDistanceHeuristic> EdgeDistanceHeuristic for Combined<H, T> {
    fn label<'bump>(
        &self,
        lhs: impl IntoIterator<Item = u64> + Clone,
        rhs: impl IntoIterator<Item = u64> + Clone,
        ctx: MatchContext<'_>,
        bump: &'bump Bump,
    ) -> LevenshteinMatrix<'bump> {
        let mat1 = self.0.label(lhs.clone(), rhs.clone(), ctx, bump);
        let mat2 = self.1.label(lhs, rhs, ctx, bump);

        if mat1.distance() <= mat2.distance() {
            mat1
        } else {
            mat2
        }
    }
}

/// A heuristic that labels edges based on the order of calls.
#[derive(Debug)]
pub struct CallOrder;

impl CallOrder {
    fn labels<'bump>(slice: impl IntoIterator<Item = u64> + Clone, bump: &'bump Bump) -> BumpVec<'bump, usize> {
        let mut indices: BumpVec<'bump, _> = slice
            .clone()
            .into_iter()
            .map(|addr| (addr, None::<usize>))
            .collect_in(bump);
        indices.sort();
        indices.dedup();

        let mut counter = 0;
        slice
            .into_iter()
            .map(|addr| {
                let i = indices.binary_search_by_key(&addr, |&(k, _)| k).unwrap();
                let (_, idx) = &mut indices[i];
                *idx.get_or_insert_with(|| {
                    counter += 1;
                    counter - 1
                })
            })
            .collect_in(bump)
    }
}

impl EdgeDistanceHeuristic for CallOrder {
    fn label<'bump>(
        &self,
        lhs: impl IntoIterator<Item = u64> + Clone,
        rhs: impl IntoIterator<Item = u64> + Clone,
        _ctx: MatchContext<'_>,
        bump: &'bump Bump,
    ) -> LevenshteinMatrix<'bump> {
        levenshtein_matrix(&Self::labels(lhs, bump), &Self::labels(rhs, bump), bump)
    }
}

/// A heuristic that labels edges based on the relative number of opcodes.
#[derive(Debug)]
pub struct RelativeCodeSize;

impl RelativeCodeSize {
    fn labels<'bump>(
        &self,
        lhs: impl IntoIterator<Item = u64>,
        rhs: impl IntoIterator<Item = u64>,
        ctx: MatchContext<'_>,
        bump: &'bump Bump,
    ) -> (BumpVec<'bump, usize>, BumpVec<'bump, usize>) {
        fn weights<'bump>(
            it: impl IntoIterator<Item = u64>,
            ctx: &CodeMetadata,
            bump: &'bump Bump,
        ) -> BumpVec<'bump, (usize, f64)> {
            let lens: BumpVec<'bump, _> = it
                .into_iter()
                .map(|addr| ctx.get_function(addr).unwrap().opcodes().len())
                .collect_in(bump);

            let Some(&max_len) = lens.iter().max() else {
                return BumpVec::new_in(bump);
            };
            let mut weights: BumpVec<'bump, _> = lens
                .into_iter()
                .enumerate()
                .map(move |(idx, len)| (idx, len as f64 / max_len as f64))
                .collect_in(bump);
            weights.sort_by_key(|(_, x)| x.to_bits());
            weights
        }

        let it1 = weights(lhs, ctx.lhs_metadata(), bump);
        let mut it2 = weights(rhs, ctx.rhs_metadata(), bump).into_iter().peekable();
        let mut counter = 0;

        let mut labels1 = bumpalo::vec![in bump; usize::MAX; it1.len()];
        let mut labels2 = bumpalo::vec![in bump; usize::MAX; it2.len()];

        for (i1, w1) in it1 {
            let Some((mut i2, w2)) = it2.next() else {
                break;
            };
            let diff = (w1 - w2).abs();
            while let Some((j, _)) = it2.next_if(|(_, w)| (w1 - w).abs() < diff) {
                i2 = j;
            }
            labels1[i1] = i1;
            labels2[i2] = i1;
            counter = i1.max(counter) + 1;
        }

        for lab in labels1.iter_mut().chain(labels2.iter_mut()) {
            if *lab == usize::MAX {
                *lab = counter;
                counter += 1;
            }
        }

        (labels1, labels2)
    }
}

impl EdgeDistanceHeuristic for RelativeCodeSize {
    fn label<'bump>(
        &self,
        lhs: impl IntoIterator<Item = u64> + Clone,
        rhs: impl IntoIterator<Item = u64> + Clone,
        ctx: MatchContext<'_>,
        bump: &'bump Bump,
    ) -> LevenshteinMatrix<'bump> {
        let (labels_l, labels_r) = self.labels(lhs, rhs, ctx, bump);
        levenshtein_matrix(&labels_l, &labels_r, bump)
    }
}

#[cfg(test)]
mod test {
    use iced_x86::Mnemonic;
    use test_case::test_case;

    use super::*;
    use crate::graph::Graph;
    use crate::object::FunctionMetadata;

    fn test_obj1() -> CodeMetadata {
        let func1 = FunctionMetadata::new(vec![Mnemonic::Call, Mnemonic::Mov]);
        let func2 = FunctionMetadata::new(vec![Mnemonic::Mov]);
        let func3 = FunctionMetadata::new(vec![]);
        CodeMetadata {
            call_graph: Graph::new(),
            functions: [(512, func1.clone()), (513, func2.clone()), (514, func3.clone())]
                .into_iter()
                .collect(),
        }
    }

    fn test_obj2() -> CodeMetadata {
        let func1 = FunctionMetadata::new(vec![Mnemonic::Call, Mnemonic::Mov]);
        let func2 = FunctionMetadata::new(vec![Mnemonic::Mov]);
        let func3 = FunctionMetadata::new(vec![]);
        CodeMetadata {
            call_graph: Graph::new(),
            functions: [(1024, func1), (1025, func2), (1026, func3)].into_iter().collect(),
        }
    }

    #[test_case(&[512, 513, 514], &[1024, 1025, 1026], &[0, 1, 2], &[0, 1, 2])]
    #[test_case(&[512, 513, 514], &[1025, 1026], &[4, 1, 2], &[1, 2])]
    #[test_case(&[514], &[1024, 1025, 1026], &[0], &[1, 2, 0])]
    #[test_case(&[], &[1024, 1025, 1026], &[], &[0, 1, 2])]
    #[test_case(&[512, 513, 514], &[], &[0, 1, 2], &[])]
    #[test_case(&[514, 512, 513], &[1024, 1025, 1026], &[0, 1, 2], &[1, 2, 0])]
    fn test_label_code_size(lhs: &[u64], rhs: &[u64], lhs_labels: &[usize], rhs_labels: &[usize]) {
        let bump = Bump::new();
        let (l, r) = RelativeCodeSize.labels(
            lhs.iter().copied(),
            rhs.iter().copied(),
            MatchContext::new(&test_obj1(), &test_obj2()),
            &bump,
        );
        assert_eq!(l, lhs_labels);
        assert_eq!(r, rhs_labels);
    }

    #[test_case(&[512, 513, 514], &[0, 1, 2])]
    #[test_case(&[512, 513, 513, 514, 513], &[0, 1, 1, 2, 1])]
    #[test_case(&[512, 513, 514, 513, 514, 515, 513, 514, 512], &[0, 1, 2, 1, 2, 3, 1, 2, 0])]
    fn test_label_call_order(edges: &[u64], labels: &[usize]) {
        let bump = Bump::new();
        let res = CallOrder::labels(edges.iter().copied(), &bump);
        assert_eq!(res, labels);
    }
}
