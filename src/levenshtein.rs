use std::mem;

use bumpalo::collections::Vec;
use bumpalo::Bump;

/// Computes the Levenshtein distance between the given two slices without using a matrix.
/// It's more efficient than [`levenshtein_matrix`], but it cannot be used to generate an
/// optimal sequence of edits.
pub fn levenshtein<A>(s: &[A], t: &[A], bump: &Bump) -> usize
where
    A: PartialEq,
{
    let n = t.len();
    let mut v0 = bump.alloc_slice_fill_iter(0..n + 1);
    let mut v1 = bump.alloc_slice_fill_copy(n + 1, 0);

    for (i, si) in s.iter().enumerate() {
        v1[0] = i + 1;

        for j in 0..n {
            let deletion_cost = v0[j + 1] + 1;
            let insertion_cost = v1[j] + 1;
            let substitution_cost = if *si == t[j] { v0[j] } else { v0[j] + 1 };

            v1[j + 1] = deletion_cost.min(insertion_cost).min(substitution_cost);
        }

        mem::swap(&mut v0, &mut v1);
    }

    v0[n]
}

/// Computes a [`LevenshteinMatrix`] for the given two slices. The resulting matrix can be used
/// to obtain a distance and an optimal sequence of edits.
pub fn levenshtein_matrix<'a, A>(s: &[A], t: &[A], bump: &'a Bump) -> LevenshteinMatrix<'a>
where
    A: PartialEq,
{
    let mut mat = LevenshteinMatrix::new(s.len(), t.len(), bump);

    for (j, tc) in t.iter().enumerate() {
        for (i, sc) in s.iter().enumerate() {
            let substitution_cost = if sc == tc { 0 } else { 1 };
            let cost = (mat.get(i, j + 1) + 1)
                .min(mat.get(i + 1, j) + 1)
                .min(mat.get(i, j) + substitution_cost);
            mat.set(i + 1, j + 1, cost);
        }
    }

    mat
}

/// A Levenshtein distance matrix.
#[derive(Debug, Default)]
pub struct LevenshteinMatrix<'a> {
    cols: usize,
    rows: usize,
    matrix: &'a mut [usize],
}

impl<'a> LevenshteinMatrix<'a> {
    #[inline]
    pub fn new(cols: usize, rows: usize, bump: &'a Bump) -> Self {
        let n = cols + 1;
        let m = rows + 1;
        let mut this = Self {
            cols: n,
            rows: m,
            matrix: bump.alloc_slice_fill_copy(n * m, 0),
        };

        for i in 0..n {
            this.set(i, 0, i);
        }
        for i in 0..m {
            this.set(0, i, i);
        }
        this
    }

    /// Returns the value at the given column and row.
    #[inline]
    pub fn get(&self, col: usize, row: usize) -> usize {
        self.matrix[row * self.cols + col]
    }

    /// Sets the value at the given column and row.
    #[inline]
    pub fn set(&mut self, col: usize, row: usize, value: usize) {
        self.matrix[row * self.cols + col] = value;
    }

    /// Returns an iterator over an optimal sequence of edits.
    /// The iterator yields edits starting from the end of the string.
    #[inline]
    pub fn edits(&self) -> Edits<'_> {
        Edits {
            x: self.cols - 1,
            y: self.rows - 1,
            matrix: self,
        }
    }

    /// Returns the Levenshtein distance between the two strings.
    #[inline]
    pub fn distance(&self) -> usize {
        self.get(self.cols - 1, self.rows - 1)
    }
}

/// An iterator over edits.
#[derive(Debug)]
pub struct Edits<'a> {
    matrix: &'a LevenshteinMatrix<'a>,
    x: usize,
    y: usize,
}

impl<'a> Edits<'a> {
    /// Returns an iterator over edits with indices at which they occur.
    pub fn with_indices(self) -> impl Iterator<Item = (usize, Edit)> + 'a {
        let mut i = self.matrix.cols - 1;
        self.map(move |edit| {
            i -= match edit {
                Edit::Insert(_) => 0,
                Edit::Delete | Edit::Substitute(_) | Edit::Noop => 1,
            };
            (i, edit)
        })
    }

    /// Applies the edits to the given input.
    pub fn apply(self, lhs: &mut Vec<'_, u8>, rhs: &[u8]) {
        for (i, edit) in self.with_indices() {
            edit.apply(i, lhs, rhs);
        }
    }
}

impl<'a> Iterator for Edits<'a> {
    type Item = Edit;

    fn next(&mut self) -> Option<Self::Item> {
        if self.x == 0 && self.y == 0 {
            return None;
        }

        let current = self.matrix.get(self.x, self.y);
        let x1 = self.x.checked_sub(1);
        let y1 = self.y.checked_sub(1);
        let diagonal = (|| Some(self.matrix.get(x1?, y1?)))().unwrap_or(usize::MAX);
        let left = (|| Some(self.matrix.get(x1?, self.y)))().unwrap_or(usize::MAX);
        let up = (|| Some(self.matrix.get(self.x, y1?)))().unwrap_or(usize::MAX);

        if diagonal <= left && diagonal <= up && diagonal <= current {
            self.x -= 1;
            self.y -= 1;
            if diagonal == current {
                Some(Edit::Noop)
            } else {
                Some(Edit::Substitute(self.y))
            }
        } else if left <= up && left <= current {
            self.x -= 1;
            Some(Edit::Delete)
        } else {
            self.y -= 1;
            Some(Edit::Insert(self.y))
        }
    }
}

/// An edit operation.
#[derive(Debug, PartialEq, Eq)]
pub enum Edit {
    Insert(usize),
    Delete,
    Substitute(usize),
    Noop,
}

impl Edit {
    pub fn apply(self, pos: usize, lhs: &mut Vec<'_, u8>, rhs: &[u8]) {
        match self {
            Edit::Insert(x) => {
                lhs.insert(pos, rhs[x]);
            }
            Edit::Delete => {
                lhs.remove(pos);
            }
            Edit::Substitute(x) => {
                lhs[pos] = rhs[x];
            }
            Edit::Noop => {}
        }
    }
}

#[cfg(test)]
mod test {
    use bumpalo::collections::CollectIn;
    use test_case::test_case;

    use super::*;

    #[test_case(b"kitten", b"sitting", 3)]
    #[test_case(b"Saturday", b"Sunday", 3)]
    #[test_case(b"Mariah Carey", b"Leonard Cohen", 9)]
    #[test_case(b"kitteenns", b"kiteeenss", 2)]
    fn test_levenshtein(s1: &[u8], s2: &[u8], expected: usize) {
        let bump = Bump::new();
        let result = super::levenshtein(s1, s2, &bump);
        assert_eq!(result, expected);
    }

    #[test_case(b"kitten", b"sitting", 3)]
    #[test_case(b"Saturday", b"Sunday", 3)]
    #[test_case(b"Mariah Carey", b"Leonard Cohen", 9)]
    #[test_case(b"kitteenns", b"kiteeenss", 2)]
    fn test_levenshtein_matrix(s1: &[u8], s2: &[u8], expected: usize) {
        let bump = Bump::new();
        let result = super::levenshtein_matrix(s1, s2, &bump);
        assert_eq!(result.distance(), expected);

        let mut tmp = s1.iter().copied().collect_in(&bump);
        result.edits().apply(&mut tmp, s2);
        assert_eq!(tmp, s2);
    }
}
