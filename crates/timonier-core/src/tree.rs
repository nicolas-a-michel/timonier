//! The parent hierarchy is a strict tree with typed levels (spec, PLAN.md Q4, ADR 0008):
//! `Repo → Component* → Package → Component* → File → Symbol*`.

use std::collections::{HashMap, HashSet};
use std::convert::Infallible;

use crate::{Node, NodeId, NodeKind};

/// A placement that breaks the tree.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TreeViolation {
    #[error("{node} ({kind}) needs a parent")]
    MissingParent { node: NodeId, kind: NodeKind },

    #[error("{node} is a repo, the root: it cannot have a parent")]
    RootWithParent { node: NodeId },

    #[error("the parent {parent} of {node} does not exist")]
    UnknownParent { node: NodeId, parent: NodeId },

    #[error("{node} ({kind}) cannot be placed under {parent} ({parent_kind})")]
    ForbiddenParent {
        node: NodeId,
        kind: NodeKind,
        parent: NodeId,
        parent_kind: NodeKind,
    },

    #[error("package {node} cannot be nested in package {ancestor}")]
    NestedPackage { node: NodeId, ancestor: NodeId },

    #[error("{node} would be its own ancestor")]
    Cycle { node: NodeId },

    #[error("{node} cannot disappear while {child} is still placed under it")]
    OrphanedChild { node: NodeId, child: NodeId },

    #[error("there is already a repo, {existing}; {node} cannot be a second one")]
    SecondRepo { node: NodeId, existing: NodeId },

    #[error("{node} appears twice")]
    DuplicateNode { node: NodeId },
}

/// Which parent kinds a node kind accepts.
pub const fn parent_allowed(child: NodeKind, parent: NodeKind) -> bool {
    use NodeKind::{Component, File, Package, Repo, Symbol};
    match child {
        Repo => false,
        Component => matches!(parent, Repo | Component | Package),
        Package => matches!(parent, Repo | Component),
        File => matches!(parent, Repo | Package | Component),
        Symbol(_) => matches!(parent, File | Symbol(_)),
    }
}

/// Why a placement check stopped: a broken tree, or the lookup itself failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckError<E> {
    Violation(TreeViolation),
    Lookup(E),
}

impl<E> From<TreeViolation> for CheckError<E> {
    fn from(violation: TreeViolation) -> Self {
        Self::Violation(violation)
    }
}

/// Checks where one node is placed. `lookup` gives the kind and parent of a current node; the
/// walk up the ancestors detects cycles and nested packages. A missing ancestor further up is
/// not reported here: it is reported when that ancestor's own child is checked.
pub fn check_placement<E>(
    node: &NodeId,
    kind: NodeKind,
    parent: Option<&NodeId>,
    mut lookup: impl FnMut(&NodeId) -> Result<Option<(NodeKind, Option<NodeId>)>, E>,
) -> Result<(), CheckError<E>> {
    let Some(parent) = parent else {
        return match kind {
            NodeKind::Repo => Ok(()),
            _ => Err(TreeViolation::MissingParent {
                node: node.clone(),
                kind,
            }
            .into()),
        };
    };
    if kind == NodeKind::Repo {
        return Err(TreeViolation::RootWithParent { node: node.clone() }.into());
    }
    let Some((parent_kind, mut next)) = lookup(parent).map_err(CheckError::Lookup)? else {
        return Err(TreeViolation::UnknownParent {
            node: node.clone(),
            parent: parent.clone(),
        }
        .into());
    };
    if !parent_allowed(kind, parent_kind) {
        return Err(TreeViolation::ForbiddenParent {
            node: node.clone(),
            kind,
            parent: parent.clone(),
            parent_kind,
        }
        .into());
    }

    let mut seen = HashSet::from([node.clone()]);
    let (mut current, mut current_kind) = (parent.clone(), parent_kind);
    loop {
        if !seen.insert(current.clone()) {
            return Err(TreeViolation::Cycle { node: node.clone() }.into());
        }
        if kind == NodeKind::Package && current_kind == NodeKind::Package {
            return Err(TreeViolation::NestedPackage {
                node: node.clone(),
                ancestor: current,
            }
            .into());
        }
        let Some(up) = next else { return Ok(()) };
        match lookup(&up).map_err(CheckError::Lookup)? {
            Some((up_kind, up_parent)) => {
                (current, current_kind, next) = (up, up_kind, up_parent);
            }
            None => return Ok(()),
        }
    }
}

