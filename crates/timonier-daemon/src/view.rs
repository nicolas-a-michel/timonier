//! Views of the map (ADR 0019): what a subscription shows of the head, and what changed in it
//! since the client's last revision; with the review diff (ADR 0024). Pure, apart from
//! [`load`].

use std::collections::{BTreeMap, HashMap, HashSet};

use timonier_core::protocol::{
    EdgeRef, EdgeReview, FanJump, NodeDebt, NodeReview, ReviewBox, ReviewChange, ReviewDependency,
    ReviewFan, ReviewMark, ReviewSummary, ViewDelta, ViewEdge, ViewNode, ViewSnapshot, ViewSpec,
};
use timonier_core::review::{self, Change, Dependency, Jump, ReviewDiff};
use timonier_core::{IdForm, Node, NodeId, NodeKind, aggregate};
use timonier_store::Reader;

use crate::Result;

pub use timonier_core::graph::Graph;

/// The head, read once for every view (PLAN.md, Phase 2: aggregation reads the whole graph).
pub fn load(reader: &mut Reader) -> Result<Graph> {
    let head = reader.head()?.seq;
    Ok(Graph::new(reader.nodes_at(head)?, reader.edges_at(head)?))
}

/// The path of the file that holds a node, relative to the root.
fn file_path(graph: &Graph, id: &NodeId) -> Option<String> {
    match graph.file_of(id)?.form() {
        IdForm::File { path } => Some(path.to_owned()),
        _ => None,
    }
}

/// The review diff the views show, computed once per change of the head (ADR 0024).
pub struct Review<'a> {
    /// The snapshot of the last review.
    pub reviewed: &'a Graph,
    pub diff: &'a ReviewDiff,
    pub summary: &'a ReviewSummary,
}

/// What the summary lists of a review diff: boxes by name, then by id.
pub fn summary(
    mark: ReviewMark,
    reviewed: &Graph,
    head: &Graph,
    diff: &ReviewDiff,
) -> ReviewSummary {
    let named = |graph: &Graph, id: &NodeId| graph.node(id).map(|n| review_box(graph, n));
    // The review's id of a box of the head, when it was renamed since.
    let was: BTreeMap<&NodeId, &NodeId> =
        diff.renamed.iter().map(|(from, to)| (to, from)).collect();
    // A dependency added is named as the head has it; one removed, as the review had it (Q34).
    let now = |id: &NodeId| named(head, id).or_else(|| named(reviewed, id));
    let then = |id: &NodeId| {
        named(reviewed, was.get(id).copied().unwrap_or(id)).or_else(|| named(head, id))
    };
    let boxes = |change: Change| -> Vec<ReviewBox> {
        sorted(
            diff.changes
                .iter()
                .filter(|(id, c)| **c == change && head.node(id).is_some_and(is_box))
                .filter_map(|(id, _)| named(head, id))
                .collect(),
        )
    };
    let dependencies = |set: &std::collections::BTreeSet<Dependency>,
                        name: &dyn Fn(&NodeId) -> Option<ReviewBox>|
     -> Vec<ReviewDependency> {
        let mut list: Vec<ReviewDependency> = set
            .iter()
            .filter_map(|d| {
                Some(ReviewDependency {
                    src: name(&d.src)?,
                    dst: name(&d.dst)?,
                })
            })
            .collect();
        list.sort_by(|a, b| {
            key(&a.src)
                .cmp(&key(&b.src))
                .then(key(&a.dst).cmp(&key(&b.dst)))
        });
        list
    };
    let mut cycles: Vec<Vec<ReviewBox>> = diff
        .cycles_added
        .iter()
        .map(|cycle| sorted(cycle.iter().filter_map(|id| named(head, id)).collect()))
        .collect();
    cycles.sort_by(|a, b| a.iter().map(key).cmp(b.iter().map(key)));
    let jump = |jump: &Jump| FanJump {
        before: jump.before,
        after: jump.after,
    };
    let mut fans: Vec<ReviewFan> = diff
        .fan_in
        .keys()
        .chain(diff.fan_out.keys())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter_map(|id| {
            Some(ReviewFan {
                item: named(head, id)?,
                fan_in: diff.fan_in.get(id).map(jump),
                fan_out: diff.fan_out.get(id).map(jump),
            })
        })
        .collect();
    fans.sort_by(|a, b| key(&a.item).cmp(&key(&b.item)));
    ReviewSummary {
        since: Some(mark),
        added: boxes(Change::Added),
        modified: boxes(Change::Modified),
        removed: sorted(
            diff.removed
                .iter()
                .filter_map(|id| named(reviewed, id))
                .collect(),
        ),
        dependencies_added: dependencies(&diff.dependencies_added, &now),
        dependencies_removed: dependencies(&diff.dependencies_removed, &then),
        cycles,
        fans,
    }
}

