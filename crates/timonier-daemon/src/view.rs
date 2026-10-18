//! Views of the map (ADR 0019): what a subscription shows of the head, and what changed in it
//! since the client's last revision. Pure, apart from [`Graph::load`].

use std::collections::{BTreeMap, HashMap, HashSet};

use timonier_core::protocol::{EdgeRef, ViewDelta, ViewEdge, ViewNode, ViewSnapshot, ViewSpec};
use timonier_core::{Edge, IdForm, Node, NodeId, NodeKind, aggregate};
use timonier_store::Reader;

use crate::Result;

/// The head, read once for every view (PLAN.md, Phase 2: aggregation reads the whole graph).
#[derive(Debug, Default)]
pub struct Graph {
    nodes: HashMap<NodeId, Node>,
    /// Children of each node, sorted by id.
    children: HashMap<NodeId, Vec<NodeId>>,
    parents: HashMap<NodeId, Option<NodeId>>,
    edges: Vec<Edge>,
    roots: Vec<NodeId>,
}

impl Graph {
    pub fn load(reader: &mut Reader) -> Result<Self> {
        let head = reader.head()?.seq;
        Ok(Self::new(reader.nodes_at(head)?, reader.edges_at(head)?))
    }

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

    fn children(&self, id: &NodeId) -> &[NodeId] {
        self.children.get(id).map_or(&[], Vec::as_slice)
    }

    /// The file that holds a node: itself, or the nearest file above a symbol.
    fn file_of(&self, id: &NodeId) -> Option<String> {
        let mut current = id;
        loop {
            let node = self.nodes.get(current)?;
            if let IdForm::File { path } = node.id.form() {
                return Some(path.to_owned());
            }
            if !node.kind.is_symbol() {
                return None;
            }
            current = node.parent.as_ref()?;
        }
    }
}

/// What a view shows at one revision.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewState {
    nodes: BTreeMap<NodeId, ViewNode>,
    edges: BTreeMap<EdgeRef, ViewEdge>,
}

impl ViewState {
    /// The top of the map is what lies under the repository; each expanded node that is shown
    /// shows its children too. An expanded node under a collapsed one stays hidden.
    pub fn compute(graph: &Graph, spec: &ViewSpec) -> Self {
        let expanded: HashSet<&NodeId> = spec.expanded.iter().collect();
        let mut top: Vec<&NodeId> = Vec::new();
        for root in &graph.roots {
            match graph.nodes.get(root).map(|n| n.kind) {
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
            let Some(node) = graph.nodes.get(id) else {
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
                    file: graph.file_of(id),
                    span: node.span,
                },
            );
            if expanded.contains(id) {
                queue.extend(children.iter().map(|child| (child, Some(id))));
            }
        }
        let visible: HashSet<NodeId> = nodes.keys().cloned().collect();
        let edges = aggregate::aggregate(&graph.parents, &graph.edges, &visible)
            .into_iter()
            .map(ViewEdge::from)
            .map(|edge| (edge.key(), edge))
            .collect();
        Self { nodes, edges }
    }

    pub fn snapshot(&self, subscription: u64, revision: u64) -> ViewSnapshot {
        ViewSnapshot {
            subscription,
            revision,
            nodes: self.nodes.values().cloned().collect(),
            edges: self.edges.values().cloned().collect(),
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
    use timonier_core::{Confidence, EdgeKind, Language, Position, Provenance, Span, SymbolKind};

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
        let state = ViewState::compute(&graph(&[]), &spec(&[]));
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
        let state = ViewState::compute(&graph, &spec(&["file:a/src/lib.rs"]));
        assert_eq!(ids(&state), ["pkg:cargo/a", "pkg:cargo/b"]);
        let state = ViewState::compute(&graph, &spec(&["pkg:cargo/a", "file:a/src/lib.rs"]));
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
        let before = ViewState::compute(&graph(&[]), &spec(&["pkg:cargo/a"]));
        let added = node("file:a/src/new.rs", NodeKind::File, Some("pkg:cargo/a"));
        let mut renamed = node("pkg:cargo/b", NodeKind::Package, Some("repo:r"));
        renamed.name = "bee".into();
        let mut nodes: Vec<Node> = graph(&[added]).nodes.into_values().collect();
        nodes.retain(|n| n.id.as_str() != "pkg:cargo/b");
        nodes.push(renamed);
        let after_graph = Graph::new(
            nodes,
            vec![call("rust:a::f", "rust:b::g", Provenance::TreeSitter)],
        );
        let after = ViewState::compute(&after_graph, &spec(&["pkg:cargo/a"]));
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

    #[test]
    fn a_snapshot_gives_back_the_state() {
        let state = ViewState::compute(&graph(&[]), &spec(&["pkg:cargo/a", "pkg:cargo/b"]));
        let snapshot = state.snapshot(1, 5);
        assert_eq!((snapshot.subscription, snapshot.revision), (1, 5));
        assert_eq!(ViewState::from_snapshot(&snapshot), state);
    }
}
