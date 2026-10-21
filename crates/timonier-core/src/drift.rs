//! What the code does against what the developer drew (spec « Comparer à l'intention », ADR
//! 0041): each drawn component anchored to a real box by its name and its place (Q68), the
//! drawn components the code lacks, the forbidden edges the code crosses and the required ones
//! it lacks. Computed from `intent.json` and the actual architecture, never written: the
//! whiteboard has a single author.

use std::collections::{BTreeMap, HashMap, HashSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::actual::{Actual, ActualDependency};
use crate::intent::{Intent, Policy};

/// A drawn component and the real box it stands for, if any.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct Anchor {
    /// The drawn component (`C-1`).
    pub component: String,
    pub name: String,
    /// The real box (`pkg:cargo/timonier-daemon`, `component:dir/src/net`); `null` when the
    /// code has none by that name.
    pub actual: Option<String>,
    /// How many real boxes had the name, the chosen one included.
    pub candidates: usize,
}

/// A dependency of the code an edge of the whiteboard is about.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct Crossing {
    /// The edge (`E-2`) and its ends as drawn.
    pub edge: String,
    pub src: String,
    pub dst: String,
    /// The real dependencies from what `src` stands for to what `dst` stands for.
    pub dependencies: Vec<ActualDependency>,
}

/// An edge the code cannot be checked against: an end has no real box.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct Unchecked {
    pub edge: String,
    /// The drawn ends that stand for nothing in the code.
    pub unanchored: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct Drift {
    /// Every drawn component, by id.
    pub anchors: Vec<Anchor>,
    /// The drawn components the code lacks.
    pub missing: Vec<String>,
    /// Forbidden edges the code crosses, with the dependencies that cross them.
    pub forbidden: Vec<Crossing>,
    /// Required edges with no dependency in the code.
    pub required: Vec<Crossing>,
    /// Forbidden or required edges an end of which stands for nothing.
    pub unchecked: Vec<Unchecked>,
}

/// A name as compared (Q68): case, spaces, dashes and underscores aside; accented and
/// non-Latin letters stay (review of Phase 7, m2).
pub fn normalize(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_whitespace() && *c != '-' && *c != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

/// The real boxes, with their parents, to find ancestors and distances.
struct Tree<'a> {
    parent: HashMap<&'a str, &'a str>,
}

impl<'a> Tree<'a> {
    fn new(actual: &'a Actual) -> Self {
        Self {
            parent: actual
                .boxes
                .iter()
                .filter_map(|b| Some((b.id.as_str(), b.parent.as_deref()?)))
                .collect(),
        }
    }

    /// The box and its ancestors, from the box up.
    fn line(&self, id: &'a str) -> Vec<&'a str> {
        let mut line = vec![id];
        let mut seen = HashSet::from([id]);
        while let Some(&up) = self.parent.get(line[line.len() - 1]) {
            if !seen.insert(up) {
                break;
            }
            line.push(up);
        }
        line
    }

    fn depth(&self, id: &'a str) -> usize {
        self.line(id).len() - 1
    }

    /// How many steps from one box to the other, through their closest common ancestor.
    fn distance(&self, a: &'a str, b: &'a str) -> usize {
        let up = self.line(a);
        let down = self.line(b);
        for (i, ancestor) in up.iter().enumerate() {
            if let Some(j) = down.iter().position(|d| d == ancestor) {
                return i + j;
            }
        }
        up.len() + down.len()
    }

    fn within(&self, id: &'a str, ancestor: &str) -> bool {
        self.line(id).contains(&ancestor)
    }
}

/// The real boxes a drawn name may stand for (Q68): the same name, letters and digits; failing
/// that, a package whose name ends with it (« Daemon » for `timonier-daemon`).
fn candidates<'a>(actual: &'a Actual, name: &str) -> Vec<&'a str> {
    let drawn = normalize(name);
    if drawn.is_empty() {
        return Vec::new();
    }
    let same: Vec<&str> = actual
        .boxes
        .iter()
        .filter(|b| normalize(&b.name) == drawn)
        .map(|b| b.id.as_str())
        .collect();
    if !same.is_empty() {
        return same;
    }
    actual
        .boxes
        .iter()
        .filter(|b| b.kind == "package" && normalize(&b.name).ends_with(&drawn))
        .map(|b| b.id.as_str())
        .collect()
}