fn is_box(node: &Node) -> bool {
    matches!(node.kind, NodeKind::Component | NodeKind::Package)
}

pub(crate) fn review_box(graph: &Graph, node: &Node) -> ReviewBox {
    // The nearest box above: a package or a component, not the repository.
    let within = graph
        .ancestors(&node.id)
        .skip(1)
        .filter_map(|id| graph.node(id))
        .find(|n| is_box(n))
        .map(|n| n.name.clone());
    ReviewBox {
        id: node.id.clone(),
        kind: node.kind,
        name: node.name.clone(),
        role: node.role,
        within,
    }
}

fn key(item: &ReviewBox) -> (&str, &str) {
    (item.name.as_str(), item.id.as_str())
}

fn sorted(mut boxes: Vec<ReviewBox>) -> Vec<ReviewBox> {
    boxes.sort_by(|a, b| key(a).cmp(&key(b)));
    boxes
}

/// The review diff on one node of the map; `None` when nothing changed for it.
fn node_review(diff: &ReviewDiff, id: &NodeId) -> Option<NodeReview> {
    let jump = |jump: &Jump| FanJump {
        before: jump.before,
        after: jump.after,
    };
    let review = NodeReview {
        change: diff.changes.get(id).map(|change| match change {
            Change::Added => ReviewChange::Added,
            Change::Modified => ReviewChange::Modified,
        }),
        fan_in: diff.fan_in.get(id).map(jump),
        fan_out: diff.fan_out.get(id).map(jump),
        cycle: diff.cycle_of(id).is_some(),
    };
    (review.change.is_some() || review.fan_in.is_some() || review.fan_out.is_some() || review.cycle)
        .then_some(review)
}

/// What a view shows at one revision.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewState {
    nodes: BTreeMap<NodeId, ViewNode>,
    edges: BTreeMap<EdgeRef, ViewEdge>,
    review: ReviewSummary,
}

impl ViewState {
    /// The top of the map is what lies under the repository; each expanded node that is shown
    /// shows its children too. An expanded node under a collapsed one stays hidden.
    pub fn compute(graph: &Graph, spec: &ViewSpec, review: Option<&Review<'_>>) -> Self {
        Self::compute_with(graph, spec, review, None)
    }

    /// [`ViewState::compute`], with the review debt of each box (ADR 0030).
    pub fn compute_with(
        graph: &Graph,
        spec: &ViewSpec,
        review: Option<&Review<'_>>,
        debt: Option<&HashMap<NodeId, NodeDebt>>,
    ) -> Self {
        let expanded: HashSet<&NodeId> = spec.expanded.iter().collect();
        let mut top: Vec<&NodeId> = Vec::new();
        for root in graph.roots() {
            match graph.node(root).map(|n| n.kind) {
                Some(NodeKind::Repo) => top.extend(graph.children(root)),
                // A node without a parent that is not the repository: the store refuses it, but
                // show it rather than lose it.
                _ => top.push(root),
            }
        }
        let mut nodes = BTreeMap::new();
        let mut queue: Vec<(&NodeId, Option<&NodeId>)> =
            top.into_iter().map(|id| (id, None)).collect();
        while let Some((id, parent)) = queue.pop() {
            let Some(node) = graph.node(id) else {
                continue;
            };
            let children = graph.children(id);
            nodes.insert(
                id.clone(),
                ViewNode {
                    id: id.clone(),
                    kind: node.kind,
                    name: node.name.clone(),
                    parent: parent.cloned(),
                    role: node.role,
                    language: node.language.clone(),
                    children: u32::try_from(children.len()).unwrap_or(u32::MAX),
                    file: file_path(graph, id),
                    span: node.span,
                    review: review.and_then(|r| node_review(r.diff, id)),
                    debt: debt.and_then(|d| d.get(id).copied()),
                },
            );
            if expanded.contains(id) {
                queue.extend(children.iter().map(|child| (child, Some(id))));
            }
        }
        let visible: HashSet<NodeId> = nodes.keys().cloned().collect();
        let links = review.map(|r| review::new_links(r.reviewed, graph, r.diff, &visible));
        let edges = aggregate::aggregate(graph.parents(), graph.edges(), &visible)
            .into_iter()
            .map(ViewEdge::from)
            .map(|mut edge| {
                if let (Some(r), Some(links)) = (review, &links) {
                    let added = links.contains(&(edge.src.clone(), edge.dst.clone()));
                    let cycle = r
                        .diff
                        .cycle_of(&edge.src)
                        .is_some_and(|cycle| cycle.contains(&edge.dst));
                    edge.review = (added || cycle).then_some(EdgeReview { added, cycle });
                }
                (edge.key(), edge)
            })
            .collect();
        Self {
            nodes,
            edges,
            review: review.map_or_else(ReviewSummary::default, |r| r.summary.clone()),
        }
    }

