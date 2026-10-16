//! Structural diff between two snapshots, with move detection (ADR 0010).
//!
//! The store finds what changed between two snapshots (a query on validity intervals); this
//! module turns those raw changes into a diff where refactorings read as moves, not as
//! "everything deleted, everything recreated".

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{ContentHash, Edge, EdgeKey, Node, NodeId, NodeKind};

/// Why two nodes were recognised as the same thing, moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MoveEvidence {
    /// Same kind and same normalised body.
    SameBody,
    /// Same kind, name and signature; the body changed.
    SameSignature,
    /// Same id, in another file: the id does not depend on the file (a Rust module moved from
    /// `foo/mod.rs` to `foo.rs`, a SCIP symbol…).
    SameId,
    /// A file whose symbols (at least half of them) moved into the new file.
    FollowedItsSymbols { moved: u32, total: u32 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum NodeChange {
    Added {
        node: Node,
    },
    Removed {
        node: Node,
    },
    Modified {
        before: Node,
        after: Node,
    },
    Moved {
        before: Node,
        after: Node,
        evidence: MoveEvidence,
    },
}

impl NodeChange {
    /// The id the change is sorted and looked up by: the new id when there is one.
    pub fn id(&self) -> &NodeId {
        match self {
            Self::Added { node } | Self::Removed { node } => &node.id,
            Self::Modified { after, .. } | Self::Moved { after, .. } => &after.id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum EdgeChange {
    Added { edge: Edge },
    Removed { edge: Edge },
    Modified { before: Edge, after: Edge },
}

impl EdgeChange {
    pub fn key(&self) -> EdgeKey {
        match self {
            Self::Added { edge } | Self::Removed { edge } => edge.key(),
            Self::Modified { after, .. } => after.key(),
        }
    }
}

/// The structural changes from snapshot `from` to snapshot `to`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphDiff {
    pub from: u64,
    pub to: u64,
    pub nodes: Vec<NodeChange>,
    pub edges: Vec<EdgeChange>,
}

impl GraphDiff {
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.edges.is_empty()
    }

    pub fn node(&self, id: &NodeId) -> Option<&NodeChange> {
        self.nodes.iter().find(|change| change.id() == id)
    }
}

/// Raw changes found by the store, before move detection.
#[derive(Clone, Debug, Default)]
pub struct RawDiff {
    pub removed_nodes: Vec<Node>,
    pub added_nodes: Vec<Node>,
    pub modified_nodes: Vec<(Node, Node)>,
    pub removed_edges: Vec<Edge>,
    pub added_edges: Vec<Edge>,
    pub modified_edges: Vec<(Edge, Edge)>,
    /// The file each removed or modified symbol was in, at `from`.
    pub file_before: HashMap<NodeId, NodeId>,
    /// The file each added or modified symbol is in, at `to`.
    pub file_after: HashMap<NodeId, NodeId>,
}

/// Turns raw changes into a diff: pairs moved symbols and files, then hides the edge changes
/// that only follow those moves. The output is sorted by id and edge key.
pub fn assemble(from: u64, to: u64, raw: RawDiff) -> GraphDiff {
    let RawDiff {
        mut removed_nodes,
        mut added_nodes,
        mut modified_nodes,
        removed_edges,
        added_edges,
        modified_edges,
        file_before,
        file_after,
    } = raw;
    removed_nodes.sort_by(|a, b| a.id.cmp(&b.id));
    added_nodes.sort_by(|a, b| a.id.cmp(&b.id));
    modified_nodes.sort_by(|a, b| a.1.id.cmp(&b.1.id));

    // Symbols that kept their id but changed file are moves too.
    let relocated: Vec<bool> = modified_nodes
        .iter()
        .map(|(before, _)| {
            before.kind.is_symbol()
                && matches!(
                    (file_before.get(&before.id), file_after.get(&before.id)),
                    (Some(old), Some(new)) if old != new
                )
        })
        .collect();

    let mut moves = pair_symbols(&removed_nodes, &added_nodes);
    let relocated_ids: Vec<&NodeId> = modified_nodes
        .iter()
        .zip(&relocated)
        .filter(|(_, moved)| **moved)
        .map(|((before, _), _)| &before.id)
        .collect();
    let files = Files {
        removed: &removed_nodes,
        added: &added_nodes,
        before: &file_before,
        after: &file_after,
    };
    moves.extend(pair_files(&files, &moves, &relocated_ids));

    let moved_from: HashSet<usize> = moves.iter().map(|m| m.removed).collect();
    let moved_to: HashSet<usize> = moves.iter().map(|m| m.added).collect();
    let renamed: HashMap<&NodeId, &NodeId> = moves
        .iter()
        .map(|m| (&removed_nodes[m.removed].id, &added_nodes[m.added].id))
        .collect();

    let mut nodes: Vec<NodeChange> = Vec::new();
    for m in &moves {
        nodes.push(NodeChange::Moved {
            before: removed_nodes[m.removed].clone(),
            after: added_nodes[m.added].clone(),
            evidence: m.evidence,
        });
    }
    let unmoved_removed = removed_nodes
        .iter()
        .enumerate()
        .filter(|(i, _)| !moved_from.contains(i));
    nodes.extend(unmoved_removed.map(|(_, node)| NodeChange::Removed { node: node.clone() }));
    let unmoved_added = added_nodes
        .iter()
        .enumerate()
        .filter(|(i, _)| !moved_to.contains(i));
    nodes.extend(unmoved_added.map(|(_, node)| NodeChange::Added { node: node.clone() }));
    for ((before, after), moved) in modified_nodes.into_iter().zip(relocated) {
        nodes.push(if moved {
            NodeChange::Moved {
                before,
                after,
                evidence: MoveEvidence::SameId,
            }
        } else {
            NodeChange::Modified { before, after }
        });
    }
    nodes.sort_by(|a, b| a.id().cmp(b.id()));

    let edges = follow_moves(removed_edges, added_edges, modified_edges, &renamed);
    GraphDiff {
        from,
        to,
        nodes,
        edges,
    }
}

struct Move {
    removed: usize,
    added: usize,
    evidence: MoveEvidence,
}

/// One-to-one pairing of removed and added symbols, in two passes over lists sorted by id, so
/// that the result is deterministic (ADR 0010):
/// 1. same kind and same body. A body that several removed (or several added) symbols share is
///    too common to identify one (empty bodies, one-liners): it only pairs symbols that kept
///    their name;
/// 2. among the symbols left, same kind, name and signature, when exactly one symbol on each
///    side has it.
fn pair_symbols(removed: &[Node], added: &[Node]) -> Vec<Move> {
    let symbol = |node: &&Node| node.kind.is_symbol();
    let mut removed_bodies: HashMap<(NodeKind, ContentHash), usize> = HashMap::new();
    for node in removed.iter().filter(symbol) {
        if let Some(hash) = node.body_hash {
            *removed_bodies.entry((node.kind, hash)).or_default() += 1;
        }
    }
    let mut by_body: BTreeMap<(NodeKind, ContentHash), Vec<usize>> = BTreeMap::new();
    let mut by_signature: BTreeMap<(NodeKind, &str, &str), Vec<usize>> = BTreeMap::new();
    for (i, node) in added.iter().enumerate().filter(|(_, n)| n.kind.is_symbol()) {
        if let Some(hash) = node.body_hash {
            by_body.entry((node.kind, hash)).or_default().push(i);
        }
        if let Some(signature) = &node.signature {
            by_signature
                .entry((node.kind, node.name.as_str(), signature.as_str()))
                .or_default()
                .push(i);
        }
    }

    let mut taken_added = HashSet::new();
    let mut taken_removed = HashSet::new();
    let mut moves = Vec::new();
    for (i, node) in removed
        .iter()
        .enumerate()
        .filter(|(_, n)| n.kind.is_symbol())
    {
        let Some(hash) = node.body_hash else { continue };
        let Some(candidates) = by_body.get(&(node.kind, hash)) else {
            continue;
        };
        let ambiguous = candidates.len() > 1
            || removed_bodies.get(&(node.kind, hash)).copied().unwrap_or(0) > 1;
        let free = |j: &&usize| !taken_added.contains(*j);
        let same_name = candidates
            .iter()
            .filter(free)
            .find(|j| added[**j].name == node.name);
        let found = if ambiguous {
            same_name
        } else {
            same_name.or_else(|| candidates.iter().find(free))
        };
        if let Some(&j) = found {
            taken_added.insert(j);
            taken_removed.insert(i);
            moves.push(Move {
                removed: i,
                added: j,
                evidence: MoveEvidence::SameBody,
            });
        }
    }
    // Several removed (or added) symbols with the same kind, name and signature cannot tell which
    // is which: no pairing then.
    let mut removed_signatures: HashMap<(NodeKind, &str, &str), usize> = HashMap::new();
    for (i, node) in removed
        .iter()
        .enumerate()
        .filter(|(_, n)| n.kind.is_symbol())
    {
        if let (false, Some(signature)) = (taken_removed.contains(&i), node.signature.as_deref()) {
            *removed_signatures
                .entry((node.kind, node.name.as_str(), signature))
                .or_default() += 1;
        }
    }
    for (i, node) in removed
        .iter()
        .enumerate()
        .filter(|(_, n)| n.kind.is_symbol())
    {
        if taken_removed.contains(&i) {
            continue;
        }
        let Some(signature) = node.signature.as_deref() else {
            continue;
        };
        let key = (node.kind, node.name.as_str(), signature);
        let Some(candidates) = by_signature.get(&key) else {
            continue;
        };
        let free: Vec<usize> = candidates
            .iter()
            .copied()
            .filter(|j| !taken_added.contains(j))
            .collect();
        if free.len() != 1 || removed_signatures.get(&key).copied().unwrap_or(0) != 1 {
            continue;
        }
        taken_added.insert(free[0]);
        moves.push(Move {
            removed: i,
            added: free[0],
            evidence: MoveEvidence::SameSignature,
        });
    }
    moves
}

/// What `pair_files` needs to know about the files of the diff.
struct Files<'a> {
    removed: &'a [Node],
    added: &'a [Node],
    before: &'a HashMap<NodeId, NodeId>,
    after: &'a HashMap<NodeId, NodeId>,
}

/// A removed file and an added file are the same file, moved, when at least half of the removed
/// file's changed symbols (and at least one) went into the added file: moved symbols, and
/// symbols that kept their id but changed file (`relocated`).
fn pair_files(files: &Files<'_>, symbol_moves: &[Move], relocated: &[&NodeId]) -> Vec<Move> {
    let added_files: HashMap<&NodeId, usize> = files
        .added
        .iter()
        .enumerate()
        .filter(|(_, n)| n.kind == NodeKind::File)
        .map(|(i, n)| (&n.id, i))
        .collect();
    let mut totals: HashMap<&NodeId, u32> = HashMap::new();
    let left = files
        .removed
        .iter()
        .filter(|n| n.kind.is_symbol())
        .map(|n| &n.id);
    for id in left.chain(relocated.iter().copied()) {
        if let Some(file) = files.before.get(id) {
            *totals.entry(file).or_default() += 1;
        }
    }
    let mut followed: BTreeMap<(&NodeId, usize), u32> = BTreeMap::new();
    let mut follow = |old: &NodeId, new: &NodeId| {
        let old_file = files.before.get(old);
        let new_file = files.after.get(new).and_then(|f| added_files.get(f));
        if let (Some(old_file), Some(new_file)) = (old_file, new_file) {
            *followed.entry((old_file, *new_file)).or_default() += 1;
        }
    };
    for m in symbol_moves {
        follow(&files.removed[m.removed].id, &files.added[m.added].id);
    }
    for id in relocated {
        follow(id, id);
    }

    let mut taken = HashSet::new();
    let mut moves = Vec::new();
    for (i, file) in files
        .removed
        .iter()
        .enumerate()
        .filter(|(_, n)| n.kind == NodeKind::File)
    {
        let total = totals.get(&file.id).copied().unwrap_or(0);
        let best = followed
            .range((&file.id, 0)..=(&file.id, usize::MAX))
            .filter(|((_, j), _)| !taken.contains(j))
            .max_by(|((_, ja), a), ((_, jb), b)| a.cmp(b).then(jb.cmp(ja)));
        if let Some(((_, j), &moved)) = best
            && total > 0
            && moved * 2 >= total
        {
            taken.insert(*j);
            moves.push(Move {
                removed: i,
                added: *j,
                evidence: MoveEvidence::FollowedItsSymbols { moved, total },
            });
        }
    }
    moves
}

/// Hides the edge changes that only rename an endpoint through a move; an edge whose attributes
/// also changed becomes a modification.
fn follow_moves(
    removed: Vec<Edge>,
    added: Vec<Edge>,
    modified: Vec<(Edge, Edge)>,
    renamed: &HashMap<&NodeId, &NodeId>,
) -> Vec<EdgeChange> {
    let mut added_by_key: BTreeMap<EdgeKey, Edge> =
        added.into_iter().map(|e| (e.key(), e)).collect();
    let mut changes = Vec::new();
    for edge in removed {
        let src = renamed.get(&edge.src).copied().unwrap_or(&edge.src);
        let dst = renamed.get(&edge.dst).copied().unwrap_or(&edge.dst);
        let followed = EdgeKey {
            src: src.clone(),
            dst: dst.clone(),
            kind: edge.kind,
        };
        let moved = followed != edge.key();
        match added_by_key.remove(&followed) {
            Some(after) if moved => {
                let unchanged = after.provenance == edge.provenance
                    && after.confidence == edge.confidence
                    && after.weight == edge.weight;
                if !unchanged {
                    changes.push(EdgeChange::Modified {
                        before: edge,
                        after,
                    });
                }
            }
            Some(after) => {
                // Same key both removed and added cannot come from the store; keep both visible.
                changes.push(EdgeChange::Modified {
                    before: edge,
                    after,
                });
            }
            None => changes.push(EdgeChange::Removed { edge }),
        }
    }
    changes.extend(
        added_by_key
            .into_values()
            .map(|edge| EdgeChange::Added { edge }),
    );
    changes.extend(
        modified
            .into_iter()
            .map(|(before, after)| EdgeChange::Modified { before, after }),
    );
    changes.sort_by_key(EdgeChange::key);
    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Confidence, EdgeKind, Provenance, SymbolKind};

    const FUNCTION: NodeKind = NodeKind::Symbol(SymbolKind::Function);

    fn id(s: &str) -> NodeId {
        NodeId::parse(s).unwrap()
    }

    fn symbol(qualified: &str, body: u8) -> Node {
        let name = qualified.rsplit("::").next().unwrap();
        Node::new(id(&format!("rust:{qualified}")), FUNCTION, name)
            .with_signature(format!("fn {name}()"))
            .with_body_hash(ContentHash::from_bytes([body; 32]))
    }

    fn file(path: &str) -> Node {
        Node::new(id(&format!("file:{path}")), NodeKind::File, path)
    }

    fn calls(src: &str, dst: &str) -> Edge {
        Edge {
            src: id(src),
            dst: id(dst),
            kind: EdgeKind::Calls,
            provenance: Provenance::Lsp,
            confidence: Confidence::CERTAIN,
            weight: 1,
        }
    }

    #[test]
    fn a_renamed_function_with_the_same_body_is_a_move() {
        let raw = RawDiff {
            removed_nodes: vec![symbol("a::old_name", 1)],
            added_nodes: vec![symbol("a::new_name", 1)],
            ..RawDiff::default()
        };
        let diff = assemble(1, 2, raw);
        assert_eq!(diff.nodes.len(), 1);
        assert!(matches!(
            diff.nodes[0],
            NodeChange::Moved {
                evidence: MoveEvidence::SameBody,
                ..
            }
        ));
    }

    #[test]
    fn a_moved_function_with_a_new_body_is_matched_by_signature() {
        let raw = RawDiff {
            removed_nodes: vec![symbol("a::f", 1)],
            added_nodes: vec![symbol("b::f", 2)],
            ..RawDiff::default()
        };
        let diff = assemble(1, 2, raw);
        assert!(matches!(
            diff.nodes[0],
            NodeChange::Moved {
                evidence: MoveEvidence::SameSignature,
                ..
            }
        ));
    }

    #[test]
    fn different_things_stay_removed_and_added() {
        let raw = RawDiff {
            removed_nodes: vec![symbol("a::f", 1)],
            added_nodes: vec![symbol("a::g", 2)],
            ..RawDiff::default()
        };
        let diff = assemble(1, 2, raw);
        let kinds: Vec<_> = diff
            .nodes
            .iter()
            .map(|c| matches!(c, NodeChange::Added { .. } | NodeChange::Removed { .. }))
            .collect();
        assert_eq!(kinds, [true, true]);
    }

    #[test]
    fn pairing_is_one_to_one_and_prefers_the_same_name() {
        let raw = RawDiff {
            removed_nodes: vec![symbol("a::f", 1), symbol("a::g", 1)],
            added_nodes: vec![symbol("b::f", 1), symbol("b::g", 1)],
            ..RawDiff::default()
        };
        let diff = assemble(1, 2, raw);
        let pairs: Vec<_> = diff
            .nodes
            .iter()
            .map(|c| match c {
                NodeChange::Moved { before, after, .. } => {
                    (before.name.clone(), after.name.clone())
                }
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(pairs, [("f".into(), "f".into()), ("g".into(), "g".into())]);
    }

    #[test]
    fn a_file_follows_its_symbols_and_its_edges_do_not_change() {
        let (old, new) = (file("src/a.rs"), file("src/b.rs"));
        let raw = RawDiff {
            removed_nodes: vec![
                old.clone(),
                symbol("a::f", 1),
                symbol("a::g", 2),
                symbol("a::h", 3),
            ],
            added_nodes: vec![new.clone(), symbol("b::f", 1), symbol("b::g", 2)],
            removed_edges: vec![calls("rust:a::f", "rust:a::g")],
            added_edges: vec![calls("rust:b::f", "rust:b::g")],
            file_before: ["rust:a::f", "rust:a::g", "rust:a::h"]
                .iter()
                .map(|s| (id(s), old.id.clone()))
                .collect(),
            file_after: ["rust:b::f", "rust:b::g"]
                .iter()
                .map(|s| (id(s), new.id.clone()))
                .collect(),
            ..RawDiff::default()
        };
        let diff = assemble(1, 2, raw);
        match diff.node(&new.id) {
            Some(NodeChange::Moved {
                before, evidence, ..
            }) => {
                assert_eq!(before.id, old.id);
                assert_eq!(
                    *evidence,
                    MoveEvidence::FollowedItsSymbols { moved: 2, total: 3 }
                );
            }
            other => panic!("expected a file move, got {other:?}"),
        }
        assert!(matches!(
            diff.node(&id("rust:a::h")),
            Some(NodeChange::Removed { .. })
        ));
        assert!(diff.edges.is_empty(), "{:?}", diff.edges);
    }

    #[test]
    fn a_file_keeping_less_than_half_of_its_symbols_is_not_moved() {
        let (old, new) = (file("a.rs"), file("b.rs"));
        let raw = RawDiff {
            removed_nodes: vec![
                old.clone(),
                symbol("a::f", 1),
                symbol("a::g", 2),
                symbol("a::h", 3),
            ],
            added_nodes: vec![new.clone(), symbol("b::f", 1)],
            file_before: ["rust:a::f", "rust:a::g", "rust:a::h"]
                .iter()
                .map(|s| (id(s), old.id.clone()))
                .collect(),
            file_after: [(id("rust:b::f"), new.id.clone())].into(),
            ..RawDiff::default()
        };
        let diff = assemble(1, 2, raw);
        assert!(matches!(diff.node(&new.id), Some(NodeChange::Added { .. })));
    }

    #[test]
    fn same_body_matches_come_before_same_signature_matches() {
        // a::f could pair with b::f by signature, but a::z has b::f's body: the body wins.
        let raw = RawDiff {
            removed_nodes: vec![symbol("a::f", 1), symbol("a::z", 2)],
            added_nodes: vec![symbol("b::f", 2)],
            ..RawDiff::default()
        };
        let diff = assemble(1, 2, raw);
        assert!(
            matches!(diff.node(&id("rust:b::f")), Some(NodeChange::Moved { before, evidence: MoveEvidence::SameBody, .. }) if before.name == "z")
        );
        assert!(matches!(
            diff.node(&id("rust:a::f")),
            Some(NodeChange::Removed { .. })
        ));
    }

    #[test]
    fn a_common_body_only_pairs_symbols_that_kept_their_name() {
        // Two empty functions removed, two others added with the same empty body: no renaming is
        // invented between different names.
        let raw = RawDiff {
            removed_nodes: vec![symbol("a::f", 0), symbol("a::g", 0)],
            added_nodes: vec![symbol("a::h", 0), symbol("a::k", 0)],
            ..RawDiff::default()
        };
        let diff = assemble(1, 2, raw);
        assert!(
            diff.nodes
                .iter()
                .all(|c| !matches!(c, NodeChange::Moved { .. })),
            "{:#?}",
            diff.nodes
        );
    }

    #[test]
    fn a_symbol_keeping_its_id_in_another_file_is_a_move_and_its_file_follows() {
        // `src/foo/mod.rs` becomes `src/foo.rs`: the module keeps its id, only its parent changes.
        let (old, new) = (file("src/foo/mod.rs"), file("src/foo.rs"));
        let before = symbol("foo", 1).with_parent(old.id.clone());
        let after = symbol("foo", 1).with_parent(new.id.clone());
        let raw = RawDiff {
            removed_nodes: vec![old.clone()],
            added_nodes: vec![new.clone()],
            modified_nodes: vec![(before.clone(), after)],
            file_before: [(before.id.clone(), old.id.clone())].into(),
            file_after: [(before.id.clone(), new.id.clone())].into(),
            ..RawDiff::default()
        };
        let diff = assemble(1, 2, raw);
        assert!(matches!(
            diff.node(&before.id),
            Some(NodeChange::Moved {
                evidence: MoveEvidence::SameId,
                ..
            })
        ));
        assert!(
            matches!(diff.node(&new.id), Some(NodeChange::Moved { before, evidence: MoveEvidence::FollowedItsSymbols { moved: 1, total: 1 }, .. }) if before.id == old.id),
            "{:#?}",
            diff.nodes
        );
        assert_eq!(diff.nodes.len(), 2);
    }

    #[test]
    fn an_edge_following_a_move_with_new_attributes_is_modified() {
        let mut after = calls("rust:b::f", "rust:x::y");
        after.weight = 4;
        let raw = RawDiff {
            removed_nodes: vec![symbol("a::f", 1)],
            added_nodes: vec![symbol("b::f", 1)],
            removed_edges: vec![calls("rust:a::f", "rust:x::y")],
            added_edges: vec![after],
            ..RawDiff::default()
        };
        let diff = assemble(1, 2, raw);
        assert_eq!(diff.edges.len(), 1);
        assert!(matches!(&diff.edges[0], EdgeChange::Modified { after, .. } if after.weight == 4));
    }

    #[test]
    fn a_shared_signature_does_not_pair_unrelated_symbols() {
        // Two `new` functions removed, two added: the signature cannot tell which is which.
        let mut raw = RawDiff {
            removed_nodes: vec![symbol("a::new", 1), symbol("b::new", 2)],
            added_nodes: vec![symbol("c::new", 3), symbol("d::new", 4)],
            ..RawDiff::default()
        };
        for node in raw
            .removed_nodes
            .iter_mut()
            .chain(raw.added_nodes.iter_mut())
        {
            node.name = "new".into();
            node.signature = Some("fn new() -> Self".into());
        }
        let diff = assemble(1, 2, raw);
        assert!(
            diff.nodes
                .iter()
                .all(|c| !matches!(c, NodeChange::Moved { .. })),
            "{:#?}",
            diff.nodes
        );
    }

    #[test]
    fn every_change_variant_round_trips_through_json() {
        let edge = calls("rust:a::f", "rust:b::g");
        let mut changed = edge.clone();
        changed.weight = 9;
        let diff = GraphDiff {
            from: 3,
            to: 5,
            nodes: vec![
                NodeChange::Added {
                    node: symbol("a::f", 1),
                },
                NodeChange::Removed {
                    node: symbol("a::g", 2),
                },
                NodeChange::Modified {
                    before: symbol("a::h", 3),
                    after: symbol("a::h", 4),
                },
                NodeChange::Moved {
                    before: symbol("a::k", 5),
                    after: symbol("b::k", 5),
                    evidence: MoveEvidence::SameBody,
                },
                NodeChange::Moved {
                    before: symbol("a::m", 5),
                    after: symbol("b::m", 6),
                    evidence: MoveEvidence::SameSignature,
                },
                NodeChange::Moved {
                    before: symbol("a::n", 7),
                    after: symbol("a::n", 7),
                    evidence: MoveEvidence::SameId,
                },
                NodeChange::Moved {
                    before: file("a.rs"),
                    after: file("b.rs"),
                    evidence: MoveEvidence::FollowedItsSymbols { moved: 2, total: 3 },
                },
            ],
            edges: vec![
                EdgeChange::Added { edge: edge.clone() },
                EdgeChange::Removed { edge: edge.clone() },
                EdgeChange::Modified {
                    before: edge.clone(),
                    after: changed,
                },
            ],
        };
        let json = serde_json::to_string(&diff).unwrap();
        assert_eq!(serde_json::from_str::<GraphDiff>(&json).unwrap(), diff);
        let key = edge.key();
        assert_eq!(
            serde_json::from_str::<EdgeKey>(&serde_json::to_string(&key).unwrap()).unwrap(),
            key
        );
    }
}
