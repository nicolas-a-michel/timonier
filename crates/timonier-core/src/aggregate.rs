//! Edges at a zoom level: aggregated from the stored edges, never stored (spec, ADR 0009).

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{Edge, EdgeKind, NodeId, Provenance};

/// An edge between two visible boxes, standing for every edge between their contents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggregatedEdge {
    pub src: NodeId,
    pub dst: NodeId,
    pub kind: EdgeKind,
    /// Sum of the weights of the edges it stands for.
    pub weight: u64,
    /// How many stored edges it stands for.
    pub edges: u32,
    /// The most reliable provenance among them.
    pub best: Provenance,
    /// True when they all come from approximate sources (tree-sitter alone): the map draws such
    /// edges differently (spec « Displayed precision »).
    pub approximate_only: bool,
    /// How many of them a manifest, a language server or SCIP confirmed: the map says how sure
    /// it is (PLAN.md 11.5).
    #[serde(default)]
    pub confirmed: u32,
}

/// Maps every edge endpoint to its nearest visible ancestor (itself included) and merges the
/// edges that land on the same pair of visible nodes.
///
/// `parents` gives the parent of every present node. Edges with an endpoint that is not present
/// (dangling, ADR 0009) or not under a visible node are dropped, as are edges inside one
/// visible node. The result is sorted by (src, dst, kind).
pub fn aggregate<'a>(
    parents: &HashMap<NodeId, Option<NodeId>>,
    edges: impl IntoIterator<Item = &'a Edge>,
    visible: &HashSet<NodeId>,
) -> Vec<AggregatedEdge> {
    let mut nearest: HashMap<&NodeId, Option<&NodeId>> = HashMap::new();
    let mut merged: BTreeMap<(NodeId, NodeId, EdgeKind), AggregatedEdge> = BTreeMap::new();
    for edge in edges {
        let src = nearest_visible(&edge.src, parents, visible, &mut nearest);
        let dst = nearest_visible(&edge.dst, parents, visible, &mut nearest);
        let (Some(src), Some(dst)) = (src, dst) else {
            continue;
        };
        if src == dst {
            continue;
        }
        let entry = merged
            .entry((src.clone(), dst.clone(), edge.kind))
            .or_insert_with(|| AggregatedEdge {
                src: src.clone(),
                dst: dst.clone(),
                kind: edge.kind,
                weight: 0,
                edges: 0,
                best: edge.provenance,
                approximate_only: true,
                confirmed: 0,
            });
        entry.weight += u64::from(edge.weight);
        entry.edges += 1;
        if edge.provenance.rank() > entry.best.rank() {
            entry.best = edge.provenance;
        }
        entry.approximate_only &= edge.provenance.is_approximate();
        if !edge.provenance.is_approximate() {
            entry.confirmed += 1;
        }
    }
    merged.into_values().collect()
}

fn nearest_visible<'a>(
    start: &'a NodeId,
    parents: &'a HashMap<NodeId, Option<NodeId>>,
    visible: &HashSet<NodeId>,
    memo: &mut HashMap<&'a NodeId, Option<&'a NodeId>>,
) -> Option<&'a NodeId> {
    let mut path = Vec::new();
    let mut current = start;
    let found = loop {
        if let Some(known) = memo.get(current) {
            break *known;
        }
        // A node that is not present (dangling endpoint) has no visible ancestor.
        let Some(parent) = parents.get(current) else {
            break None;
        };
        path.push(current);
        if visible.contains(current) {
            break Some(current);
        }
        match parent {
            Some(parent) => current = parent,
            None => break None,
        }
        if path.len() > parents.len() {
            break None; // A cycle: the store prevents it; stay safe anyway.
        }
    };
    for node in path {
        memo.insert(node, found);
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Confidence;

    fn id(s: &str) -> NodeId {
        NodeId::parse(s).unwrap()
    }

    fn edge(src: &str, dst: &str, provenance: Provenance, weight: u32) -> Edge {
        Edge {
            src: id(src),
            dst: id(dst),
            kind: EdgeKind::Calls,
            provenance,
            confidence: Confidence::CERTAIN,
            weight,
        }
    }

    /// repo → pkg a (files a1, a2), pkg b (file b1); symbols under the files.
    fn parents() -> HashMap<NodeId, Option<NodeId>> {
        [
            ("repo:r", None),
            ("pkg:cargo/a", Some("repo:r")),
            ("pkg:cargo/b", Some("repo:r")),
            ("file:a/1.rs", Some("pkg:cargo/a")),
            ("file:a/2.rs", Some("pkg:cargo/a")),
            ("file:b/1.rs", Some("pkg:cargo/b")),
            ("rust:a::f", Some("file:a/1.rs")),
            ("rust:a::g", Some("file:a/2.rs")),
            ("rust:b::h", Some("file:b/1.rs")),
        ]
        .into_iter()
        .map(|(n, p)| (id(n), p.map(id)))
        .collect()
    }

    #[test]
    fn edges_between_contents_become_one_edge_between_boxes() {
        let edges = [
            edge("rust:a::f", "rust:b::h", Provenance::Lsp, 2),
            edge("file:a/2.rs", "file:b/1.rs", Provenance::TreeSitter, 3),
            edge("rust:a::f", "rust:a::g", Provenance::Lsp, 1), // inside package a: hidden
        ];
        let visible = HashSet::from([id("pkg:cargo/a"), id("pkg:cargo/b")]);
        let aggregated = aggregate(&parents(), &edges, &visible);
        assert_eq!(
            aggregated,
            [AggregatedEdge {
                src: id("pkg:cargo/a"),
                dst: id("pkg:cargo/b"),
                kind: EdgeKind::Calls,
                weight: 5,
                edges: 2,
                best: Provenance::Lsp,
                approximate_only: false,
                confirmed: 1,
            }]
        );
    }

    #[test]
    fn zooming_in_splits_the_edge() {
        let edges = [
            edge("rust:a::f", "rust:b::h", Provenance::TreeSitter, 1),
            edge("rust:a::g", "rust:b::h", Provenance::TreeSitter, 1),
        ];
        let visible = HashSet::from([id("file:a/1.rs"), id("file:a/2.rs"), id("pkg:cargo/b")]);
        let aggregated = aggregate(&parents(), &edges, &visible);
        assert_eq!(aggregated.len(), 2);
        assert!(
            aggregated
                .iter()
                .all(|e| e.approximate_only && e.dst == id("pkg:cargo/b"))
        );
    }

    #[test]
    fn aggregated_edges_round_trip_through_json() {
        let edge = AggregatedEdge {
            src: id("pkg:cargo/a"),
            dst: id("pkg:cargo/b"),
            kind: EdgeKind::Calls,
            weight: 5,
            edges: 2,
            best: Provenance::Lsp,
            approximate_only: false,
            confirmed: 2,
        };
        let json = serde_json::to_string(&edge).unwrap();
        assert_eq!(serde_json::from_str::<AggregatedEdge>(&json).unwrap(), edge);
    }

    #[test]
    fn dangling_and_invisible_endpoints_are_dropped() {
        let edges = [
            edge("rust:a::f", "rust:not::indexed", Provenance::Lsp, 1),
            edge("rust:a::f", "rust:b::h", Provenance::Lsp, 1),
        ];
        let visible = HashSet::from([id("pkg:cargo/a")]);
        assert!(aggregate(&parents(), &edges, &visible).is_empty());
    }
}