    /// The review summary the view shows.
    pub fn review(&self) -> &ReviewSummary {
        &self.review
    }

    pub fn snapshot(&self, subscription: u64, revision: u64) -> ViewSnapshot {
        ViewSnapshot {
            subscription,
            revision,
            nodes: self.nodes.values().cloned().collect(),
            edges: self.edges.values().cloned().collect(),
            review: self.review.clone(),
        }
    }

    /// From `self` to `next`, as the delta from `revision` to `revision + 1`.
    pub fn delta(&self, next: &Self, subscription: u64, revision: u64) -> ViewDelta {
        ViewDelta {
            subscription,
            base: revision,
            revision: revision + 1,
            nodes: changed(&self.nodes, &next.nodes),
            removed_nodes: removed(&self.nodes, &next.nodes),
            edges: changed(&self.edges, &next.edges),
            removed_edges: removed(&self.edges, &next.edges),
            review: (self.review != next.review).then(|| next.review.clone()),
        }
    }

    /// The state a client reaches by applying `delta` to `self`: what a client does.
    pub fn apply(&mut self, delta: &ViewDelta) {
        for id in &delta.removed_nodes {
            self.nodes.remove(id);
        }
        for edge in &delta.removed_edges {
            self.edges.remove(edge);
        }
        for node in &delta.nodes {
            self.nodes.insert(node.id.clone(), node.clone());
        }
        for edge in &delta.edges {
            self.edges.insert(edge.key(), edge.clone());
        }
        if let Some(review) = &delta.review {
            self.review = review.clone();
        }
    }

    pub fn from_snapshot(snapshot: &ViewSnapshot) -> Self {
        Self {
            nodes: snapshot
                .nodes
                .iter()
                .map(|n| (n.id.clone(), n.clone()))
                .collect(),
            edges: snapshot
                .edges
                .iter()
                .map(|e| (e.key(), e.clone()))
                .collect(),
            review: snapshot.review.clone(),
        }
    }

    pub fn nodes(&self) -> impl Iterator<Item = &ViewNode> {
        self.nodes.values()
    }

    pub fn edges(&self) -> impl Iterator<Item = &ViewEdge> {
        self.edges.values()
    }
}

fn changed<K: Ord, V: Clone + PartialEq>(
    before: &BTreeMap<K, V>,
    after: &BTreeMap<K, V>,
) -> Vec<V> {
    after
        .iter()
        .filter(|(key, value)| before.get(*key) != Some(*value))
        .map(|(_, value)| value.clone())
        .collect()
}