/// Anchors every drawn component (Q68): one candidate, it; several, the one closest to the
/// components of the same layer anchored already, else the highest in the tree, then by id.
pub fn anchor(intent: &Intent, actual: &Actual) -> Vec<Anchor> {
    let tree = Tree::new(actual);
    let found: Vec<(usize, Vec<&str>)> = intent
        .components
        .iter()
        .enumerate()
        .map(|(i, c)| (i, candidates(actual, &c.name)))
        .collect();
    let mut chosen: BTreeMap<usize, &str> = found
        .iter()
        .filter(|(_, c)| c.len() == 1)
        .map(|(i, c)| (*i, c[0]))
        .collect();
    for (i, options) in found.iter().filter(|(_, c)| c.len() > 1) {
        let layer = &intent.components[*i].layer;
        let neighbours: Vec<&str> = chosen
            .iter()
            .filter(|(j, _)| layer.is_some() && intent.components[**j].layer == *layer)
            .map(|(_, id)| *id)
            .collect();
        let best = options.iter().min_by_key(|option| {
            let near: usize = neighbours.iter().map(|n| tree.distance(option, n)).sum();
            (near, tree.depth(option), **option)
        });
        if let Some(best) = best {
            chosen.insert(*i, best);
        }
    }
    intent
        .components
        .iter()
        .enumerate()
        .map(|(i, c)| Anchor {
            component: c.id.clone(),
            name: c.name.clone(),
            actual: chosen.get(&i).map(|id| (*id).to_owned()),
            candidates: found[i].1.len(),
        })
        .collect()
}

/// The comparison of the whiteboard with the code.
pub fn drift(intent: &Intent, actual: &Actual) -> Drift {
    let tree = Tree::new(actual);
    let anchors = anchor(intent, actual);
    let anchored: HashMap<String, String> = anchors
        .iter()
        .filter_map(|a| Some((a.component.clone(), a.actual.clone()?)))
        .collect();
    // What a drawn end stands for: a component, its box; a layer, the boxes of its components.
    let stands_for = |end: &str| -> Option<Vec<&str>> {
        if let Some(id) = anchored.get(end) {
            return Some(vec![id.as_str()]);
        }
        if intent.layers.iter().any(|l| l.id == end) {
            let boxes: Vec<&str> = intent
                .components
                .iter()
                .filter(|c| c.layer.as_deref() == Some(end))
                .filter_map(|c| anchored.get(&c.id).map(String::as_str))
                .collect();
            return (!boxes.is_empty()).then_some(boxes);
        }
        None
    };
    let unanchored = |end: &str| -> bool { stands_for(end).is_none() };
    let mut drift = Drift {
        missing: anchors
            .iter()
            .filter(|a| a.actual.is_none())
            .map(|a| a.component.clone())
            .collect(),
        anchors,
        ..Drift::default()
    };
    for edge in &intent.edges {
        if edge.policy == Policy::Allowed {
            continue;
        }
        let (Some(from), Some(to)) = (stands_for(&edge.src), stands_for(&edge.dst)) else {
            drift.unchecked.push(Unchecked {
                edge: edge.id.clone(),
                unanchored: [&edge.src, &edge.dst]
                    .into_iter()
                    .filter(|end| unanchored(end))
                    .cloned()
                    .collect(),
            });
            continue;
        };
        let inside = |id: &str, boxes: &[&str]| boxes.iter().any(|b| tree.within(id, b));
        // A dependency whose two ends are on both sides is inside, not across.
        let dependencies: Vec<ActualDependency> = actual
            .dependencies
            .iter()
            .filter(|d| {
                inside(&d.from, &from)
                    && inside(&d.to, &to)
                    && !(inside(&d.from, &to) && inside(&d.to, &from))
            })
            .cloned()
            .collect();
        let crossing = Crossing {
            edge: edge.id.clone(),
            src: edge.src.clone(),
            dst: edge.dst.clone(),
            dependencies,
        };
        match edge.policy {
            Policy::Forbidden if !crossing.dependencies.is_empty() => {
                drift.forbidden.push(crossing)
            }
            Policy::Required if crossing.dependencies.is_empty() => drift.required.push(crossing),
            _ => {}
        }
    }
    drift
}

