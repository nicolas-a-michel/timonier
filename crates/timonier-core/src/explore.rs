//! The graph for the agents (PLAN.md 11.10, ADR 0076): `get_component` and `query_graph`, the
//! same model the developer's map shows, names and summaries given by hand included. Pure: the
//! daemon gives the graph.

use std::collections::{BTreeMap, HashSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::aggregate::{AggregatedEdge, aggregate};
use crate::graph::Graph;
use crate::{EdgeKind, NodeId, NodeKind, Role};

/// At most this many children, files or nodes in a page.
pub const MAX_LIST: usize = 200;

/// `get_component`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetComponent {
    /// A box of the map, as the tools give it: `component:dir/crates/core`, `pkg:cargo/core`.
    #[schemars(length(min = 1, max = 512))]
    pub component: String,
}

/// `query_graph`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryGraph {
    /// Boxes opened on what they hold, as the developer opens them on the map; none: the top.
    #[serde(default)]
    #[schemars(length(max = 50), inner(length(min = 1, max = 512)))]
    pub expanded: Vec<String>,
    /// How many nodes the page lists (200 at most, and by default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1, max = 200))]
    pub limit: Option<u32>,
    /// Where the previous page ended: its `next_cursor`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 16))]
    pub cursor: Option<String>,
}

/// A box or a file in a list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Brief {
    pub id: String,
    pub kind: NodeKind,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// What it holds, opened.
    pub children: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

/// A link between two boxes, standing for the stored links between their contents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
    pub links: u32,
    /// Of `links`, those a manifest, a language server or SCIP confirmed (PLAN.md 11.5).
    pub confirmed: u32,
}

/// What `get_component` returns.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Component {
    #[serde(flatten)]
    pub brief: Brief,
    pub children: Vec<Brief>,
    /// Its files, relative to the root, sorted; `files_total` says how many there are.
    pub files: Vec<String>,
    pub files_total: usize,
    /// Its links to the boxes around it, at each level of the map: the boxes beside it and
    /// beside each box that holds it.
    pub uses: Vec<Link>,
    pub used_by: Vec<Link>,
}

/// What `query_graph` returns: a page of the map's nodes, and the links that leave them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphPage {
    pub nodes: Vec<Brief>,
    pub links: Vec<Link>,
    pub total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

fn brief(graph: &Graph, id: &NodeId) -> Option<Brief> {
    let node = graph.node(id)?;
    Some(Brief {
        id: id.to_string(),
        kind: node.kind,
        name: node.name.clone(),
        role: node.role,
        summary: graph.summary(id).cloned(),
        children: u32::try_from(graph.children(id).len()).unwrap_or(u32::MAX),
        parent: node.parent.as_ref().map(ToString::to_string),
    })
}

fn link(edge: &AggregatedEdge) -> Link {
    Link {
        from: edge.src.to_string(),
        to: edge.dst.to_string(),
        kind: edge.kind,
        links: edge.edges,
        confirmed: edge.confirmed,
    }
}

/// The map's top: what lies under the repository.
fn top(graph: &Graph) -> Vec<NodeId> {
    graph
        .roots()
        .iter()
        .flat_map(|root| match graph.node(root).map(|n| n.kind) {
            Some(NodeKind::Repo) => graph.children(root).to_vec(),
            _ => vec![root.clone()],
        })
        .collect()
}

/// A box of the graph: `None` when the id names none, or names a file or a symbol.
pub fn component(graph: &Graph, id: &str) -> Option<Component> {
    let id = NodeId::parse(id.to_owned()).ok()?;
    let node = graph.node(&id)?;
    if !matches!(node.kind, NodeKind::Package | NodeKind::Component) {
        return None;
    }
    let mut files = Vec::new();
    let mut pending = vec![id.clone()];
    while let Some(at) = pending.pop() {
        for child in graph.children(&at) {
            match graph.node(child).map(|n| n.kind) {
                Some(NodeKind::File) => {
                    if let Some(crate::IdForm::File { path }) = Some(child.form()) {
                        files.push(path.to_owned());
                    }
                }
                Some(NodeKind::Package | NodeKind::Component) => pending.push(child.clone()),
                _ => {}
            }
        }
    }
    files.sort();
    let files_total = files.len();
    files.truncate(MAX_LIST);
    // The boxes beside it, and beside each box that holds it: the map's levels around it.
    let mut visible: HashSet<NodeId> = top(graph).into_iter().collect();
    // `ancestors` starts with the box itself: what holds it, from there up.
    let holders: Vec<&NodeId> = graph.ancestors(&id).filter(|a| **a != id).collect();
    for holder in &holders {
        visible.extend(graph.children(holder).iter().cloned());
    }
    for holder in &holders {
        visible.remove(*holder);
    }
    visible.insert(id.clone());
    let edges = aggregate(graph.parents(), graph.edges(), &visible);
    let uses = edges.iter().filter(|e| e.src == id).map(link).collect();
    let used_by = edges.iter().filter(|e| e.dst == id).map(link).collect();
    let children = graph
        .children(&id)
        .iter()
        .take(MAX_LIST)
        .filter_map(|c| brief(graph, c))
        .collect();
    Some(Component {
        brief: brief(graph, &id)?,
        children,
        files,
        files_total,
        uses,
        used_by,
    })
}