fn removed<K: Ord + Clone, V>(before: &BTreeMap<K, V>, after: &BTreeMap<K, V>) -> Vec<K> {
    before
        .keys()
        .filter(|key| !after.contains_key(*key))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use timonier_core::{
        Confidence, Edge, EdgeKind, Language, Node, Position, Provenance, Span, SymbolKind,
    };

    use super::*;

    fn id(s: &str) -> NodeId {
        NodeId::parse(s).unwrap()
    }

    fn node(i: &str, kind: NodeKind, parent: Option<&str>) -> Node {
        let mut node = Node::new(id(i), kind, i.rsplit([':', '/']).next().unwrap());
        if let Some(parent) = parent {
            node = node.with_parent(id(parent));
        }
        node
    }

    fn call(src: &str, dst: &str, provenance: Provenance) -> Edge {
        Edge {
            src: id(src),
            dst: id(dst),
            kind: EdgeKind::Calls,
            provenance,
            confidence: Confidence::CERTAIN,
            weight: 1,
        }
    }

    /// repo → packages a and b → their files → one function each; a calls b.
    fn graph(extra: &[Node]) -> Graph {
        let span = Span::new(Position::new(0, 0), Position::new(0, 9)).unwrap();
        let function = NodeKind::Symbol(SymbolKind::Function);
        let mut nodes = vec![
            node("repo:r", NodeKind::Repo, None),
            node("pkg:cargo/a", NodeKind::Package, Some("repo:r")),
            node("pkg:cargo/b", NodeKind::Package, Some("repo:r")),
            node("file:a/src/lib.rs", NodeKind::File, Some("pkg:cargo/a"))
                .with_language(Language::new("rust").unwrap()),
            node("file:b/src/lib.rs", NodeKind::File, Some("pkg:cargo/b")),
            node("rust:a::f", function, Some("file:a/src/lib.rs")).with_span(span),
            node("rust:b::g", function, Some("file:b/src/lib.rs")),
        ];
        nodes.extend_from_slice(extra);
        let edges = vec![
            call("rust:a::f", "rust:b::g", Provenance::Lsp),
            call(
                "file:a/src/lib.rs",
                "file:b/src/lib.rs",
                Provenance::TreeSitter,
            ),
            // Dangling: dropped.
            call("rust:a::f", "rust:gone::x", Provenance::TreeSitter),
        ];
        Graph::new(nodes, edges)
    }

    fn spec(expanded: &[&str]) -> ViewSpec {
        ViewSpec {
            expanded: expanded.iter().map(|i| id(i)).collect(),
        }
    }

    fn ids(state: &ViewState) -> Vec<&str> {
        state.nodes().map(|n| n.id.as_str()).collect()
    }

    #[test]
    fn the_top_is_what_lies_under_the_repository() {
        let state = ViewState::compute(&graph(&[]), &spec(&[]), None);
        assert_eq!(ids(&state), ["pkg:cargo/a", "pkg:cargo/b"]);
        let a = state.nodes().next().unwrap();
        assert_eq!(
            (a.parent.as_ref(), a.children, a.file.as_ref()),
            (None, 1, None)
        );
        let edges: Vec<_> = state.edges().collect();
        assert_eq!(edges.len(), 1);
        let edge = edges[0];
        assert_eq!(
            (edge.src.as_str(), edge.dst.as_str()),
            ("pkg:cargo/a", "pkg:cargo/b")
        );
        assert_eq!(
            (edge.edges, edge.best, edge.approximate_only),
            (2, Provenance::Lsp, false)
        );
    }

    #[test]
    fn expanded_nodes_show_their_children_when_shown_themselves() {
        let graph = graph(&[]);
        // A file expanded under a collapsed package stays hidden.
        let state = ViewState::compute(&graph, &spec(&["file:a/src/lib.rs"]), None);
        assert_eq!(ids(&state), ["pkg:cargo/a", "pkg:cargo/b"]);
        let state = ViewState::compute(&graph, &spec(&["pkg:cargo/a", "file:a/src/lib.rs"]), None);
        assert_eq!(
            ids(&state),
            [
                "file:a/src/lib.rs",
                "pkg:cargo/a",
                "pkg:cargo/b",
                "rust:a::f"
            ]
        );
        let f = state
            .nodes()
            .find(|n| n.id.as_str() == "rust:a::f")
            .unwrap();
        assert_eq!(
            f.parent.as_ref().map(NodeId::as_str),
            Some("file:a/src/lib.rs")
        );
        assert_eq!(f.file.as_deref(), Some("a/src/lib.rs"));
        assert!(f.span.is_some());
        // The call now leaves from the function; the file-level edge from its file.
        let mut pairs: Vec<(&str, &str)> = state
            .edges()
            .map(|e| (e.src.as_str(), e.dst.as_str()))
            .collect();
        pairs.sort();
        assert_eq!(
            pairs,
            [
                ("file:a/src/lib.rs", "pkg:cargo/b"),
                ("rust:a::f", "pkg:cargo/b")
            ]
        );
    }

    #[test]
    fn a_delta_brings_the_old_state_to_the_new_one() {
        let before = ViewState::compute(&graph(&[]), &spec(&["pkg:cargo/a"]), None);
        let added = node("file:a/src/new.rs", NodeKind::File, Some("pkg:cargo/a"));
        let mut renamed = node("pkg:cargo/b", NodeKind::Package, Some("repo:r"));
        renamed.name = "bee".into();
        let mut nodes: Vec<Node> = graph(&[added]).nodes().cloned().collect();
        nodes.retain(|n| n.id.as_str() != "pkg:cargo/b");
        nodes.push(renamed);
        let after_graph = Graph::new(
            nodes,
            vec![call("rust:a::f", "rust:b::g", Provenance::TreeSitter)],
        );
        let after = ViewState::compute(&after_graph, &spec(&["pkg:cargo/a"]), None);
        let delta = before.delta(&after, 7, 3);
        assert_eq!((delta.subscription, delta.base, delta.revision), (7, 3, 4));
        let names: Vec<&str> = delta.nodes.iter().map(|n| n.name.as_str()).collect();
        // The new file, the package that gained a child, the renamed package.
        assert_eq!(names, ["new.rs", "a", "bee"]);
        assert!(delta.removed_nodes.is_empty());
        // The edge kept its key but changed: approximate now.
        assert_eq!(delta.edges.len(), 1);
        assert!(delta.edges[0].approximate_only);
        let mut client = before.clone();
        client.apply(&delta);
        assert_eq!(client, after);
        assert!(after.delta(&after, 7, 4).is_empty());
        // Back: the file is removed.
        let back = after.delta(&before, 7, 4);
        assert_eq!(back.removed_nodes, [id("file:a/src/new.rs")]);
        let mut client = after.clone();
        client.apply(&back);
        assert_eq!(client, before);
    }

    /// ADR 0024: what the map shows of the review diff, on the nodes, on the edges, and in the
    /// summary; a delta carries the summary when it changes.
    #[test]
    fn a_view_shows_the_review_diff() {
        use std::collections::HashMap;

        use timonier_core::ContentHash;

        let mut reviewed_nodes: Vec<Node> = graph(&[]).nodes().cloned().collect();
        reviewed_nodes.push(node("pkg:cargo/gone", NodeKind::Package, Some("repo:r")));
        let reviewed = Graph::new(reviewed_nodes, Vec::new());
        let head = graph(&[node("pkg:cargo/c", NodeKind::Package, Some("repo:r"))]);
        let hash = |n: u8| ContentHash::from_bytes([n; 32]);
        let contents = |a: u8| {
            HashMap::from([
                ("file:a/src/lib.rs".to_owned(), hash(a)),
                ("file:b/src/lib.rs".to_owned(), hash(9)),
            ])
        };
        // a's file was written since the review: its calls to b are new.
        let diff = review::compare(&reviewed, &head, &contents(1), &contents(2));
        let mark = ReviewMark {
            seq: 3,
            timestamp_ms: 1_000,
        };
        let summary = summary(mark, &reviewed, &head, &diff);
        let review = Review {
            reviewed: &reviewed,
            diff: &diff,
            summary: &summary,
        };
        let state = ViewState::compute(&head, &spec(&[]), Some(&review));
        let change = |i: &str| {
            state
                .nodes()
                .find(|n| n.id.as_str() == i)
                .and_then(|n| n.review)
                .and_then(|r| r.change)
        };
        assert_eq!(change("pkg:cargo/a"), Some(ReviewChange::Modified));
        assert_eq!(change("pkg:cargo/b"), None);
        assert_eq!(change("pkg:cargo/c"), Some(ReviewChange::Added));
        let edge = state.edges().next().unwrap();
        assert_eq!(
            (edge.src.as_str(), edge.dst.as_str(), edge.review),
            (
                "pkg:cargo/a",
                "pkg:cargo/b",
                Some(EdgeReview {
                    added: true,
                    cycle: false
                })
            )
        );
        let names = |boxes: &[ReviewBox]| boxes.iter().map(|b| b.name.clone()).collect::<Vec<_>>();
        let summary = state.review();
        assert_eq!(summary.since, Some(mark));
        assert_eq!(names(&summary.added), ["c"]);
        assert_eq!(names(&summary.modified), ["a"]);
        assert_eq!(names(&summary.removed), ["gone"]);
        let dependencies: Vec<(&str, &str)> = summary
            .dependencies_added
            .iter()
            .map(|d| (d.src.name.as_str(), d.dst.name.as_str()))
            .collect();
        assert_eq!(dependencies, [("a", "b")]);
        // Without a review, nothing (Q27); the delta brings the summary.
        let without = ViewState::compute(&head, &spec(&[]), None);
        assert!(without.nodes().all(|n| n.review.is_none()));
        assert_eq!(without.review(), &ReviewSummary::default());
        let delta = without.delta(&state, 1, 0);
        assert_eq!(delta.review.as_ref(), Some(state.review()));
        let mut client = without.clone();
        client.apply(&delta);
        assert_eq!(client, state);
        assert!(state.delta(&state, 1, 1).is_empty());
    }

    /// Review n° 1 of Phase 5, I3: a cycle added shows on its nodes, on its edges, and in the
    /// summary, with the jumps and the dependencies removed.
    #[test]
    fn a_view_shows_the_cycles_the_jumps_and_what_went() {
        use std::collections::HashMap;

        use timonier_core::ContentHash;

        let packages = ["a", "b", "c", "d", "e", "f"];
        let nodes = || {
            let mut nodes = vec![node("repo:r", NodeKind::Repo, None)];
            for p in packages {
                nodes.push(node(
                    &format!("pkg:cargo/{p}"),
                    NodeKind::Package,
                    Some("repo:r"),
                ));
                nodes.push(node(
                    &format!("file:{p}/src/lib.rs"),
                    NodeKind::File,
                    Some(&format!("pkg:cargo/{p}")),
                ));
            }
            nodes
        };
        let file = |p: &str| format!("file:{p}/src/lib.rs");
        let link = |a: &str, b: &str| call(&file(a), &file(b), Provenance::TreeSitter);
        let reviewed = Graph::new(nodes(), vec![link("a", "b"), link("a", "c")]);
        // b calls a back, d, e and f call b; a no longer calls c.
        let head = Graph::new(
            nodes(),
            vec![
                link("a", "b"),
                link("b", "a"),
                link("d", "b"),
                link("e", "b"),
                link("f", "b"),
            ],
        );
        let at = |changed: &[&str]| -> HashMap<String, ContentHash> {
            packages
                .iter()
                .map(|p| {
                    let byte = u8::from(changed.contains(p));
                    (file(p), ContentHash::from_bytes([byte; 32]))
                })
                .collect()
        };
        let diff = review::compare(&reviewed, &head, &at(&[]), &at(&["a", "b", "d", "e", "f"]));
        let mark = ReviewMark {
            seq: 1,
            timestamp_ms: 0,
        };
        let summary = summary(mark, &reviewed, &head, &diff);
        let review = Review {
            reviewed: &reviewed,
            diff: &diff,
            summary: &summary,
        };
        let state = ViewState::compute(&head, &spec(&[]), Some(&review));
        let review_of = |i: &str| {
            state
                .nodes()
                .find(|n| n.id.as_str() == i)
                .and_then(|n| n.review)
        };
        assert!(review_of("pkg:cargo/a").is_some_and(|r| r.cycle));
        let b = review_of("pkg:cargo/b").expect("b changed");
        assert!(b.cycle);
        assert_eq!(
            b.fan_in,
            Some(FanJump {
                before: 1,
                after: 4
            })
        );
        assert!(review_of("pkg:cargo/c").is_none_or(|r| !r.cycle));
        let edge = |src: &str, dst: &str| {
            state
                .edges()
                .find(|e| e.src.as_str() == src && e.dst.as_str() == dst)
                .and_then(|e| e.review)
        };
        assert_eq!(
            edge("pkg:cargo/b", "pkg:cargo/a"),
            Some(EdgeReview {
                added: true,
                cycle: true
            })
        );
        assert_eq!(
            edge("pkg:cargo/a", "pkg:cargo/b"),
            Some(EdgeReview {
                added: false,
                cycle: true
            })
        );
        let summary = state.review();
        let cycles: Vec<Vec<String>> = summary
            .cycles
            .iter()
            .map(|cycle| cycle.iter().map(|b| b.name.clone()).collect())
            .collect();
        assert_eq!(cycles, [["a", "b"]]);
        assert_eq!(summary.fans.len(), 1);
        assert_eq!(summary.fans[0].item.name, "b");
        assert_eq!(
            summary.fans[0].fan_in,
            Some(FanJump {
                before: 1,
                after: 4
            })
        );
        let removed: Vec<(&str, &str)> = summary
            .dependencies_removed
            .iter()
            .map(|d| (d.src.name.as_str(), d.dst.name.as_str()))
            .collect();
        assert_eq!(removed, [("a", "c")]);
    }

    /// Review n° 2 of Phase 5, m-4 (Q34): a crate whose manifest went is modified, and the
    /// dependency it lost is named as the review had it.
    #[test]
    fn a_crate_without_its_manifest_is_modified_and_named_as_it_was() {
        use std::collections::HashMap;

        use timonier_core::ContentHash;

        let depends = Edge {
            kind: EdgeKind::DependsOn,
            ..call("pkg:cargo/a", "pkg:cargo/c", Provenance::Manifest)
        };
        let reviewed = Graph::new(
            vec![
                node("repo:r", NodeKind::Repo, None),
                node("pkg:cargo/a", NodeKind::Package, Some("repo:r")),
                node("file:a/src/lib.rs", NodeKind::File, Some("pkg:cargo/a")),
                node("pkg:cargo/c", NodeKind::Package, Some("repo:r")),
                node("file:c/src/lib.rs", NodeKind::File, Some("pkg:cargo/c")),
            ],
            vec![depends.clone()],
        );
        // c/Cargo.toml deleted: c is an external dependency of a, its file left under the repo.
        let head = Graph::new(
            vec![
                node("repo:r", NodeKind::Repo, None),
                node("pkg:cargo/a", NodeKind::Package, Some("repo:r")),
                node("file:a/src/lib.rs", NodeKind::File, Some("pkg:cargo/a")),
                node(
                    "component:external/other",
                    NodeKind::Component,
                    Some("repo:r"),
                ),
                node(
                    "pkg:cargo/c",
                    NodeKind::Package,
                    Some("component:external/other"),
                ),
                node("file:c/src/lib.rs", NodeKind::File, Some("repo:r")),
            ],
            vec![depends],
        );
        let contents: HashMap<String, ContentHash> = ["file:a/src/lib.rs", "file:c/src/lib.rs"]
            .into_iter()
            .map(|f| (f.to_owned(), ContentHash::from_bytes([0; 32])))
            .collect();
        let diff = review::compare(&reviewed, &head, &contents, &contents);
        let mark = ReviewMark {
            seq: 1,
            timestamp_ms: 0,
        };
        let summary = summary(mark, &reviewed, &head, &diff);
        let names = |boxes: &[ReviewBox]| boxes.iter().map(|b| b.name.clone()).collect::<Vec<_>>();
        assert!(
            names(&summary.modified).contains(&"c".to_owned()),
            "{summary:?}"
        );
        let removed: Vec<(&str, &str, Option<&str>)> = summary
            .dependencies_removed
            .iter()
            .map(|d| {
                (
                    d.src.name.as_str(),
                    d.dst.name.as_str(),
                    d.dst.within.as_deref(),
                )
            })
            .collect();
        assert_eq!(removed, [("a", "c", None)]);
    }

    #[test]
    fn a_snapshot_gives_back_the_state() {
        let state = ViewState::compute(&graph(&[]), &spec(&["pkg:cargo/a", "pkg:cargo/b"]), None);
        let snapshot = state.snapshot(1, 5);
        assert_eq!((snapshot.subscription, snapshot.revision), (1, 5));
        assert_eq!(ViewState::from_snapshot(&snapshot), state);
    }
}