/// The drift as the map shows it (ADR 0042): the drift, the names of what is drawn, and the line
/// of ancestors of every real box it cites, from the top, so that a box not drawn on the map is
/// shown on the one that holds it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct DriftView {
    /// Something is drawn on the whiteboard.
    pub drawn: bool,
    /// Why `intent.json` does not read, when it does not.
    pub problem: Option<String>,
    pub drift: Drift,
    /// The drawn layers and components, by id.
    #[ts(type = "Record<string, string>")]
    pub names: BTreeMap<String, String>,
    /// The ancestors of each real box the drift cites, from the top, the box excluded.
    #[ts(type = "Record<string, Array<string>>")]
    pub ancestors: BTreeMap<String, Vec<String>>,
}

/// The drift of the code from `intent`, with what the map needs to show it.
pub fn view(intent: &Intent, actual: &Actual) -> DriftView {
    let tree = Tree::new(actual);
    let drift = drift(intent, actual);
    let cited = drift
        .anchors
        .iter()
        .filter_map(|a| a.actual.as_deref())
        .chain(
            drift
                .forbidden
                .iter()
                .flat_map(|c| &c.dependencies)
                .flat_map(|d| [d.from.as_str(), d.to.as_str()]),
        );
    let mut ancestors = BTreeMap::new();
    for id in cited {
        let mut line: Vec<String> = tree
            .line(id)
            .iter()
            .skip(1)
            .map(|a| (*a).to_owned())
            .collect();
        line.reverse();
        ancestors.insert(id.to_owned(), line);
    }
    let names = intent
        .layers
        .iter()
        .map(|l| (l.id.clone(), l.name.clone()))
        .chain(
            intent
                .components
                .iter()
                .map(|c| (c.id.clone(), c.name.clone())),
        )
        .collect();
    DriftView {
        drawn: !intent.is_empty(),
        problem: None,
        drift,
        names,
        ancestors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actual::ActualBox;

    fn boxed(id: &str, kind: &str, name: &str, parent: Option<&str>) -> ActualBox {
        ActualBox {
            id: id.into(),
            kind: kind.into(),
            name: name.into(),
            role: None,
            parent: parent.map(Into::into),
        }
    }

    fn dependency(from: &str, to: &str) -> ActualDependency {
        ActualDependency {
            from: from.into(),
            to: to.into(),
            kinds: vec!["calls".into()],
            weight: 2,
        }
    }

    /// Two crates; the daemon has a `net` component, its `store` component reaches the store
    /// crate; the cli has a `net` component too.
    fn actual() -> Actual {
        Actual {
            version: 1,
            boxes: vec![
                boxed(
                    "pkg:cargo/timonier-daemon",
                    "package",
                    "timonier-daemon",
                    None,
                ),
                boxed(
                    "component:dir/daemon/net",
                    "component",
                    "net",
                    Some("pkg:cargo/timonier-daemon"),
                ),
                boxed(
                    "component:dir/daemon/db",
                    "component",
                    "db",
                    Some("pkg:cargo/timonier-daemon"),
                ),
                boxed(
                    "pkg:cargo/timonier-store",
                    "package",
                    "timonier-store",
                    None,
                ),
                boxed("pkg:cargo/timonier-cli", "package", "timonier-cli", None),
                boxed(
                    "component:dir/cli/net",
                    "component",
                    "net",
                    Some("pkg:cargo/timonier-cli"),
                ),
            ],
            dependencies: vec![
                dependency("component:dir/daemon/db", "pkg:cargo/timonier-store"),
                dependency("pkg:cargo/timonier-cli", "pkg:cargo/timonier-daemon"),
                dependency("component:dir/daemon/net", "component:dir/daemon/db"),
            ],
        }
    }

    fn intent(text: &str) -> Intent {
        crate::intent::parse(text).unwrap()
    }

    #[test]
    fn anchors_by_name_then_by_place() {
        let drawn = intent(
            r#"{"version": 1,
                "layers": [{"id": "L-1", "name": "Core"}],
                "components": [
                  {"id": "C-1", "name": "Daemon", "layer": "L-1", "role": ""},
                  {"id": "C-2", "name": "NET", "layer": "L-1", "role": ""},
                  {"id": "C-3", "name": "Timonier Store", "layer": null, "role": ""},
                  {"id": "C-4", "name": "Web", "layer": null, "role": ""},
                  {"id": "C-5", "name": "", "layer": null, "role": ""}
                ]}"#,
        );
        let anchors = anchor(&drawn, &actual());
        let said: Vec<_> = anchors
            .iter()
            .map(|a| (a.component.as_str(), a.actual.as_deref(), a.candidates))
            .collect();
        assert_eq!(
            said,
            [
                // A package whose name ends with the drawn one.
                ("C-1", Some("pkg:cargo/timonier-daemon"), 1),
                // Two `net`: the one near the daemon, drawn in the same layer.
                ("C-2", Some("component:dir/daemon/net"), 2),
                // Case, spaces and dashes do not count.
                ("C-3", Some("pkg:cargo/timonier-store"), 1),
                ("C-4", None, 0),
                ("C-5", None, 0),
            ]
        );
        // Alone in no layer, the highest, then the first by id.
        let alone = intent(
            r#"{"version": 1, "components": [{"id": "C-1", "name": "net", "layer": null, "role": ""}]}"#,
        );
        assert_eq!(
            anchor(&alone, &actual())[0].actual.as_deref(),
            Some("component:dir/cli/net")
        );
    }

    #[test]
    fn finds_forbidden_edges_crossed_and_required_ones_missing() {
        let drawn = intent(
            r#"{"version": 1,
                "layers": [{"id": "L-1", "name": "Core"}, {"id": "L-2", "name": "Storage"}],
                "components": [
                  {"id": "C-1", "name": "Daemon", "layer": "L-1", "role": ""},
                  {"id": "C-2", "name": "Store", "layer": "L-2", "role": ""},
                  {"id": "C-3", "name": "Cli", "layer": null, "role": ""},
                  {"id": "C-4", "name": "Web", "layer": null, "role": ""}
                ],
                "edges": [
                  {"id": "E-1", "src": "L-1", "dst": "L-2", "kind": "depends_on", "policy": "forbidden"},
                  {"id": "E-2", "src": "C-2", "dst": "C-1", "kind": "depends_on", "policy": "required"},
                  {"id": "E-3", "src": "C-3", "dst": "C-1", "kind": "depends_on", "policy": "required"},
                  {"id": "E-4", "src": "C-4", "dst": "C-1", "kind": "depends_on", "policy": "forbidden"},
                  {"id": "E-5", "src": "C-3", "dst": "C-2", "kind": "depends_on", "policy": "allowed"}
                ]}"#,
        );
        let drift = drift(&drawn, &actual());
        assert_eq!(drift.missing, ["C-4"]);
        // Between layers, the edge holds for their content: the daemon's db reaches the store.
        assert_eq!(drift.forbidden.len(), 1);
        assert_eq!(drift.forbidden[0].edge, "E-1");
        assert_eq!(
            drift.forbidden[0].dependencies,
            [dependency(
                "component:dir/daemon/db",
                "pkg:cargo/timonier-store"
            )]
        );
        // The store does not reach the daemon; the cli does.
        assert_eq!(
            drift
                .required
                .iter()
                .map(|c| c.edge.as_str())
                .collect::<Vec<_>>(),
            ["E-2"]
        );
        assert_eq!(
            drift.unchecked,
            [Unchecked {
                edge: "E-4".into(),
                unanchored: vec!["C-4".into()]
            }]
        );
    }

    #[test]
    fn the_map_gets_the_names_and_the_ancestors_of_what_is_cited() {
        let drawn = intent(
            r#"{"version": 1,
                "layers": [{"id": "L-1", "name": "Core"}, {"id": "L-2", "name": "Storage"}],
                "components": [{"id": "C-1", "name": "net", "layer": "L-1", "role": ""},
                               {"id": "C-2", "name": "Daemon", "layer": "L-1", "role": ""},
                               {"id": "C-3", "name": "Store", "layer": "L-2", "role": ""}],
                "edges": [{"id": "E-1", "src": "L-1", "dst": "L-2", "kind": "depends_on", "policy": "forbidden"}]}"#,
        );
        let shown = view(&drawn, &actual());
        assert!(shown.drawn);
        assert_eq!(shown.names["L-2"], "Storage");
        assert_eq!(
            shown.ancestors["component:dir/daemon/net"],
            ["pkg:cargo/timonier-daemon"]
        );
        // The ends of the dependency that crosses, the deep one included.
        assert_eq!(
            shown.ancestors["component:dir/daemon/db"],
            ["pkg:cargo/timonier-daemon"]
        );
        assert!(shown.ancestors["pkg:cargo/timonier-store"].is_empty());
    }

    /// PLAN.md 7.6: a whiteboard and a graph of code give the drift expected, through the
    /// architecture the map aggregates.
    #[test]
    fn a_whiteboard_and_a_graph_give_the_expected_drift() {
        use crate::graph::Graph;
        use crate::{Confidence, Edge, EdgeKind, Node, NodeId, NodeKind, Provenance};
        let id = |s: &str| NodeId::parse(s).unwrap();
        let node = |i: &str, kind: NodeKind, parent: Option<&str>| {
            let mut node = Node::new(id(i), kind, i.rsplit(['/', ':']).next().unwrap());
            if let Some(parent) = parent {
                node = node.with_parent(id(parent));
            }
            node
        };
        let edge = |src: &str, dst: &str| Edge {
            src: id(src),
            dst: id(dst),
            kind: EdgeKind::Calls,
            provenance: Provenance::TreeSitter,
            confidence: Confidence::CERTAIN,
            weight: 1,
        };
        // `app` (its `ui` component calls the store) and `store`; `web` is only drawn.
        let graph = Graph::new(
            vec![
                node("repo:r", NodeKind::Repo, None),
                node("pkg:cargo/app", NodeKind::Package, Some("repo:r")),
                node(
                    "component:dir/app/ui",
                    NodeKind::Component,
                    Some("pkg:cargo/app"),
                ),
                node(
                    "file:app/ui/view.rs",
                    NodeKind::File,
                    Some("component:dir/app/ui"),
                ),
                node("pkg:cargo/store", NodeKind::Package, Some("repo:r")),
                node("file:store/lib.rs", NodeKind::File, Some("pkg:cargo/store")),
            ],
            vec![edge("file:app/ui/view.rs", "file:store/lib.rs")],
        );
        let drawn = intent(
            r#"{"version": 1,
                "layers": [{"id": "L-1", "name": "Front"}, {"id": "L-2", "name": "Data"}],
                "components": [{"id": "C-1", "name": "App", "layer": "L-1", "role": ""},
                               {"id": "C-2", "name": "Web", "layer": "L-1", "role": ""},
                               {"id": "C-3", "name": "Store", "layer": "L-2", "role": ""}],
                "edges": [{"id": "E-1", "src": "L-1", "dst": "L-2", "kind": "depends_on", "policy": "forbidden"},
                          {"id": "E-2", "src": "C-3", "dst": "C-1", "kind": "depends_on", "policy": "required"}]}"#,
        );
        let shown = view(&drawn, &crate::actual::actual(&graph));
        let drift = &shown.drift;
        assert_eq!(drift.missing, ["C-2"]);
        assert_eq!(drift.forbidden[0].edge, "E-1");
        assert_eq!(
            drift.forbidden[0].dependencies[0].from,
            "component:dir/app/ui"
        );
        assert_eq!(drift.forbidden[0].dependencies[0].to, "pkg:cargo/store");
        assert_eq!(drift.required[0].edge, "E-2");
        assert_eq!(shown.ancestors["component:dir/app/ui"], ["pkg:cargo/app"]);
    }

    #[test]
    fn nothing_drawn_nothing_to_say() {
        assert_eq!(drift(&Intent::empty(), &actual()), Drift::default());
        // The empty whiteboard `init` puts: nothing drawn yet.
        assert!(!view(&Intent::empty(), &actual()).drawn);
        assert_eq!(normalize(" Timonier_Daemon-2 "), "timonierdaemon2");
        assert_eq!(normalize("Données"), "données");
        assert_ne!(normalize("Données"), normalize("Donnes"));
    }
}
