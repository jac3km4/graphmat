use std::hash::Hash;

use ordered_multimap::list_ordered_multimap::EntryValues;
use ordered_multimap::ListOrderedMultimap;

/// A graph represented as an adjacency list.
#[derive(Debug, Default)]
pub struct Graph<A>(ListOrderedMultimap<A, A>);

impl<A: Eq + PartialEq + Hash> Graph<A> {
    /// Create a new empty graph.
    #[inline]
    pub fn new() -> Self {
        Self(ListOrderedMultimap::new())
    }

    /// Adds an edge to the graph.
    #[inline]
    pub fn add_edge(&mut self, a: A, b: A) {
        self.0.append(a, b);
    }

    /// Checks whether the graph contains a vertex.
    #[inline]
    pub fn has_vertex(&self, a: A) -> bool {
        self.0.contains_key(&a)
    }

    /// Returns a [Star] representing the vertex and its edges.
    #[inline]
    pub fn get_star(&self, vertex: A) -> Star<'_, A> {
        Star {
            edges: self.0.get_all(&vertex),
            vertex,
        }
    }
}

/// A vertex and its edges.
#[derive(Debug)]
pub struct Star<'graph, A> {
    vertex: A,
    edges: EntryValues<'graph, A, A>,
}

impl<'graph, A> Star<'graph, A> {
    /// Returns the vertex.
    #[inline]
    pub fn vertex(&self) -> &A {
        &self.vertex
    }

    /// Returns an iterator over the edges.
    #[inline]
    pub fn edges(&self) -> impl ExactSizeIterator<Item = &'graph A> + Clone {
        self.edges.clone()
    }
}
