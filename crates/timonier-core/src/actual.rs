//! `.timonier/actual.json`, the real architecture as the code holds it (ADR 0033): the boxes of
//! the map (packages and components) and the dependencies between them, aggregated as on the
//! map. Written before each commit; the whiteboard is compared to it (Phase 7). No date and no
//! commit: the same graph always gives the same file.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::graph::Graph;
use crate::tracking::FORMAT_VERSION;
use crate::{NodeId, NodeKind, aggregate};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Actual {
    pub version: u32,
    /// By id.
    pub boxes: Vec<ActualBox>,
    /// By the box they leave, then the one they reach.
    pub dependencies: Vec<ActualDependency>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActualBox {
    /// Its id in the graph (`pkg:cargo/demo`, `component:dir/src`).
    pub id: String,
    /// `package` or `component`.
    pub kind: String,
    pub name: String,
    /// Its architectural role (`entry_point`, `utility`…), when it has one.
    pub role: Option<String>,
    /// The box that holds it; `null` at the top.
    pub parent: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ts_rs::TS)]
#[serde(deny_unknown_fields)]
pub struct ActualDependency {
    pub from: String,
    pub to: String,
    /// The kinds of the edges it stands for (`calls`, `imports`, `depends_on`…), sorted.
    pub kinds: Vec<String>,
    /// The sum of their weights.
    pub weight: u64,
}

fn is_box(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::Package | NodeKind::Component)
}

/// The architecture `graph` holds.
pub fn actual(graph: &Graph) -> Actual {
    let boxed: HashSet<NodeId> = graph
        .nodes()
        .filter(|n| is_box(n.kind))
        .map(|n| n.id.clone())
        .collect();
    let mut boxes: Vec<ActualBox> = graph
        .nodes()
        .filter(|n| is_box(n.kind))
        .map(|node| ActualBox {
            id: node.id.to_string(),
            kind: node.kind.as_str().to_owned(),
            name: node.name.clone(),
            role: node.role.map(|r| r.as_str().to_owned()),
            parent: graph
                .ancestors(&node.id)
                .skip(1)
                .find(|a| boxed.contains(*a))
                .map(ToString::to_string),
        })
        .collect();
    boxes.sort_by(|a, b| a.id.cmp(&b.id));
    // Each edge between the deepest boxes that hold its ends, as the map draws them.
    let mut merged: BTreeMap<(String, String), (BTreeSet<&'static str>, u64)> = BTreeMap::new();
    for edge in aggregate::aggregate(graph.parents(), graph.edges(), &boxed) {
        let entry = merged
            .entry((edge.src.to_string(), edge.dst.to_string()))
            .or_default();
        entry.0.insert(edge.kind.as_str());
        entry.1 += edge.weight;
    }
    let dependencies = merged
        .into_iter()
        .map(|((from, to), (kinds, weight))| ActualDependency {
            from,
            to,
            kinds: kinds.into_iter().map(str::to_owned).collect(),
            weight,
        })
        .collect();
    Actual {
        version: FORMAT_VERSION,
        boxes,
        dependencies,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Edge, EdgeKind, Node, Provenance, Role};

    fn id(s: &str) -> NodeId {
        NodeId::parse(s).unwrap()
    }

    fn node(i: &str, kind: NodeKind, parent: Option<&str>) -> Node {
        let mut node = Node::new(id(i), kind, i.rsplit(['/', ':']).next().unwrap());
        if let Some(parent) = parent {
            node = node.with_parent(id(parent));
        }
        node
    }

    fn edge(src: &str, dst: &str, kind: EdgeKind, provenance: Provenance, weight: u32) -> Edge {
        Edge {
            src: id(src),
            dst: id(dst),
            kind,
            provenance,
            confidence: crate::Confidence::CERTAIN,
            weight,
        }
    }

    #[test]
    fn the_boxes_and_their_dependencies_as_the_map_aggregates_them() {
        let mut a = node("pkg:cargo/a", NodeKind::Package, Some("repo:r"));
        a.role = Some(Role::EntryPoint);
        let graph = Graph::new(
            vec![
                node("repo:r", NodeKind::Repo, None),
                a,
                node(
                    "component:dir/a/net",
                    NodeKind::Component,
                    Some("pkg:cargo/a"),
                ),
                node(
                    "file:a/net/x.rs",
                    NodeKind::File,
                    Some("component:dir/a/net"),
                ),
                node("file:a/lib.rs", NodeKind::File, Some("pkg:cargo/a")),
                node("pkg:cargo/b", NodeKind::Package, Some("repo:r")),
                node("file:b/lib.rs", NodeKind::File, Some("pkg:cargo/b")),
            ],
            vec![
                edge(
                    "file:a/net/x.rs",
                    "file:b/lib.rs",
                    EdgeKind::Calls,
                    Provenance::TreeSitter,
                    2,
                ),
                edge(
                    "file:a/lib.rs",
                    "file:b/lib.rs",
                    EdgeKind::Imports,
                    Provenance::TreeSitter,
                    1,
                ),
                edge(
                    "pkg:cargo/a",
                    "pkg:cargo/b",
                    EdgeKind::DependsOn,
                    Provenance::Manifest,
                    1,
                ),
                // Inside one box: no dependency.
                edge(
                    "file:a/net/x.rs",
                    "file:a/net/x.rs",
                    EdgeKind::Calls,
                    Provenance::TreeSitter,
                    1,
                ),
            ],
        );
        let actual = actual(&graph);
        let ids: Vec<&str> = actual.boxes.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, ["component:dir/a/net", "pkg:cargo/a", "pkg:cargo/b"]);
        assert_eq!(actual.boxes[0].parent.as_deref(), Some("pkg:cargo/a"));
        assert_eq!(actual.boxes[1].parent, None);
        assert_eq!(actual.boxes[1].role.as_deref(), Some("entry_point"));
        let deps: Vec<(&str, &str, Vec<&str>, u64)> = actual
            .dependencies
            .iter()
            .map(|d| {
                (
                    d.from.as_str(),
                    d.to.as_str(),
                    d.kinds.iter().map(String::as_str).collect(),
                    d.weight,
                )
            })
            .collect();
        assert_eq!(
            deps,
            [
                ("component:dir/a/net", "pkg:cargo/b", vec!["calls"], 2),
                (
                    "pkg:cargo/a",
                    "pkg:cargo/b",
                    vec!["depends_on", "imports"],
                    2
                ),
            ]
        );
        // The same graph, the same file.
        assert_eq!(
            serde_json::to_string(&actual).unwrap(),
            serde_json::to_string(&super::actual(&graph)).unwrap()
        );
    }
}