/// Checks a complete set of nodes: every placement, a single repo, no duplicate.
pub fn validate_forest(nodes: &[Node]) -> Result<(), TreeViolation> {
    let mut by_id: HashMap<&NodeId, &Node> = HashMap::with_capacity(nodes.len());
    let mut repo: Option<&NodeId> = None;
    for node in nodes {
        if by_id.insert(&node.id, node).is_some() {
            return Err(TreeViolation::DuplicateNode {
                node: node.id.clone(),
            });
        }
        if node.kind == NodeKind::Repo {
            if let Some(existing) = repo {
                return Err(TreeViolation::SecondRepo {
                    node: node.id.clone(),
                    existing: existing.clone(),
                });
            }
            repo = Some(&node.id);
        }
    }
    for node in nodes {
        let lookup =
            |id: &NodeId| Ok::<_, Infallible>(by_id.get(id).map(|n| (n.kind, n.parent.clone())));
        match check_placement(&node.id, node.kind, node.parent.as_ref(), lookup) {
            Ok(()) => {}
            Err(CheckError::Violation(violation)) => return Err(violation),
            Err(CheckError::Lookup(never)) => match never {},
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::SymbolKind;

    fn node(id: &str, kind: NodeKind, parent: Option<&str>) -> Node {
        let mut node = Node::new(NodeId::parse(id).unwrap(), kind, id);
        node.parent = parent.map(|p| NodeId::parse(p).unwrap());
        node
    }

    const FUNCTION: NodeKind = NodeKind::Symbol(SymbolKind::Function);

    fn sample() -> Vec<Node> {
        vec![
            node("repo:r", NodeKind::Repo, None),
            node("component:servers", NodeKind::Component, Some("repo:r")),
            node(
                "pkg:cargo/daemon",
                NodeKind::Package,
                Some("component:servers"),
            ),
            node(
                "component:daemon/rpc",
                NodeKind::Component,
                Some("pkg:cargo/daemon"),
            ),
            node(
                "file:daemon/src/rpc.rs",
                NodeKind::File,
                Some("component:daemon/rpc"),
            ),
            node(
                "rust:daemon::rpc",
                NodeKind::Symbol(SymbolKind::Module),
                Some("file:daemon/src/rpc.rs"),
            ),
            node(
                "rust:daemon::rpc::serve",
                FUNCTION,
                Some("rust:daemon::rpc"),
            ),
        ]
    }

    #[test]
    fn accepts_the_decided_levels() {
        assert_eq!(validate_forest(&sample()), Ok(()));
    }

    #[test]
    fn rejects_forbidden_placements() {
        let mut nodes = sample();
        nodes.push(node("rust:loose", FUNCTION, Some("pkg:cargo/daemon")));
        assert!(matches!(
            validate_forest(&nodes),
            Err(TreeViolation::ForbiddenParent { .. })
        ));

        let mut nodes = sample();
        nodes.push(node(
            "pkg:cargo/inner",
            NodeKind::Package,
            Some("component:daemon/rpc"),
        ));
        assert!(matches!(
            validate_forest(&nodes),
            Err(TreeViolation::NestedPackage { .. })
        ));

        let mut nodes = sample();
        nodes.push(node("file:orphan.rs", NodeKind::File, None));
        assert!(matches!(
            validate_forest(&nodes),
            Err(TreeViolation::MissingParent { .. })
        ));

        let mut nodes = sample();
        nodes.push(node(
            "file:ghost.rs",
            NodeKind::File,
            Some("pkg:cargo/missing"),
        ));
        assert!(matches!(
            validate_forest(&nodes),
            Err(TreeViolation::UnknownParent { .. })
        ));

        let mut nodes = sample();
        nodes.push(node("repo:other", NodeKind::Repo, None));
        assert!(matches!(
            validate_forest(&nodes),
            Err(TreeViolation::SecondRepo { .. })
        ));
    }

    #[test]
    fn rejects_cycles() {
        let nodes = vec![
            node("repo:r", NodeKind::Repo, None),
            node("component:a", NodeKind::Component, Some("component:b")),
            node("component:b", NodeKind::Component, Some("component:a")),
        ];
        assert!(matches!(
            validate_forest(&nodes),
            Err(TreeViolation::Cycle { .. })
        ));
    }

    /// Builds a random valid tree: each new node picks a kind and a random allowed parent.
    fn valid_tree() -> impl Strategy<Value = Vec<Node>> {
        prop::collection::vec(
            (0_usize..NodeKind::ALL.len(), any::<prop::sample::Index>()),
            0..60,
        )
        .prop_map(|choices| {
            let mut nodes = vec![node("repo:r", NodeKind::Repo, None)];
            for (i, (kind_index, parent_pick)) in choices.into_iter().enumerate() {
                let kind = NodeKind::ALL[kind_index];
                let candidates: Vec<&Node> = nodes
                    .iter()
                    .filter(|parent| parent_allowed(kind, parent.kind))
                    .filter(|parent| {
                        kind != NodeKind::Package || !has_package_ancestor(&nodes, parent)
                    })
                    .collect();
                if candidates.is_empty() {
                    continue;
                }
                let parent = candidates[parent_pick.index(candidates.len())].id.clone();
                let id = match kind {
                    NodeKind::Component => format!("component:c{i}"),
                    NodeKind::Package => format!("pkg:cargo/p{i}"),
                    NodeKind::File => format!("file:f{i}.rs"),
                    _ => format!("rust:s{i}"),
                };
                nodes.push(node(&id, kind, Some(parent.as_str())));
            }
            nodes
        })
    }

    fn has_package_ancestor(nodes: &[Node], start: &Node) -> bool {
        let mut current = Some(start);
        while let Some(n) = current {
            if n.kind == NodeKind::Package {
                return true;
            }
            current = n
                .parent
                .as_ref()
                .and_then(|p| nodes.iter().find(|m| &m.id == p));
        }
        false
    }

    fn descendants(nodes: &[Node], root: &NodeId) -> Vec<NodeId> {
        let mut found = vec![root.clone()];
        let mut i = 0;
        while i < found.len() {
            let current = found[i].clone();
            found.extend(
                nodes
                    .iter()
                    .filter(|n| n.parent.as_ref() == Some(&current))
                    .map(|n| n.id.clone()),
            );
            i += 1;
        }
        found
    }

    /// An independent, deliberately naive judge of a whole set of nodes, to compare with.
    fn oracle_accepts(nodes: &[Node]) -> bool {
        let by_id: HashMap<&NodeId, &Node> = nodes.iter().map(|n| (&n.id, n)).collect();
        if by_id.len() != nodes.len()
            || nodes.iter().filter(|n| n.kind == NodeKind::Repo).count() > 1
        {
            return false;
        }
        nodes.iter().all(|node| {
            let Some(parent_id) = &node.parent else {
                return node.kind == NodeKind::Repo;
            };
            let Some(parent) = by_id.get(parent_id) else {
                return false;
            };
            if node.kind == NodeKind::Repo || !parent_allowed(node.kind, parent.kind) {
                return false;
            }
            // Walk up at most `len` steps: going further means a cycle.
            let mut current = Some(*parent);
            for _ in 0..=nodes.len() {
                let Some(ancestor) = current else { return true };
                if ancestor.id == node.id
                    || (node.kind == NodeKind::Package && ancestor.kind == NodeKind::Package)
                {
                    return false;
                }
                current = ancestor.parent.as_ref().and_then(|p| by_id.get(p).copied());
            }
            false
        })
    }

    #[derive(Clone, Debug)]
    enum Mutation {
        Reparent {
            node: prop::sample::Index,
            parent: Option<prop::sample::Index>,
        },
        Rekind {
            node: prop::sample::Index,
            kind: usize,
        },
    }

    fn mutations() -> impl Strategy<Value = Vec<Mutation>> {
        prop::collection::vec(
            prop_oneof![
                (
                    any::<prop::sample::Index>(),
                    prop::option::of(any::<prop::sample::Index>())
                )
                    .prop_map(|(node, parent)| Mutation::Reparent { node, parent }),
                (any::<prop::sample::Index>(), 0..NodeKind::ALL.len())
                    .prop_map(|(node, kind)| Mutation::Rekind { node, kind }),
            ],
            1..4,
        )
    }

    proptest! {
        #[test]
        fn the_checker_agrees_with_a_naive_judge_on_any_mutation(nodes in valid_tree(), mutations in mutations()) {
            let mut nodes = nodes;
            for mutation in mutations {
                let len = nodes.len();
                match mutation {
                    Mutation::Reparent { node, parent } => {
                        let parent = parent.map(|p| nodes[p.index(len)].id.clone());
                        nodes[node.index(len)].parent = parent;
                    }
                    Mutation::Rekind { node, kind } => nodes[node.index(len)].kind = NodeKind::ALL[kind],
                }
            }
            let result = validate_forest(&nodes);
            prop_assert_eq!(result.is_ok(), oracle_accepts(&nodes), "checker said {:?}", result);
        }
    }

    proptest! {
        #[test]
        fn every_valid_construction_is_accepted(nodes in valid_tree()) {
            prop_assert_eq!(validate_forest(&nodes), Ok(()));
        }

        #[test]
        fn reparenting_under_a_descendant_is_a_cycle(nodes in valid_tree(), pick in any::<prop::sample::Index>(), below in any::<prop::sample::Index>()) {
            // Only components can sit under components, so cycles are built from them.
            let components: Vec<_> = nodes.iter().filter(|n| n.kind == NodeKind::Component).map(|n| n.id.clone()).collect();
            prop_assume!(!components.is_empty());
            let moved = components[pick.index(components.len())].clone();
            let under: Vec<_> = descendants(&nodes, &moved).into_iter()
                .filter(|d| nodes.iter().any(|n| &n.id == d && n.kind == NodeKind::Component))
                .collect();
            let new_parent = under[below.index(under.len())].clone();
            let mut nodes = nodes;
            nodes.iter_mut().find(|n| n.id == moved).unwrap().parent = Some(new_parent);
            let result = validate_forest(&nodes);
            let is_cycle = matches!(result, Err(TreeViolation::Cycle { .. }));
            prop_assert!(is_cycle, "expected a cycle, got {:?}", result);
        }
    }
}
