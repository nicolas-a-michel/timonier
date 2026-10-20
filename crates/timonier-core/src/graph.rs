//! A snapshot of the graph in memory, with its tree: what the views and the review diff read
//! (ADR 0019, 0024).

use std::collections::HashMap;

use crate::{Edge, IdForm, Node, NodeId};

#[derive(Debug, Default)]
pub struct Graph {
    nodes: HashMap<NodeId, Node>,
    /// Children of each node, sorted by id.
    children: HashMap<NodeId, Vec<NodeId>>,
    parents: HashMap<NodeId, Option<NodeId>>,
    edges: Vec<Edge>,
    /// Nodes without a parent, sorted: the repository.
    roots: Vec<NodeId>,
}

impl Graph {
    pub fn new(nodes: Vec<Node>, edges: Vec<Edge>) -> Self {
        let mut children: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        let mut parents = HashMap::with_capacity(nodes.len());
        let mut roots = Vec::new();
        for node in &nodes {
            parents.insert(node.id.clone(), node.parent.clone());
            match &node.parent {
                Some(parent) => children
                    .entry(parent.clone())
                    .or_default()
                    .push(node.id.clone()),
                None => roots.push(node.id.clone()),
            }
        }
        for list in children.values_mut() {
            list.sort();
        }
        roots.sort();
        Self {
            nodes: nodes.into_iter().map(|n| (n.id.clone(), n)).collect(),
            children,
            parents,
            edges,
            roots,
        }
    }

    pub fn node(&self, id: &NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }

    pub fn contains(&self, id: &NodeId) -> bool {
        self.nodes.contains_key(id)
    }

    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.values()
    }

    /// The children of a node, sorted by id.
    pub fn children(&self, id: &NodeId) -> &[NodeId] {
        self.children.get(id).map_or(&[], Vec::as_slice)
    }

    /// The parent of every node.
    pub fn parents(&self) -> &HashMap<NodeId, Option<NodeId>> {
        &self.parents
    }

    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    pub fn roots(&self) -> &[NodeId] {
        &self.roots
    }

    /// The node and its ancestors, from the node up; nothing when the node is not here.
    pub fn ancestors<'g>(&'g self, id: &NodeId) -> impl Iterator<Item = &'g NodeId> {
        let mut next = self.nodes.get(id).map(|n| &n.id);
        std::iter::from_fn(move || {
            let current = next?;
            next = self.parents.get(current).and_then(Option::as_ref);
            Some(current)
        })
    }

    /// The file that holds a node: itself, or the nearest file above a symbol.
    pub fn file_of(&self, id: &NodeId) -> Option<&NodeId> {
        for current in self.ancestors(id) {
            if matches!(current.form(), IdForm::File { .. }) {
                return Some(current);
            }
            if !self.nodes.get(current)?.kind.is_symbol() {
                return None;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NodeKind, SymbolKind};

    fn id(s: &str) -> NodeId {
        NodeId::parse(s).unwrap()
    }

    fn graph() -> Graph {
        let node = |i: &str, kind, parent: Option<&str>| {
            let node = Node::new(id(i), kind, i);
            match parent {
                Some(parent) => node.with_parent(id(parent)),
                None => node,
            }
        };
        let function = NodeKind::Symbol(SymbolKind::Function);
        Graph::new(
            vec![
                node("repo:r", NodeKind::Repo, None),
                node("pkg:cargo/a", NodeKind::Package, Some("repo:r")),
                node("file:a/src/lib.rs", NodeKind::File, Some("pkg:cargo/a")),
                node("rust:a::m", function, Some("file:a/src/lib.rs")),
                node("rust:a::m::f", function, Some("rust:a::m")),
            ],
            Vec::new(),
        )
    }

    #[test]
    fn ancestors_go_from_the_node_up_to_the_repository() {
        let graph = graph();
        let chain: Vec<&str> = graph
            .ancestors(&id("rust:a::m::f"))
            .map(NodeId::as_str)
            .collect();
        assert_eq!(
            chain,
            [
                "rust:a::m::f",
                "rust:a::m",
                "file:a/src/lib.rs",
                "pkg:cargo/a",
                "repo:r"
            ]
        );
        assert_eq!(graph.ancestors(&id("rust:gone::x")).count(), 0);
        assert_eq!(graph.roots(), [id("repo:r")]);
        assert_eq!(
            graph.children(&id("pkg:cargo/a")),
            [id("file:a/src/lib.rs")]
        );
    }

    #[test]
    fn a_symbol_is_in_the_nearest_file_above_it_and_a_package_in_none() {
        let graph = graph();
        let lib = id("file:a/src/lib.rs");
        assert_eq!(graph.file_of(&id("rust:a::m::f")), Some(&lib));
        assert_eq!(graph.file_of(&lib), Some(&lib));
        assert_eq!(graph.file_of(&id("pkg:cargo/a")), None);
        assert_eq!(graph.file_of(&id("rust:gone::x")), None);
    }
}