/// The map as the developer sees it with `expanded` open: the top, and what each open box holds.
pub fn query(graph: &Graph, input: &QueryGraph) -> Result<GraphPage, String> {
    let expanded: HashSet<NodeId> = input
        .expanded
        .iter()
        .map(|id| NodeId::parse(id.clone()).map_err(|_| format!("`{id}` is no id of the graph")))
        .collect::<Result<_, _>>()?;
    let mut shown: Vec<NodeId> = Vec::new();
    let mut pending = top(graph);
    pending.reverse();
    while let Some(id) = pending.pop() {
        if expanded.contains(&id) {
            let mut children = graph.children(&id).to_vec();
            children.reverse();
            pending.extend(children);
        }
        shown.push(id);
    }
    let visible: HashSet<NodeId> = shown.iter().cloned().collect();
    let start = match &input.cursor {
        Some(c) => c
            .parse::<usize>()
            .map_err(|_| "cursor is the next_cursor a page gave".to_owned())?,
        None => 0,
    };
    let limit = input
        .limit
        .map_or(MAX_LIST, |l| (l as usize).clamp(1, MAX_LIST));
    let page: Vec<&NodeId> = shown.iter().skip(start).take(limit).collect();
    let in_page: HashSet<&NodeId> = page.iter().copied().collect();
    let mut by_source: BTreeMap<(String, String, EdgeKind), Link> = BTreeMap::new();
    for edge in aggregate(graph.parents(), graph.edges(), &visible) {
        if in_page.contains(&edge.src) {
            let l = link(&edge);
            by_source.insert((l.from.clone(), l.to.clone(), l.kind), l);
        }
    }
    let end = start + page.len();
    Ok(GraphPage {
        nodes: page.into_iter().filter_map(|id| brief(graph, id)).collect(),
        links: by_source.into_values().collect(),
        total: shown.len(),
        next_cursor: (end < shown.len()).then(|| end.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Edge, Node, Provenance};

    fn id(s: &str) -> NodeId {
        NodeId::parse(s).unwrap()
    }

    fn graph() -> Graph {
        let node = |i: &str, kind, name: &str, parent: Option<&str>| {
            let n = Node::new(id(i), kind, name);
            match parent {
                Some(p) => n.with_parent(id(p)),
                None => n,
            }
        };
        let nodes = vec![
            node("repo:r", NodeKind::Repo, "r", None),
            node("pkg:cargo/app", NodeKind::Package, "app", Some("repo:r"))
                .with_role(Role::EntryPoint),
            node(
                "pkg:cargo/store",
                NodeKind::Package,
                "store",
                Some("repo:r"),
            )
            .with_role(Role::Persistence),
            node(
                "component:dir/app/ui",
                NodeKind::Component,
                "ui",
                Some("pkg:cargo/app"),
            ),
            node(
                "file:app/main.rs",
                NodeKind::File,
                "main.rs",
                Some("pkg:cargo/app"),
            ),
            node(
                "file:app/ui/view.rs",
                NodeKind::File,
                "view.rs",
                Some("component:dir/app/ui"),
            ),
            node(
                "file:store/lib.rs",
                NodeKind::File,
                "lib.rs",
                Some("pkg:cargo/store"),
            ),
        ];
        let edge = |a: &str, b: &str, provenance| Edge {
            src: id(a),
            dst: id(b),
            kind: EdgeKind::Calls,
            provenance,
            confidence: crate::Confidence::CERTAIN,
            weight: 1,
        };
        let edges = vec![
            edge("file:app/main.rs", "file:store/lib.rs", Provenance::Lsp),
            edge(
                "file:app/ui/view.rs",
                "file:store/lib.rs",
                Provenance::TreeSitter,
            ),
            edge(
                "file:app/main.rs",
                "file:app/ui/view.rs",
                Provenance::TreeSitter,
            ),
        ];
        Graph::new(nodes, edges)
            .with_summaries([("pkg:cargo/store".to_owned(), "Keeps the orders.".to_owned())])
    }

    #[test]
    fn a_component_says_what_it_holds_and_what_it_links_to() {
        let graph = graph();
        let ui = component(&graph, "component:dir/app/ui").unwrap();
        assert_eq!(ui.files, ["app/ui/view.rs"]);
        // At the levels around it: the store beside its package, the main file beside it.
        let uses: Vec<(&str, u32, u32)> = ui
            .uses
            .iter()
            .map(|l| (l.to.as_str(), l.links, l.confirmed))
            .collect();
        assert_eq!(uses, [("pkg:cargo/store", 1, 0)]);
        assert_eq!(ui.used_by[0].from, "file:app/main.rs");
        let app = component(&graph, "pkg:cargo/app").unwrap();
        assert_eq!(app.files, ["app/main.rs", "app/ui/view.rs"]);
        assert_eq!((app.uses[0].links, app.uses[0].confirmed), (2, 1));
        let store = component(&graph, "pkg:cargo/store").unwrap();
        assert_eq!(store.brief.summary.as_deref(), Some("Keeps the orders."));
        assert!(component(&graph, "file:app/main.rs").is_none());
        assert!(component(&graph, "pkg:cargo/none").is_none());
    }

    #[test]
    fn the_graph_reads_as_the_map_shows_it_page_by_page() {
        let graph = graph();
        let top = query(&graph, &QueryGraph::default()).unwrap();
        let names: Vec<&str> = top.nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, ["app", "store"]);
        assert_eq!(top.links.len(), 1);
        let opened = query(
            &graph,
            &QueryGraph {
                expanded: vec!["pkg:cargo/app".into()],
                limit: Some(2),
                cursor: None,
            },
        )
        .unwrap();
        assert_eq!(opened.total, 4);
        assert_eq!(opened.next_cursor.as_deref(), Some("2"));
        let next = query(
            &graph,
            &QueryGraph {
                expanded: vec!["pkg:cargo/app".into()],
                limit: Some(2),
                cursor: opened.next_cursor.clone(),
            },
        )
        .unwrap();
        assert_eq!(next.next_cursor, None);
        let all: usize = opened.nodes.len() + next.nodes.len();
        assert_eq!(all, 4);
        assert!(
            query(
                &graph,
                &QueryGraph {
                    expanded: vec!["nope".into()],
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
}
