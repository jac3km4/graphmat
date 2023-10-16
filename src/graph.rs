use std::hash::Hash;

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

    /// Returns a [Star] representing the node and its vertices.
    #[inline]
    pub fn get_star(&self, node: A) -> Star<'_, A> {
        Star {
            vertex: node,
            graph: self,
        }
    }
}

/// A vertex and its edges.
#[derive(Debug)]
pub struct Star<'graph, A> {
    vertex: A,
    graph: &'graph Graph<A>,
}

impl<'graph, A: Eq + PartialEq + Hash> Star<'graph, A> {
    /// Returns the vertex.
    #[inline]
    pub fn vertex(&self) -> &A {
        &self.vertex
    }

    /// Returns an iterator over the edges.
    #[inline]
    pub fn edges(&self) -> impl Iterator<Item = &'graph A> + Clone {
        self.graph.0.get_all(&self.vertex)
    }
}
