//! The review diff (ADR 0024): what changed between the snapshot of the last review and the
//! head, at the level of the map's boxes.
//!
//! A box is a component or a package. Two boxes depend on each other when an edge goes from
//! inside one to inside the other, and they are siblings: the children of the nearest ancestor
//! that holds both ends. Only what a change of the code explains counts: a dependency that
//! appeared or went when a file that carries it changed (Q28), a box when it holds a changed
//! file (Q30): neither what the language server alone changed, nor a regrouping of files into
//! components. A box is new or gone only when code comes or goes with it (Q33): a file, or
//! for a package its manifest (Q35). A dependency comes or goes with an edge, in a changed file
//! (Q36), or with a file moved between two boxes that are no regrouping. A file moved and a box
//! renamed are found again under their new id: modified, not deleted then added (spec
//! « Identité stable »).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::graph::Graph;
use crate::{ContentHash, Edge, NodeId, NodeKind};

/// A fan-in or fan-out jumped when it grew by at least this much…
pub const JUMP_BY: u32 = 3;

/// …and by at least half as much again (`2 × after ≥ 3 × before`).
fn jumped(before: u32, after: u32) -> bool {
    after >= before + JUMP_BY && 2 * u64::from(after) >= 3 * u64::from(before)
}

/// What happened to a box or a file present at the head.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Change {
    Added,
    Modified,
}

/// A fan-in or a fan-out, at the review and at the head.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Jump {
    pub before: u32,
    pub after: u32,
}

/// Some edge goes from inside `src` to inside `dst`, two sibling boxes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Dependency {
    pub src: NodeId,
    pub dst: NodeId,
}

/// The changes from the review to the head.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReviewDiff {
    /// Files present on one side only, or whose content hash differs.
    pub changed_files: BTreeSet<NodeId>,
    /// Files moved, with their symbols, and boxes renamed: their id at the review, and at the
    /// head.
    pub renamed: BTreeMap<NodeId, NodeId>,
    /// Packages a manifest made, undid or changed (Q34, Q35): new, gone, renamed, or internal
    /// at one end and external at the other; by their ids at the review and at the head.
    pub by_manifest: BTreeSet<NodeId>,
    /// Boxes and files of the head that are new or changed.
    pub changes: BTreeMap<NodeId, Change>,
    /// Boxes of the review gone from the head.
    pub removed: BTreeSet<NodeId>,
    /// Boxes that a regrouping alone made or undid (Q33): new at the head and not added, or
    /// gone from it and not removed. A file moved to or from them brings no dependency.
    pub regrouped: BTreeSet<NodeId>,
    pub dependencies_added: BTreeSet<Dependency>,
    pub dependencies_removed: BTreeSet<Dependency>,
    /// Boxes strongly connected under one parent at the head, that no cycle of the review
    /// held.
    pub cycles_added: Vec<BTreeSet<NodeId>>,
    pub fan_in: BTreeMap<NodeId, Jump>,
    pub fan_out: BTreeMap<NodeId, Jump>,
}

impl ReviewDiff {
    /// The cycle added that holds this box.
    pub fn cycle_of(&self, id: &NodeId) -> Option<&BTreeSet<NodeId>> {
        self.cycles_added.iter().find(|cycle| cycle.contains(id))
    }
}

/// Compares the review with the head. `reviewed` and `current` give the content hash of each
/// file, by scope key (`file:<path>`), at the review and now.
pub fn compare(
    review: &Graph,
    head: &Graph,
    reviewed: &HashMap<String, ContentHash>,
    current: &HashMap<String, ContentHash>,
) -> ReviewDiff {
    let changed = changed_files(review, head, reviewed, current);
    let renamed = renamed(review, head, reviewed, current);
    let rename = |id: &NodeId| renamed.get(id).unwrap_or(id).clone();
    let held = (holding(review), holding(head));
    let manifest = by_manifest(review, head, &renamed, &held);
    let (changes, removed) = changes(review, head, &changed, &renamed, &held, &manifest);
    let regrouped = regrouped(review, head, &renamed, &changes, &removed);
    let edges = Edges::new(review, head, &changed, &renamed, &manifest, &regrouped);
    // A dependency comes or goes with an edge that a change of the code brought or took (Q28,
    // Q36), or with a file moved between two boxes that are no regrouping (Q38).
    let (moved_before, moved_after) = edges.moved_dependencies();
    // The review's dependencies under the head's ids.
    let reviewed_raw = dependencies(review, &edges, Side::Review);
    let mut before: BTreeMap<Dependency, Carried> = BTreeMap::new();
    for (dependency, carried) in &reviewed_raw {
        let mapped = Dependency {
            src: rename(&dependency.src),
            dst: rename(&dependency.dst),
        };
        *before.entry(mapped).or_default() |= *carried;
    }
    let after = dependencies(head, &edges, Side::Head);
    let dependencies_added: BTreeSet<Dependency> = after
        .iter()
        .filter(|(dependency, carried)| {
            (carried.changed || moved_after.contains(*dependency))
                && !before.contains_key(*dependency)
        })
        .map(|(dependency, _)| dependency.clone())
        .collect();
    let dependencies_removed: BTreeSet<Dependency> = before
        .iter()
        .filter(|(dependency, carried)| {
            (carried.changed || moved_before.contains(*dependency))
                && !after.contains_key(*dependency)
        })
        .map(|(dependency, _)| dependency.clone())
        .collect();
    let reviewed_dependencies: BTreeSet<Dependency> = before.keys().cloned().collect();
    // The fans: the review's dependencies, with what the code changed.
    let counted: BTreeSet<Dependency> = reviewed_dependencies
        .difference(&dependencies_removed)
        .chain(&dependencies_added)
        .cloned()
        .collect();
    // The cycles: in the head's boxes as they are, regroupings included, without what the
    // language server alone brought and with what it alone took (Q28); counted when a
    // dependency added closes them.
    let structure: BTreeSet<Dependency> = after
        .iter()
        .filter(|(dependency, carried)| carried.real || before.contains_key(*dependency))
        .chain(
            before
                .iter()
                .filter(|(dependency, carried)| !carried.real && !after.contains_key(*dependency)),
        )
        .map(|(dependency, _)| dependency.clone())
        .collect();

    let old_cycles: Vec<BTreeSet<NodeId>> = cycles(review, &reviewed_raw.into_keys().collect())
        .into_iter()
        .map(|cycle| cycle.iter().map(rename).collect())
        .collect();
    let cycles_added = cycles(head, &structure)
        .into_iter()
        .filter(|cycle| !old_cycles.iter().any(|old| cycle.is_subset(old)))
        .filter(|cycle| {
            dependencies_added
                .iter()
                .any(|d| cycle.contains(&d.src) && cycle.contains(&d.dst))
        })
        .collect();

    // Boxes of the head the review had, under the same id or another.
    let counterparts: HashSet<NodeId> = review
        .nodes()
        .filter(|n| is_box(n.kind))
        .map(|n| rename(&n.id))
        .collect();
    let (in_before, out_before) = fans(&reviewed_dependencies);
    let (in_after, out_after) = fans(&counted);
    let mut fan_in = BTreeMap::new();
    let mut fan_out = BTreeMap::new();
    for node in head
        .nodes()
        .filter(|n| is_box(n.kind) && counterparts.contains(&n.id))
    {
        let count = |fans: &HashMap<&NodeId, u32>| fans.get(&node.id).copied().unwrap_or(0);
        let (before, after) = (count(&in_before), count(&in_after));
        if jumped(before, after) {
            fan_in.insert(node.id.clone(), Jump { before, after });
        }
        let (before, after) = (count(&out_before), count(&out_after));
        if jumped(before, after) {
            fan_out.insert(node.id.clone(), Jump { before, after });
        }
    }

    ReviewDiff {
        changes,
        removed,
        regrouped,
        by_manifest: manifest.into_iter().collect(),
        changed_files: changed,
        renamed,
        dependencies_added,
        dependencies_removed,
        cycles_added,
        fan_in,
        fan_out,
    }
}

/// The pairs of nodes of `visible` that an edge of the head links now, and no edge of the
/// review linked, by a change of the code (Q28, Q36): the edges the map shows as new. Each edge
/// counts for the nearest visible ancestors of its ends, as on the map (`aggregate`).
pub fn new_links(
    review: &Graph,
    head: &Graph,
    diff: &ReviewDiff,
    visible: &HashSet<NodeId>,
) -> HashSet<(NodeId, NodeId)> {
    let manifest: HashSet<NodeId> = diff.by_manifest.iter().cloned().collect();
    let edges = Edges::new(
        review,
        head,
        &diff.changed_files,
        &diff.renamed,
        &manifest,
        &diff.regrouped,
    );
    let mut then = nearest_visible(review, visible, Some(&diff.renamed));
    let mut now = nearest_visible(head, visible, None);
    let before: HashSet<(NodeId, NodeId)> = review
        .edges()
        .iter()
        .filter_map(|edge| {
            let (src, dst) = (then(&edge.src)?, then(&edge.dst)?);
            (src != dst).then_some((src, dst))
        })
        .collect();
    let mut links: HashMap<(NodeId, NodeId), bool> = HashMap::new();
    for edge in head.edges() {
        let (Some(src), Some(dst)) = (now(&edge.src), now(&edge.dst)) else {
            continue;
        };
        if src == dst {
            continue;
        }
        // The line the review drew for it, when a move may have changed it (Q36, Q38).
        let moved = edges.kept_and_may_move(edge) && {
            let (from_src, from_dst) =
                (then(edges.was_of(&edge.src)), then(edges.was_of(&edge.dst)));
            edges.end_moves(&edge.src, from_src.as_ref(), Some(&src))
                || edges.end_moves(&edge.dst, from_dst.as_ref(), Some(&dst))
        };
        let counts = edges.carried(head, edge, Side::Head).changed || moved;
        *links.entry((src, dst)).or_default() |= counts;
    }
    links
        .into_iter()
        .filter(|(pair, counts)| *counts && !before.contains(pair))
        .map(|(pair, _)| pair)
        .collect()
}

/// The nearest visible ancestor of a node, the node first; under the head's ids with
/// `renamed`, for the review.
fn nearest_visible<'g>(
    graph: &'g Graph,
    visible: &'g HashSet<NodeId>,
    renamed: Option<&'g BTreeMap<NodeId, NodeId>>,
) -> impl FnMut(&NodeId) -> Option<NodeId> + 'g {
    let mut nearest: HashMap<NodeId, Option<NodeId>> = HashMap::new();
    move |id: &NodeId| {
        if let Some(found) = nearest.get(id) {
            return found.clone();
        }
        let found = graph
            .ancestors(id)
            .map(|a| renamed.and_then(|r| r.get(a)).unwrap_or(a).clone())
            .find(|a| visible.contains(a));
        nearest.insert(id.clone(), found.clone());
        found
    }
}

fn is_box(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::Component | NodeKind::Package)
}

fn changed_files(
    review: &Graph,
    head: &Graph,
    reviewed: &HashMap<String, ContentHash>,
    current: &HashMap<String, ContentHash>,
) -> BTreeSet<NodeId> {
    let files = |graph: &Graph| -> BTreeSet<NodeId> {
        graph
            .nodes()
            .filter(|n| n.kind == NodeKind::File)
            .map(|n| n.id.clone())
            .collect()
    };
    let (before, after) = (files(review), files(head));
    before
        .symmetric_difference(&after)
        .cloned()
        .chain(
            before
                .intersection(&after)
                .filter(|id| reviewed.get(id.as_str()) != current.get(id.as_str()))
                .cloned(),
        )
        .collect()
}

/// What the review had under another id (spec « Stable identity »). A file gone from one path
/// and found at one other with the same content is moved, and so are its symbols. Then a box of
/// the review gone from the head and a new box of the same kind are one, renamed or moved, when
/// each holds the majority of the other's files; each box once, the largest overlap first.
fn renamed(
    review: &Graph,
    head: &Graph,
    reviewed: &HashMap<String, ContentHash>,
    current: &HashMap<String, ContentHash>,
) -> BTreeMap<NodeId, NodeId> {
    let lonely = |graph: &Graph, other: &Graph, kind: fn(NodeKind) -> bool| -> Vec<NodeId> {
        let mut ids: Vec<NodeId> = graph
            .nodes()
            .filter(|n| kind(n.kind) && !other.contains(&n.id))
            .map(|n| n.id.clone())
            .collect();
        ids.sort();
        ids
    };
    let is_file = |kind: NodeKind| kind == NodeKind::File;
    let by_content = |files: Vec<NodeId>, hashes: &HashMap<String, ContentHash>| {
        let mut by: HashMap<ContentHash, Vec<NodeId>> = HashMap::new();
        for file in files {
            if let Some(hash) = hashes.get(file.as_str()) {
                by.entry(*hash).or_default().push(file);
            }
        }
        by
    };
    let gone = by_content(lonely(review, head, is_file), reviewed);
    let new = by_content(lonely(head, review, is_file), current);
    let mut renamed = BTreeMap::new();
    for (hash, from) in &gone {
        // One with this content on each side: a body shared by several is no evidence.
        if let ([from], Some([to])) = (from.as_slice(), new.get(hash).map(Vec::as_slice)) {
            renamed.insert(from.clone(), to.clone());
        }
    }
    let mut symbols: Vec<(NodeId, NodeId)> = renamed
        .iter()
        .flat_map(|(from, to)| moved_symbols(review, from, head, to))
        .collect();
    // A file that kept its path and its content, but whose symbols changed ids: it changed
    // crate, whose name its qualified names hold (ADR 0013; Q35, Q37).
    for file in head.nodes().filter(|n| n.kind == NodeKind::File) {
        let same = review.contains(&file.id)
            && reviewed
                .get(file.id.as_str())
                .is_some_and(|hash| current.get(file.id.as_str()) == Some(hash));
        if same && head.children(&file.id).iter().any(|c| !review.contains(c)) {
            symbols.extend(moved_symbols(review, &file.id, head, &file.id));
        }
    }
    // The files under each box, by the head's ids.
    let files_under = |graph: &Graph, map: &BTreeMap<NodeId, NodeId>| {
        let mut under: HashMap<NodeId, HashSet<NodeId>> = HashMap::new();
        for file in graph.nodes().filter(|n| n.kind == NodeKind::File) {
            let id = map.get(&file.id).unwrap_or(&file.id).clone();
            for above in graph.ancestors(&file.id).skip(1) {
                under.entry(above.clone()).or_default().insert(id.clone());
            }
        }
        under
    };
    let before = files_under(review, &renamed);
    let after = files_under(head, &BTreeMap::new());
    let new_boxes: HashSet<NodeId> = lonely(head, review, is_box).into_iter().collect();
    let mut pairs: Vec<(usize, NodeId, NodeId)> = Vec::new();
    for from in lonely(review, head, is_box) {
        let Some(old) = before.get(&from).filter(|f| !f.is_empty()) else {
            continue;
        };
        let kind = review.node(&from).map(|n| n.kind);
        // The new boxes of the same kind that hold its files now, and how many of them.
        let mut shared: HashMap<&NodeId, usize> = HashMap::new();
        for file in old {
            for above in head.ancestors(file).skip(1) {
                if new_boxes.contains(above) && head.node(above).map(|n| n.kind) == kind {
                    *shared.entry(above).or_default() += 1;
                }
            }
        }
        for (to, shared) in shared {
            let now = after.get(to).map_or(0, HashSet::len);
            if 2 * shared > old.len() && 2 * shared > now {
                pairs.push((shared, from.clone(), to.clone()));
            }
        }
    }
    pairs.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| (&a.1, &a.2).cmp(&(&b.1, &b.2))));
    let mut taken = HashSet::new();
    for (_, from, to) in pairs {
        if !renamed.contains_key(&from) && taken.insert(to.clone()) {
            renamed.insert(from, to);
        }
    }
    renamed.extend(symbols);
    renamed
}

/// The symbols of a file moved with the same content, paired by their place in it: the names
/// from the file down, and the span. Their ids hold the module path, which the move changed.
fn moved_symbols(
    review: &Graph,
    from: &NodeId,
    head: &Graph,
    to: &NodeId,
) -> Vec<(NodeId, NodeId)> {
    type Place = (Vec<(NodeKind, String)>, Option<crate::Span>);
    let places = |graph: &Graph, file: &NodeId| -> HashMap<Place, Vec<NodeId>> {
        let mut places: HashMap<Place, Vec<NodeId>> = HashMap::new();
        let mut queue: Vec<(&NodeId, Vec<(NodeKind, String)>)> = vec![(file, Vec::new())];
        while let Some((at, path)) = queue.pop() {
            for child in graph.children(at) {
                let Some(node) = graph.node(child) else {
                    continue;
                };
                let mut path = path.clone();
                path.push((node.kind, node.name.clone()));
                places
                    .entry((path.clone(), node.span))
                    .or_default()
                    .push(child.clone());
                queue.push((child, path));
            }
        }
        places
    };
    let after = places(head, to);
    places(review, from)
        .into_iter()
        .filter_map(
            |(place, old)| match (old.as_slice(), after.get(&place).map(Vec::as_slice)) {
                ([old], Some([new])) if old != new => Some((old.clone(), new.clone())),
                _ => None,
            },
        )
        .collect()
}

/// Which end of the comparison a graph is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Review,
    Head,
}

/// What the edges that carry a dependency, or a line of the map, say of it.
#[derive(Clone, Copy, Debug, Default)]
struct Carried {
    /// An edge the other end has not, that a change of the code explains (Q28, Q36).
    changed: bool,
    /// An edge the other end has too, or that a change of the code explains: not the language
    /// server's alone.
    real: bool,
}

impl std::ops::BitOrAssign for Carried {
    fn bitor_assign(&mut self, other: Self) {
        self.changed |= other.changed;
        self.real |= other.real;
    }
}

/// The edges of both ends under the head's ids, and what changed in the files.
struct Edges<'a> {
    graphs: (&'a Graph, &'a Graph),
    changed: &'a BTreeSet<NodeId>,
    renamed: &'a BTreeMap<NodeId, NodeId>,
    /// The review's id of what the head renamed.
    was: HashMap<&'a NodeId, &'a NodeId>,
    regrouped: &'a BTreeSet<NodeId>,
    moved: HashSet<NodeId>,
    /// What may move between boxes, by its ids at the head: the files moved to another folder
    /// (Q40), the packages their manifest made, undid or changed (not renamed), and their files
    /// (Q34, Q35, Q37).
    moving: HashSet<NodeId>,
    review: HashSet<(NodeId, NodeId)>,
    head: HashSet<(NodeId, NodeId)>,
}

impl<'a> Edges<'a> {
    fn new(
        review: &'a Graph,
        head: &'a Graph,
        changed: &'a BTreeSet<NodeId>,
        renamed: &'a BTreeMap<NodeId, NodeId>,
        manifest: &'a HashSet<NodeId>,
        regrouped: &'a BTreeSet<NodeId>,
    ) -> Self {
        let rename = |id: &NodeId| renamed.get(id).unwrap_or(id).clone();
        let was: HashMap<&NodeId, &NodeId> = renamed.iter().map(|(from, to)| (to, from)).collect();
        let moved = moved_files(review, renamed);
        // A file renamed in its folder does not move: a change of its component is the
        // regrouping's (Q40).
        fn folder(id: &NodeId) -> Option<&str> {
            id.as_str().rsplit_once('/').map(|(folder, _)| folder)
        }
        let mut moving: HashSet<NodeId> = renamed
            .iter()
            .filter(|(from, to)| moved.contains(*from) && folder(from) != folder(to))
            .map(|(_, to)| to.clone())
            .collect();
        // A package its manifest renamed is the same box (spec « Stable identity »): neither it
        // nor its files move.
        let renamed_package = |p: &NodeId| renamed.contains_key(p) || was.contains_key(p);
        for package in manifest.iter().filter(|p| !renamed_package(p)) {
            moving.insert(package.clone());
            // Its files under the head's ids, renamed in their folder or not (review n° 8, m-1).
            moving.extend(files_in(review, package).into_iter().map(rename));
            moving.extend(files_in(head, package).into_iter().cloned());
        }
        Self {
            graphs: (review, head),
            changed,
            renamed,
            was,
            regrouped,
            moved,
            moving,
            review: review
                .edges()
                .iter()
                .map(|e| (rename(&e.src), rename(&e.dst)))
                .collect(),
            head: head
                .edges()
                .iter()
                .map(|e| (e.src.clone(), e.dst.clone()))
                .collect(),
        }
    }

    /// The review's id of a node of the head.
    fn was_of<'n>(&'n self, id: &'n NodeId) -> &'n NodeId {
        self.was.get(id).copied().unwrap_or(id)
    }

    fn rename(&self, id: &NodeId) -> NodeId {
        self.renamed.get(id).unwrap_or(id).clone()
    }

    /// Whether a head's edge both ends have may have moved with an end: its file, or its
    /// package, may move.
    fn kept_and_may_move(&self, edge: &Edge) -> bool {
        let head = self.graphs.1;
        let may = |id: &NodeId| self.moving.contains(head.file_of(id).unwrap_or(id));
        (may(&edge.src) || may(&edge.dst))
            && self.review.contains(&(edge.src.clone(), edge.dst.clone()))
    }

    /// Whether an end of a head's edge both ends have moved, from where the review placed it
    /// (`before`, under the head's ids) to where the head does (`after`): its file moved, or
    /// the package that holds it changed by its manifest (Q34, Q35, Q37); neither the box it
    /// left nor the one it joined is a regrouping (Q38); and, its package staying where it
    /// was, not above that package, where only the package's regrouping moved it.
    fn end_moves(&self, end: &NodeId, before: Option<&NodeId>, after: Option<&NodeId>) -> bool {
        if before == after {
            return false;
        }
        let (review, head) = self.graphs;
        let element = head.file_of(end).unwrap_or(end);
        if !self.moving.contains(element) {
            return false;
        }
        if [before, after]
            .into_iter()
            .flatten()
            .any(|b| self.regrouped.contains(b))
        {
            return false;
        }
        let package = |graph: &Graph, id: &NodeId| {
            graph
                .ancestors(id)
                .find(|a| graph.node(a).is_some_and(|n| n.kind == NodeKind::Package))
                .cloned()
        };
        match (
            package(review, self.was_of(element)),
            package(head, element),
        ) {
            (Some(then), Some(now)) if self.rename(&then) == now && !self.moving.contains(&now) => {
                let above_then = review
                    .ancestors(&then)
                    .skip(1)
                    .any(|a| Some(&self.rename(a)) == before);
                let above_now = head.ancestors(&now).skip(1).any(|a| Some(a) == after);
                !above_then && !above_now
            }
            _ => true,
        }
    }

    /// The dependencies a move changed (Q36, Q38): for each edge both ends have, whose end
    /// moved, the dependency it carried at the review and the one it carries at the head,
    /// under the head's ids.
    fn moved_dependencies(&self) -> (HashSet<Dependency>, HashSet<Dependency>) {
        let (review, head) = self.graphs;
        let mut before = HashSet::new();
        let mut after = HashSet::new();
        let boxes = |graph: &Graph, (src, dst): (&NodeId, &NodeId)| {
            [src, dst]
                .iter()
                .all(|id| graph.node(id).is_some_and(|n| is_box(n.kind)))
        };
        for edge in head.edges() {
            if !self.kept_and_may_move(edge) {
                continue;
            }
            let then = siblings(review, self.was_of(&edge.src), self.was_of(&edge.dst));
            let now = siblings(head, &edge.src, &edge.dst);
            let then_ids = then.map(|(src, dst)| (self.rename(src), self.rename(dst)));
            let now_ids = now.map(|(src, dst)| (src.clone(), dst.clone()));
            if then_ids == now_ids {
                continue;
            }
            let moved = self.end_moves(
                &edge.src,
                then_ids.as_ref().map(|p| &p.0),
                now_ids.as_ref().map(|p| &p.0),
            ) || self.end_moves(
                &edge.dst,
                then_ids.as_ref().map(|p| &p.1),
                now_ids.as_ref().map(|p| &p.1),
            );
            if !moved {
                continue;
            }
            if let (Some(pair), Some((src, dst))) = (then, then_ids)
                && boxes(review, pair)
            {
                before.insert(Dependency { src, dst });
            }
            if let (Some(pair), Some((src, dst))) = (now, now_ids)
                && boxes(head, pair)
            {
                after.insert(Dependency { src, dst });
            }
        }
        (before, after)
    }

    /// What an edge of one end says. A change of the code explains it when an edited, new or
    /// deleted file holds one of its ends, a move being no edit; an edge with no end in a file
    /// (between packages, from the manifests) always.
    fn carried(&self, graph: &Graph, edge: &Edge, side: Side) -> Carried {
        let rename = |id: &NodeId| self.renamed.get(id).unwrap_or(id).clone();
        let (key, other) = match side {
            Side::Review => ((rename(&edge.src), rename(&edge.dst)), &self.head),
            Side::Head => ((edge.src.clone(), edge.dst.clone()), &self.review),
        };
        let files: Vec<&NodeId> = [graph.file_of(&edge.src), graph.file_of(&edge.dst)]
            .into_iter()
            .flatten()
            .collect();
        let explained = files.is_empty()
            || files
                .iter()
                .any(|f| self.changed.contains(*f) && !self.moved.contains(*f));
        let kept = other.contains(&key);
        Carried {
            changed: !kept && explained,
            real: kept || explained,
        }
    }
}

/// The files moved, at both of their paths.
fn moved_files(review: &Graph, renamed: &BTreeMap<NodeId, NodeId>) -> HashSet<NodeId> {
    renamed
        .iter()
        .filter(|(from, _)| review.node(from).is_some_and(|n| n.kind == NodeKind::File))
        .flat_map(|(from, to)| [from.clone(), to.clone()])
        .collect()
}

/// The boxes a regrouping alone made or undid (Q33): new at the head and not added, gone from
/// it and not removed.
fn regrouped(
    review: &Graph,
    head: &Graph,
    renamed: &BTreeMap<NodeId, NodeId>,
    changes: &BTreeMap<NodeId, Change>,
    removed: &BTreeSet<NodeId>,
) -> BTreeSet<NodeId> {
    let targets: HashSet<&NodeId> = renamed.values().collect();
    let made = head.nodes().filter(|n| {
        is_box(n.kind)
            && !review.contains(&n.id)
            && !targets.contains(&n.id)
            && changes.get(&n.id) != Some(&Change::Added)
    });
    let undone = review.nodes().filter(|n| {
        is_box(n.kind)
            && !head.contains(&n.id)
            && !renamed.contains_key(&n.id)
            && !removed.contains(&n.id)
    });
    made.chain(undone).map(|n| n.id.clone()).collect()
}

/// The children of the nearest common ancestor of `a` and `b` that hold them; `None` when one
/// holds the other, or one is not in the graph (a dangling edge, ADR 0009).
fn siblings<'g>(graph: &'g Graph, a: &NodeId, b: &NodeId) -> Option<(&'g NodeId, &'g NodeId)> {
    let above_a: Vec<&NodeId> = graph.ancestors(a).collect();
    let mut previous = None;
    for current in graph.ancestors(b) {
        if let Some(at) = above_a.iter().position(|n| *n == current) {
            // `b` holds `a`, or `a` holds `b`: no siblings.
            return Some((*above_a.get(at.checked_sub(1)?)?, previous?));
        }
        previous = Some(current);
    }
    None
}

/// Every dependency between sibling boxes, and what the edges that carry it say.
fn dependencies(graph: &Graph, edges: &Edges, side: Side) -> BTreeMap<Dependency, Carried> {
    let mut dependencies: BTreeMap<Dependency, Carried> = BTreeMap::new();
    for edge in graph.edges() {
        let Some((src, dst)) = siblings(graph, &edge.src, &edge.dst) else {
            continue;
        };
        let boxes = [src, dst]
            .iter()
            .all(|id| graph.node(id).is_some_and(|n| is_box(n.kind)));
        if !boxes {
            continue;
        }
        let dependency = Dependency {
            src: src.clone(),
            dst: dst.clone(),
        };
        *dependencies.entry(dependency).or_default() |= edges.carried(graph, edge, side);
    }
    dependencies
}

/// The strongly connected sets of at least two boxes, among the children of each parent.
fn cycles(graph: &Graph, dependencies: &BTreeSet<Dependency>) -> Vec<BTreeSet<NodeId>> {
    let parent = |id: &NodeId| graph.parents().get(id).cloned().flatten();
    let mut by_parent: BTreeMap<Option<NodeId>, Vec<&Dependency>> = BTreeMap::new();
    for dependency in dependencies {
        let (src, dst) = (parent(&dependency.src), parent(&dependency.dst));
        // Put back from the review, two boxes may no longer be siblings.
        if src == dst && graph.contains(&dependency.src) && graph.contains(&dependency.dst) {
            by_parent.entry(src).or_default().push(dependency);
        }
    }
    let mut cycles = Vec::new();
    for edges in by_parent.values() {
        cycles.extend(
            strongly_connected(edges)
                .into_iter()
                .filter(|component| component.len() >= 2),
        );
    }
    cycles.sort();
    cycles
}

/// Tarjan's algorithm, without recursion: the strongly connected components of the graph the
/// dependencies make.
fn strongly_connected(edges: &[&Dependency]) -> Vec<BTreeSet<NodeId>> {
    let mut vertices: Vec<&NodeId> = edges.iter().flat_map(|d| [&d.src, &d.dst]).collect();
    vertices.sort();
    vertices.dedup();
    let index_of: HashMap<&NodeId, usize> =
        vertices.iter().enumerate().map(|(i, v)| (*v, i)).collect();
    let mut successors: Vec<Vec<usize>> = vec![Vec::new(); vertices.len()];
    for dependency in edges {
        successors[index_of[&dependency.src]].push(index_of[&dependency.dst]);
    }
    let count = vertices.len();
    let mut index = vec![usize::MAX; count];
    let mut low = vec![0; count];
    let mut on_stack = vec![false; count];
    let mut stack = Vec::new();
    let mut next = 0;
    let mut components = Vec::new();
    for start in 0..count {
        if index[start] != usize::MAX {
            continue;
        }
        // (vertex, next successor to look at)
        let mut work = vec![(start, 0)];
        index[start] = next;
        low[start] = next;
        next += 1;
        stack.push(start);
        on_stack[start] = true;
        while let Some(&mut (vertex, ref mut at)) = work.last_mut() {
            if let Some(&successor) = successors[vertex].get(*at) {
                *at += 1;
                if index[successor] == usize::MAX {
                    index[successor] = next;
                    low[successor] = next;
                    next += 1;
                    stack.push(successor);
                    on_stack[successor] = true;
                    work.push((successor, 0));
                } else if on_stack[successor] {
                    low[vertex] = low[vertex].min(index[successor]);
                }
                continue;
            }
            work.pop();
            if let Some(&(parent, _)) = work.last() {
                low[parent] = low[parent].min(low[vertex]);
            }
            if low[vertex] == index[vertex] {
                let mut component = BTreeSet::new();
                while let Some(member) = stack.pop() {
                    on_stack[member] = false;
                    component.insert(vertices[member].clone());
                    if member == vertex {
                        break;
                    }
                }
                components.push(component);
            }
        }
    }
    components
}

/// How many distinct boxes depend on each box, and how many it depends on.
fn fans(dependencies: &BTreeSet<Dependency>) -> (HashMap<&NodeId, u32>, HashMap<&NodeId, u32>) {
    let mut fan_in: HashMap<&NodeId, u32> = HashMap::new();
    let mut fan_out: HashMap<&NodeId, u32> = HashMap::new();
    for dependency in dependencies {
        *fan_in.entry(&dependency.dst).or_default() += 1;
        *fan_out.entry(&dependency.src).or_default() += 1;
    }
    (fan_in, fan_out)
}

/// The nodes that hold a file.
fn holding(graph: &Graph) -> HashSet<NodeId> {
    graph
        .nodes()
        .filter(|n| n.kind == NodeKind::File)
        .flat_map(|n| graph.ancestors(&n.id).skip(1).cloned())
        .collect()
}

/// The files under a node.
fn files_in<'g>(graph: &'g Graph, id: &NodeId) -> Vec<&'g NodeId> {
    let mut files = Vec::new();
    let mut queue = vec![id];
    while let Some(at) = queue.pop() {
        for child in graph.children(at) {
            if graph.node(child).is_some_and(|n| n.kind == NodeKind::File) {
                files.push(child);
            } else {
                queue.push(child);
            }
        }
    }
    files
}

/// The packages a manifest made, undid or changed (Q34, Q35): new, gone, renamed, or that hold
/// files at one end only (internal at one, external at the other). Their ids at both ends.
fn by_manifest(
    review: &Graph,
    head: &Graph,
    renamed: &BTreeMap<NodeId, NodeId>,
    (held_before, held): &(HashSet<NodeId>, HashSet<NodeId>),
) -> HashSet<NodeId> {
    let was: HashMap<&NodeId, &NodeId> = renamed.iter().map(|(from, to)| (to, from)).collect();
    let mut packages = HashSet::new();
    for package in head.nodes().filter(|n| n.kind == NodeKind::Package) {
        let id = &package.id;
        let before = if review.contains(id) {
            Some(id)
        } else {
            was.get(id).copied()
        };
        match before {
            None => {
                packages.insert(id.clone());
            }
            Some(old) if old != id || held_before.contains(old) != held.contains(id) => {
                packages.insert(id.clone());
                packages.insert(old.clone());
            }
            Some(_) => {}
        }
    }
    packages.extend(
        review
            .nodes()
            .filter(|n| {
                n.kind == NodeKind::Package && !head.contains(&n.id) && !renamed.contains_key(&n.id)
            })
            .map(|n| n.id.clone()),
    );
    packages
}

/// Boxes and files of the head that are new or changed, and boxes of the review gone. Only a
/// change of the code counts (Q30, Q33): a box is new when it brings a new file, or holds no
/// file (an external package, a category of them: it comes with the manifests); gone when one
/// of its files was deleted, or it held none. A package comes and goes with its manifest, which
/// is code (Q35): new when it is, gone when it is, unless renamed. Otherwise a box, new, gone or
/// kept, is a regrouping: modified when it holds a changed file (edited or moved), at the
/// review or now. A package changed by its manifest (renamed, or that lost or gained all its
/// files, Q34) is modified; it counts, as a file does, for the boxes that hold it, and its
/// files, as moved ones, for the boxes they left or joined (Q37). A file moved is modified.
fn changes(
    review: &Graph,
    head: &Graph,
    changed: &BTreeSet<NodeId>,
    renamed: &BTreeMap<NodeId, NodeId>,
    (held_before, held): &(HashSet<NodeId>, HashSet<NodeId>),
    manifest: &HashSet<NodeId>,
) -> (BTreeMap<NodeId, Change>, BTreeSet<NodeId>) {
    let rename = |id: &NodeId| renamed.get(id).unwrap_or(id).clone();
    let was: HashMap<NodeId, NodeId> = renamed
        .iter()
        .map(|(from, to)| (to.clone(), from.clone()))
        .collect();
    let before_of = |id: &NodeId| -> Option<NodeId> {
        if review.contains(id) {
            Some(id.clone())
        } else {
            was.get(id).cloned()
        }
    };
    let mut touched: HashSet<NodeId> = HashSet::new();
    // The boxes code came into, and those code went from: a file new, or deleted, not moved; a
    // package new or gone, with its manifest.
    let mut came: HashSet<&NodeId> = HashSet::new();
    let mut went: HashSet<&NodeId> = HashSet::new();
    for file in changed {
        touched.extend(head.ancestors(file).cloned());
        touched.extend(review.ancestors(file).map(rename));
        if !review.contains(file) && !was.contains_key(file) {
            came.extend(head.ancestors(file));
        }
        if !head.contains(file) && !renamed.contains_key(file) {
            went.extend(review.ancestors(file));
        }
    }
    for package in head.nodes().filter(|n| manifest.contains(&n.id)) {
        let old = before_of(&package.id);
        if old.is_none() {
            came.extend(head.ancestors(&package.id));
        }
        touched.extend(head.ancestors(&package.id).cloned());
        if let Some(old) = &old {
            touched.extend(review.ancestors(old).map(rename));
        }
        // Its files: the boxes they left (Q37).
        for file in files_in(head, &package.id) {
            if let Some(was) = before_of(file) {
                touched.extend(review.ancestors(&was).skip(1).map(rename));
            }
        }
    }
    for package in review.nodes().filter(|n| manifest.contains(&n.id)) {
        if !head.contains(&package.id) && !renamed.contains_key(&package.id) {
            went.extend(review.ancestors(&package.id));
            touched.extend(review.ancestors(&package.id).map(rename));
        }
        // Its files: the boxes they joined (Q37).
        for file in files_in(review, &package.id) {
            touched.extend(head.ancestors(&rename(file)).skip(1).cloned());
        }
    }
    let mut changes = BTreeMap::new();
    for node in head.nodes() {
        let file = node.kind == NodeKind::File;
        if !is_box(node.kind) && !file {
            continue;
        }
        let change = match before_of(&node.id) {
            None if file => Some(Change::Added),
            None if came.contains(&node.id) || !held.contains(&node.id) => Some(Change::Added),
            // A file moved is changed: its new path was not at the review.
            Some(_) if file => changed.contains(&node.id).then_some(Change::Modified),
            _ => touched.contains(&node.id).then_some(Change::Modified),
        };
        if let Some(change) = change {
            changes.insert(node.id.clone(), change);
        }
    }
    let removed = review
        .nodes()
        .filter(|n| is_box(n.kind) && !head.contains(&n.id) && !renamed.contains_key(&n.id))
        .filter(|n| went.contains(&n.id) || !held_before.contains(&n.id))
        .map(|n| n.id.clone())
        .collect();
    (changes, removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Confidence, EdgeKind, Node, Provenance, Role, SymbolKind};

    fn id(s: &str) -> NodeId {
        NodeId::parse(s).unwrap()
    }

    /// A small repository, as fixtures of the review and of the head: a node per line, with
    /// its kind and parent.
    #[derive(Clone, Default)]
    struct Fixture {
        nodes: Vec<Node>,
        edges: Vec<Edge>,
        contents: HashMap<String, ContentHash>,
    }

    impl Fixture {
        /// repo r → packages a, b, c → each a file `<p>/src/lib.rs` with a function `<p>::f`.
        fn new() -> Self {
            let mut fixture = Self::default();
            fixture.node("repo:r", NodeKind::Repo, None);
            for p in ["a", "b", "c"] {
                fixture.package(p);
            }
            fixture
        }

        fn node(&mut self, i: &str, kind: NodeKind, parent: Option<&str>) -> &mut Self {
            let name = i.rsplit([':', '/']).next().unwrap_or(i);
            let mut node = Node::new(id(i), kind, name);
            if let Some(parent) = parent {
                node = node.with_parent(id(parent));
            }
            self.nodes.retain(|n| n.id != node.id);
            self.nodes.push(node);
            self
        }

        fn package(&mut self, p: &str) -> &mut Self {
            self.node(&format!("pkg:cargo/{p}"), NodeKind::Package, Some("repo:r"));
            self.file(&format!("{p}/src/lib.rs"), &format!("pkg:cargo/{p}"))
        }

        fn file(&mut self, path: &str, parent: &str) -> &mut Self {
            let file = format!("file:{path}");
            self.node(&file, NodeKind::File, Some(parent));
            let function = format!("rust:{}::f", path.replace('/', "_"));
            self.node(
                &function,
                NodeKind::Symbol(SymbolKind::Function),
                Some(&file),
            );
            self.contents.insert(file, hash(path, 0));
            self
        }

        fn remove(&mut self, i: &str) -> &mut Self {
            let gone = id(i);
            self.nodes
                .retain(|n| n.id != gone && n.parent.as_ref() != Some(&gone));
            self.contents.remove(i);
            self
        }

        /// The file's content changes.
        fn edit(&mut self, path: &str, version: u8) -> &mut Self {
            self.contents
                .insert(format!("file:{path}"), hash(path, version));
            self
        }

        fn edge(
            &mut self,
            src: &str,
            dst: &str,
            kind: EdgeKind,
            provenance: Provenance,
        ) -> &mut Self {
            self.edges.push(Edge {
                src: id(src),
                dst: id(dst),
                kind,
                provenance,
                confidence: Confidence::CERTAIN,
                weight: 1,
            });
            self
        }

        /// A call from package `a`'s file to package `b`'s.
        fn call(&mut self, a: &str, b: &str) -> &mut Self {
            let file = |p: &str| format!("file:{p}/src/lib.rs");
            self.edge(&file(a), &file(b), EdgeKind::Calls, Provenance::TreeSitter)
        }

        /// `git mv`: the file at another path under `parent`, its content and its function the
        /// same, its edges with it.
        fn move_file(&mut self, from: &str, to: &str, parent: &str) -> &mut Self {
            let content = self.contents[&format!("file:{from}")];
            self.remove(&format!("file:{from}")).file(to, parent);
            self.contents.insert(format!("file:{to}"), content);
            let function = |path: &str| format!("rust:{}::f", path.replace('/', "_"));
            let renames = [
                (format!("file:{from}"), format!("file:{to}")),
                (function(from), function(to)),
            ];
            for edge in &mut self.edges {
                for (old, new) in &renames {
                    if edge.src.as_str() == old {
                        edge.src = id(new);
                    }
                    if edge.dst.as_str() == old {
                        edge.dst = id(new);
                    }
                }
            }
            self.nodes.retain(|n| n.id.as_str() != function(from));
            self
        }

        fn unlink(&mut self, src: &str, dst: &str) -> &mut Self {
            self.edges
                .retain(|e| !(e.src.as_str() == src && e.dst.as_str() == dst));
            self
        }

        fn graph(&self) -> Graph {
            Graph::new(self.nodes.clone(), self.edges.clone())
        }
    }

    fn hash(path: &str, version: u8) -> ContentHash {
        let mut bytes = [version; 32];
        for (i, b) in path.bytes().enumerate().take(31) {
            bytes[i + 1] = b;
        }
        ContentHash::from_bytes(bytes)
    }

    fn diff(review: &Fixture, head: &Fixture) -> ReviewDiff {
        compare(
            &review.graph(),
            &head.graph(),
            &review.contents,
            &head.contents,
        )
    }

    fn ids<'a>(set: impl IntoIterator<Item = &'a NodeId>) -> Vec<&'a str> {
        set.into_iter().map(NodeId::as_str).collect()
    }

    fn dependency(src: &str, dst: &str) -> Dependency {
        Dependency {
            src: id(src),
            dst: id(dst),
        }
    }

    #[test]
    fn nothing_changed_is_an_empty_diff() {
        let mut review = Fixture::new();
        review.call("a", "b");
        assert_eq!(diff(&review, &review.clone()), ReviewDiff::default());
    }

    #[test]
    fn boxes_come_and_go_by_their_ids() {
        let review = Fixture::new();
        let mut head = review.clone();
        head.remove("pkg:cargo/c")
            .remove("file:c/src/lib.rs")
            .package("d");
        let diff = diff(&review, &head);
        assert_eq!(diff.changes[&id("pkg:cargo/d")], Change::Added);
        assert_eq!(diff.changes[&id("file:d/src/lib.rs")], Change::Added);
        assert_eq!(ids(&diff.removed), ["pkg:cargo/c"]);
        assert_eq!(
            ids(&diff.changed_files),
            ["file:c/src/lib.rs", "file:d/src/lib.rs"]
        );
        assert!(!diff.changes.contains_key(&id("pkg:cargo/a")));
    }

    #[test]
    fn a_changed_file_marks_its_boxes_up_to_the_top() {
        let mut review = Fixture::new();
        review
            .node(
                "component:dir/a/src/x",
                NodeKind::Component,
                Some("pkg:cargo/a"),
            )
            .file("a/src/x/one.rs", "component:dir/a/src/x")
            .file("a/src/x/two.rs", "component:dir/a/src/x");
        let mut head = review.clone();
        head.edit("a/src/x/one.rs", 1);
        let diff = diff(&review, &head);
        let changes: Vec<(&str, Change)> =
            diff.changes.iter().map(|(i, c)| (i.as_str(), *c)).collect();
        assert_eq!(
            changes,
            [
                ("component:dir/a/src/x", Change::Modified),
                ("file:a/src/x/one.rs", Change::Modified),
                ("pkg:cargo/a", Change::Modified)
            ]
        );
        // A file gone: its boxes at the review, the package above included.
        let mut head = review.clone();
        head.remove("file:a/src/x/two.rs");
        let diff = super::compare(
            &review.graph(),
            &head.graph(),
            &review.contents,
            &head.contents,
        );
        assert_eq!(
            ids(diff.changes.keys()),
            ["component:dir/a/src/x", "pkg:cargo/a"]
        );
    }

    #[test]
    fn a_moved_file_modifies_both_boxes() {
        let mut review = Fixture::new();
        review
            .node(
                "component:dir/a/src/x",
                NodeKind::Component,
                Some("pkg:cargo/a"),
            )
            .node(
                "component:dir/a/src/y",
                NodeKind::Component,
                Some("pkg:cargo/a"),
            )
            .file("a/src/x/one.rs", "component:dir/a/src/x")
            .file("a/src/x/two.rs", "component:dir/a/src/x")
            .file("a/src/y/three.rs", "component:dir/a/src/y");
        let mut head = review.clone();
        // The same content, at another path: moved, not new.
        head.remove("file:a/src/x/two.rs")
            .file("a/src/y/two.rs", "component:dir/a/src/y");
        head.contents.insert(
            "file:a/src/y/two.rs".into(),
            review.contents["file:a/src/x/two.rs"],
        );
        let diff = diff(&review, &head);
        assert_eq!(diff.changes[&id("component:dir/a/src/x")], Change::Modified);
        assert_eq!(diff.changes[&id("component:dir/a/src/y")], Change::Modified);
        assert_eq!(diff.changes[&id("file:a/src/y/two.rs")], Change::Modified);
        assert_eq!(
            diff.renamed.get(&id("file:a/src/x/two.rs")),
            Some(&id("file:a/src/y/two.rs"))
        );
        // A role that changed on unchanged code is no change of the code (Q30).
        let mut roled = review.clone();
        let b = roled
            .nodes
            .iter_mut()
            .find(|n| n.id.as_str() == "pkg:cargo/b")
            .unwrap();
        *b = b.clone().with_role(Role::Persistence);
        let diff = super::compare(
            &review.graph(),
            &roled.graph(),
            &review.contents,
            &roled.contents,
        );
        assert!(diff.changes.is_empty(), "{diff:?}");
    }

    #[test]
    fn a_dependency_counts_when_a_change_of_the_code_explains_it() {
        let review = Fixture::new();
        // The language server alone found a call: no file changed (Q28).
        let mut verified = review.clone();
        verified.edge(
            "rust:a_src_lib.rs::f",
            "rust:b_src_lib.rs::f",
            EdgeKind::Calls,
            Provenance::Lsp,
        );
        assert!(diff(&review, &verified).dependencies_added.is_empty());
        // The same call, written since.
        verified.edit("a/src/lib.rs", 1);
        let diff = diff(&review, &verified);
        assert_eq!(
            diff.dependencies_added,
            BTreeSet::from([dependency("pkg:cargo/a", "pkg:cargo/b")])
        );
    }

    #[test]
    fn a_dependency_gone_counts_when_a_change_of_the_code_explains_it() {
        let mut review = Fixture::new();
        review.call("a", "b");
        let mut head = review.clone();
        head.unlink("file:a/src/lib.rs", "file:b/src/lib.rs");
        // The language server contradicted tree-sitter: no file changed.
        assert!(diff(&review, &head).dependencies_removed.is_empty());
        head.edit("b/src/lib.rs", 1);
        assert_eq!(
            diff(&review, &head).dependencies_removed,
            BTreeSet::from([dependency("pkg:cargo/a", "pkg:cargo/b")])
        );
    }

    #[test]
    fn a_dependency_between_packages_always_counts() {
        let review = Fixture::new();
        let mut head = review.clone();
        head.edge(
            "pkg:cargo/a",
            "pkg:cargo/c",
            EdgeKind::DependsOn,
            Provenance::Manifest,
        );
        assert_eq!(
            diff(&review, &head).dependencies_added,
            BTreeSet::from([dependency("pkg:cargo/a", "pkg:cargo/c")])
        );
    }

    #[test]
    fn only_siblings_depend_on_each_other() {
        let mut review = Fixture::new();
        review
            .node(
                "component:dir/a/src/x",
                NodeKind::Component,
                Some("pkg:cargo/a"),
            )
            .file("a/src/x/one.rs", "component:dir/a/src/x");
        let mut head = review.clone();
        head.edit("a/src/x/one.rs", 1)
            // Inside a, to b: a depends on b, not the component.
            .edge(
                "file:a/src/x/one.rs",
                "file:b/src/lib.rs",
                EdgeKind::Imports,
                Provenance::TreeSitter,
            )
            // From a file to the component beside it: the file is no box.
            .edge(
                "file:a/src/lib.rs",
                "file:a/src/x/one.rs",
                EdgeKind::Imports,
                Provenance::TreeSitter,
            )
            // Dangling (ADR 0009).
            .edge(
                "file:a/src/x/one.rs",
                "file:gone/src/lib.rs",
                EdgeKind::Calls,
                Provenance::TreeSitter,
            );
        assert_eq!(
            diff(&review, &head).dependencies_added,
            BTreeSet::from([dependency("pkg:cargo/a", "pkg:cargo/b")])
        );
    }

    #[test]
    fn a_cycle_added_is_found_once_and_an_old_one_is_not_new() {
        let mut review = Fixture::new();
        review.call("a", "b");
        let mut head = review.clone();
        head.call("b", "c").call("c", "a").edit("c/src/lib.rs", 1);
        let diff = diff(&review, &head);
        let cycles: Vec<Vec<&str>> = diff.cycles_added.iter().map(ids).collect();
        assert_eq!(cycles, [["pkg:cargo/a", "pkg:cargo/b", "pkg:cargo/c"]]);
        assert!(diff.cycle_of(&id("pkg:cargo/b")).is_some());
        // Already there at the review: not added.
        let since = super::compare(&head.graph(), &head.graph(), &head.contents, &head.contents);
        assert!(since.cycles_added.is_empty(), "{since:?}");
    }

    #[test]
    fn a_cycle_that_grows_is_added_and_one_the_server_alone_closed_is_not() {
        let mut review = Fixture::new();
        review.call("a", "b").call("b", "a");
        let mut head = review.clone();
        head.call("b", "c").call("c", "b").edit("c/src/lib.rs", 1);
        assert_eq!(diff(&review, &head).cycles_added.len(), 1);
        // Found by the language server alone, on unchanged files.
        let mut verified = review.clone();
        verified.edge(
            "rust:b_src_lib.rs::f",
            "rust:c_src_lib.rs::f",
            EdgeKind::Calls,
            Provenance::Lsp,
        );
        verified.edge(
            "rust:c_src_lib.rs::f",
            "rust:b_src_lib.rs::f",
            EdgeKind::Calls,
            Provenance::Lsp,
        );
        assert!(diff(&review, &verified).cycles_added.is_empty());
        // With d: c calls d, and d, edited, calls b since. The cycle needs b → c, which the
        // server alone found on unchanged files: no cycle the code made (Q28).
        let mut review = Fixture::new();
        review.package("d").call("c", "d");
        let mut half = review.clone();
        half.edge(
            "rust:b_src_lib.rs::f",
            "rust:c_src_lib.rs::f",
            EdgeKind::Calls,
            Provenance::Lsp,
        )
        .call("d", "b")
        .edit("d/src/lib.rs", 1);
        let half = diff(&review, &half);
        assert_eq!(
            half.dependencies_added,
            BTreeSet::from([dependency("pkg:cargo/d", "pkg:cargo/b")])
        );
        assert!(half.cycles_added.is_empty(), "{:?}", half.cycles_added);
    }

    #[test]
    fn a_fan_in_jumps_by_three_and_by_half() {
        let mut review = Fixture::new();
        for p in ["d", "e", "f", "g", "h"] {
            review.package(p);
        }
        review.call("b", "a").call("c", "a");
        let mut head = review.clone();
        for p in ["d", "e", "f"] {
            head.call(p, "a").edit(&format!("{p}/src/lib.rs"), 1);
        }
        let diff = diff(&review, &head);
        assert_eq!(
            diff.fan_in.get(&id("pkg:cargo/a")),
            Some(&Jump {
                before: 2,
                after: 5
            })
        );
        assert!(diff.fan_out.is_empty(), "each file depends on one more");
        // 4 → 6: two more only.
        let mut review = Fixture::new();
        for p in ["d", "e", "f", "g"] {
            review.package(p);
        }
        review
            .call("b", "a")
            .call("c", "a")
            .call("d", "a")
            .call("e", "a");
        let mut head = review.clone();
        head.call("f", "a")
            .call("g", "a")
            .edit("f/src/lib.rs", 1)
            .edit("g/src/lib.rs", 1);
        assert!(
            super::compare(
                &review.graph(),
                &head.graph(),
                &review.contents,
                &head.contents
            )
            .fan_in
            .is_empty()
        );
        assert!(jumped(0, 3) && !jumped(0, 2) && !jumped(4, 6) && jumped(4, 7) && jumped(2, 5));
        // Three more, but not half as many again.
        assert!(!jumped(10, 13) && jumped(10, 15));
    }

    #[test]
    fn what_the_server_alone_took_away_still_counts_for_the_jumps() {
        let mut review = Fixture::new();
        for p in ["d", "e", "f"] {
            review.package(p);
        }
        review.call("b", "a").call("c", "a");
        let mut head = review.clone();
        // The server contradicted c → a, on unchanged files; d, e and f call a since.
        head.unlink("file:c/src/lib.rs", "file:a/src/lib.rs");
        for p in ["d", "e", "f"] {
            head.call(p, "a").edit(&format!("{p}/src/lib.rs"), 1);
        }
        assert_eq!(
            diff(&review, &head).fan_in.get(&id("pkg:cargo/a")),
            Some(&Jump {
                before: 2,
                after: 5
            })
        );
    }

    #[test]
    fn a_cycle_that_shrinks_is_no_cycle_added() {
        let mut review = Fixture::new();
        review.call("a", "b").call("b", "c").call("c", "a");
        let mut head = review.clone();
        head.unlink("file:c/src/lib.rs", "file:a/src/lib.rs")
            .call("b", "a")
            .edit("c/src/lib.rs", 1)
            .edit("b/src/lib.rs", 1);
        let diff = diff(&review, &head);
        assert!(diff.cycles_added.is_empty(), "{:?}", diff.cycles_added);
    }

    /// Review n° 5 of Phase 5, m-1: for the cycles, what the server alone took away is put
    /// back (Q28), and what the code took away is not.
    #[test]
    fn a_cycle_counts_what_the_code_took_and_not_what_the_server_took() {
        // a calls b's lib.rs; b has a second file, two.rs.
        let mut review = Fixture::new();
        review.file("b/src/two.rs", "pkg:cargo/b").call("a", "b");
        // The server contradicts a → b on unchanged files; two.rs, edited, calls a: a cycle.
        let mut head = review.clone();
        head.unlink("file:a/src/lib.rs", "file:b/src/lib.rs")
            .edge(
                "file:b/src/two.rs",
                "file:a/src/lib.rs",
                EdgeKind::Calls,
                Provenance::TreeSitter,
            )
            .edit("b/src/two.rs", 1);
        let diff = diff(&review, &head);
        let cycles: Vec<Vec<&str>> = diff.cycles_added.iter().map(ids).collect();
        assert_eq!(cycles, [["pkg:cargo/a", "pkg:cargo/b"]]);
        // a, edited, no longer calls b: no cycle.
        head.edit("a/src/lib.rs", 1);
        let edited = super::compare(
            &review.graph(),
            &head.graph(),
            &review.contents,
            &head.contents,
        );
        assert!(edited.cycles_added.is_empty(), "{:?}", edited.cycles_added);
    }

    #[test]
    fn a_regrouping_of_unchanged_files_is_no_change() {
        let mut review = Fixture::new();
        review
            .node(
                "component:cluster/a/src/one.rs",
                NodeKind::Component,
                Some("pkg:cargo/a"),
            )
            .node(
                "component:cluster/a/src/three.rs",
                NodeKind::Component,
                Some("pkg:cargo/a"),
            )
            .file("a/src/one.rs", "component:cluster/a/src/one.rs")
            .file("a/src/two.rs", "component:cluster/a/src/one.rs")
            .file("a/src/three.rs", "component:cluster/a/src/three.rs")
            .file("a/src/four.rs", "component:cluster/a/src/three.rs");
        let mut head = review.clone();
        // Clustering put two.rs with three.rs, their contents unchanged (Q30).
        head.node(
            "file:a/src/two.rs",
            NodeKind::File,
            Some("component:cluster/a/src/three.rs"),
        );
        let diff = diff(&review, &head);
        assert!(diff.changed_files.is_empty());
        assert!(diff.changes.is_empty(), "{diff:?}");
        // A level split into components, then flat again: nothing new, nothing gone.
        let mut flat = Fixture::new();
        for file in ["one", "two", "three", "four"] {
            flat.file(&format!("a/src/{file}.rs"), "pkg:cargo/a");
        }
        let split = super::compare(
            &flat.graph(),
            &review.graph(),
            &flat.contents,
            &review.contents,
        );
        assert!(
            split.changes.is_empty() && split.removed.is_empty(),
            "{split:?}"
        );
        let merged = super::compare(
            &review.graph(),
            &flat.graph(),
            &review.contents,
            &flat.contents,
        );
        assert!(
            merged.changes.is_empty() && merged.removed.is_empty(),
            "{merged:?}"
        );
        // Split, with a file that changed: its component holds no new code, it is modified,
        // not new (Q33).
        let mut edited = review.clone();
        edited.edit("a/src/two.rs", 1);
        let split = super::compare(
            &flat.graph(),
            &edited.graph(),
            &flat.contents,
            &edited.contents,
        );
        assert_eq!(
            split.changes.get(&id("component:cluster/a/src/one.rs")),
            Some(&Change::Modified)
        );
        assert!(
            !split
                .changes
                .contains_key(&id("component:cluster/a/src/three.rs"))
        );
    }

    #[test]
    fn a_folder_renamed_is_modified_with_its_links() {
        let mut review = Fixture::new();
        review
            .node(
                "component:dir/a/tests",
                NodeKind::Component,
                Some("pkg:cargo/a"),
            )
            .file("a/tests/one.rs", "component:dir/a/tests")
            .file("a/tests/two.rs", "component:dir/a/tests")
            .edge(
                "file:a/tests/one.rs",
                "file:b/src/lib.rs",
                EdgeKind::Calls,
                Provenance::TreeSitter,
            );
        // git mv tests checks: the same files at other paths, the component under another id.
        let mut head = Fixture::new();
        head.node(
            "component:dir/a/checks",
            NodeKind::Component,
            Some("pkg:cargo/a"),
        )
        .file("a/checks/one.rs", "component:dir/a/checks")
        .file("a/checks/two.rs", "component:dir/a/checks")
        .edge(
            "file:a/checks/one.rs",
            "file:b/src/lib.rs",
            EdgeKind::Calls,
            Provenance::TreeSitter,
        );
        for file in ["one", "two"] {
            head.contents.insert(
                format!("file:a/checks/{file}.rs"),
                review.contents[&format!("file:a/tests/{file}.rs")],
            );
        }
        let diff = diff(&review, &head);
        let changes: Vec<(&str, Change)> =
            diff.changes.iter().map(|(i, c)| (i.as_str(), *c)).collect();
        assert_eq!(
            changes,
            [
                ("component:dir/a/checks", Change::Modified),
                ("file:a/checks/one.rs", Change::Modified),
                ("file:a/checks/two.rs", Change::Modified),
                ("pkg:cargo/a", Change::Modified)
            ]
        );
        assert!(diff.removed.is_empty(), "{diff:?}");
        assert!(diff.dependencies_added.is_empty() && diff.dependencies_removed.is_empty());
        // On the map: the component open, its link is the review's, under the new name.
        let visible: HashSet<NodeId> = ["pkg:cargo/a", "pkg:cargo/b", "component:dir/a/checks"]
            .into_iter()
            .map(id)
            .collect();
        assert!(new_links(&review.graph(), &head.graph(), &diff, &visible).is_empty());
    }

    #[test]
    fn a_folder_renamed_before_the_commit_leaves_no_box_gone() {
        let mut review = Fixture::new();
        review
            .node(
                "component:dir/a/tests",
                NodeKind::Component,
                Some("pkg:cargo/a"),
            )
            .file("a/tests/one.rs", "component:dir/a/tests")
            .file("a/tests/two.rs", "component:dir/a/tests");
        // git mv tests checks, before the commit reclusters (ADR 0016): the files under the
        // package, and no component for them yet.
        let mut head = Fixture::new();
        head.file("a/checks/one.rs", "pkg:cargo/a")
            .file("a/checks/two.rs", "pkg:cargo/a");
        for file in ["one", "two"] {
            head.contents.insert(
                format!("file:a/checks/{file}.rs"),
                review.contents[&format!("file:a/tests/{file}.rs")],
            );
        }
        let diff = diff(&review, &head);
        // Its code went nowhere: no box gone (Q33).
        assert!(diff.removed.is_empty(), "{diff:?}");
        let changes: Vec<(&str, Change)> =
            diff.changes.iter().map(|(i, c)| (i.as_str(), *c)).collect();
        assert_eq!(
            changes,
            [
                ("file:a/checks/one.rs", Change::Modified),
                ("file:a/checks/two.rs", Change::Modified),
                ("pkg:cargo/a", Change::Modified)
            ]
        );
        // One of its files deleted: its code went, the box is gone.
        head.remove("file:a/checks/two.rs");
        let gone = super::compare(
            &review.graph(),
            &head.graph(),
            &review.contents,
            &head.contents,
        );
        assert_eq!(ids(&gone.removed), ["component:dir/a/tests"]);
    }

    #[test]
    fn a_folder_renamed_before_the_commit_keeps_its_dependencies() {
        // tests calls core, core calls tests back: a cycle already at the review.
        let mut review = Fixture::new();
        review
            .node(
                "component:dir/a/tests",
                NodeKind::Component,
                Some("pkg:cargo/a"),
            )
            .node(
                "component:dir/a/core",
                NodeKind::Component,
                Some("pkg:cargo/a"),
            )
            .file("a/tests/one.rs", "component:dir/a/tests")
            .file("a/tests/two.rs", "component:dir/a/tests")
            .file("a/core/core.rs", "component:dir/a/core")
            .edge(
                "file:a/tests/one.rs",
                "file:a/core/core.rs",
                EdgeKind::Calls,
                Provenance::TreeSitter,
            )
            .edge(
                "file:a/core/core.rs",
                "file:a/tests/two.rs",
                EdgeKind::Calls,
                Provenance::TreeSitter,
            );
        // git mv tests checks, before the commit: the files under the package, their calls
        // the same.
        let mut head = review.clone();
        head.remove("component:dir/a/tests")
            .remove("file:a/tests/one.rs")
            .remove("file:a/tests/two.rs")
            .unlink("file:a/tests/one.rs", "file:a/core/core.rs")
            .unlink("file:a/core/core.rs", "file:a/tests/two.rs")
            .file("a/checks/one.rs", "pkg:cargo/a")
            .file("a/checks/two.rs", "pkg:cargo/a")
            .edge(
                "file:a/checks/one.rs",
                "file:a/core/core.rs",
                EdgeKind::Calls,
                Provenance::TreeSitter,
            )
            .edge(
                "file:a/core/core.rs",
                "file:a/checks/two.rs",
                EdgeKind::Calls,
                Provenance::TreeSitter,
            );
        for file in ["one", "two"] {
            head.contents.insert(
                format!("file:a/checks/{file}.rs"),
                review.contents[&format!("file:a/tests/{file}.rs")],
            );
        }
        let diff = diff(&review, &head);
        assert_eq!(ids(&diff.regrouped), ["component:dir/a/tests"]);
        // Nothing went: neither the box (Q33) nor its dependencies.
        assert!(diff.removed.is_empty(), "{diff:?}");
        assert!(
            diff.dependencies_removed.is_empty() && diff.dependencies_added.is_empty(),
            "{diff:?}"
        );
        assert!(diff.cycles_added.is_empty());
        // Package a open: its calls are the review's.
        let visible: HashSet<NodeId> = [
            "pkg:cargo/a",
            "pkg:cargo/b",
            "pkg:cargo/c",
            "file:a/src/lib.rs",
            "file:a/checks/one.rs",
            "file:a/checks/two.rs",
            "component:dir/a/core",
        ]
        .into_iter()
        .map(id)
        .collect();
        assert!(new_links(&review.graph(), &head.graph(), &diff, &visible).is_empty());
        // A comment written in core.rs meanwhile: its calls are the same, nothing more (Q36).
        head.edit("a/core/core.rs", 1);
        let commented = super::compare(
            &review.graph(),
            &head.graph(),
            &review.contents,
            &head.contents,
        );
        assert!(
            commented.dependencies_removed.is_empty() && commented.dependencies_added.is_empty(),
            "{commented:?}"
        );
        assert!(new_links(&review.graph(), &head.graph(), &commented, &visible).is_empty());
    }

    #[test]
    fn a_regrouped_box_keeps_what_its_edits_bring() {
        let mut review = Fixture::new();
        for (folder, file) in [("x", "one"), ("y", "two")] {
            let component = format!("component:dir/a/{folder}");
            review
                .node(&component, NodeKind::Component, Some("pkg:cargo/a"))
                .file(&format!("a/{folder}/{file}.rs"), &component);
        }
        review.node(
            "component:dir/a/z",
            NodeKind::Component,
            Some("pkg:cargo/a"),
        );
        review.file("a/z/three.rs", "component:dir/a/z");
        // The commit clusters one.rs and two.rs together, in a new component; one.rs, edited,
        // calls three.rs now.
        let mut head = review.clone();
        head.remove("component:dir/a/x")
            .remove("component:dir/a/y")
            .node(
                "component:cluster/a/x/one.rs",
                NodeKind::Component,
                Some("pkg:cargo/a"),
            )
            .node(
                "file:a/x/one.rs",
                NodeKind::File,
                Some("component:cluster/a/x/one.rs"),
            )
            .node(
                "file:a/y/two.rs",
                NodeKind::File,
                Some("component:cluster/a/x/one.rs"),
            )
            .edit("a/x/one.rs", 1)
            .edge(
                "file:a/x/one.rs",
                "file:a/z/three.rs",
                EdgeKind::Calls,
                Provenance::TreeSitter,
            );
        // Edited, its calls the same: the regrouping brings no dependency (Q36).
        let mut same = head.clone();
        same.unlink("file:a/x/one.rs", "file:a/z/three.rs");
        let same = super::compare(
            &review.graph(),
            &same.graph(),
            &review.contents,
            &same.contents,
        );
        assert!(
            same.dependencies_added.is_empty() && same.dependencies_removed.is_empty(),
            "{same:?}"
        );
        let diff = diff(&review, &head);
        let cluster = id("component:cluster/a/x/one.rs");
        assert!(diff.regrouped.contains(&cluster), "{diff:?}");
        assert_eq!(diff.changes.get(&cluster), Some(&Change::Modified));
        assert_eq!(
            diff.dependencies_added,
            BTreeSet::from([dependency(
                "component:cluster/a/x/one.rs",
                "component:dir/a/z"
            )])
        );
        let visible: HashSet<NodeId> = [
            "pkg:cargo/a",
            "pkg:cargo/b",
            "pkg:cargo/c",
            "file:a/src/lib.rs",
            "component:cluster/a/x/one.rs",
            "component:dir/a/z",
        ]
        .into_iter()
        .map(id)
        .collect();
        assert_eq!(
            new_links(&review.graph(), &head.graph(), &diff, &visible),
            HashSet::from([(cluster, id("component:dir/a/z"))])
        );
    }

    #[test]
    fn a_cycle_through_a_regrouped_box_counts_when_an_edit_closes_it() {
        // x, y and z; one.rs (in x) calls three.rs (in z), which holds four.rs too.
        let mut review = Fixture::new();
        for (folder, file) in [("x", "one"), ("y", "two"), ("z", "three")] {
            let component = format!("component:dir/a/{folder}");
            review
                .node(&component, NodeKind::Component, Some("pkg:cargo/a"))
                .file(&format!("a/{folder}/{file}.rs"), &component);
        }
        review.file("a/z/four.rs", "component:dir/a/z").edge(
            "file:a/x/one.rs",
            "file:a/z/three.rs",
            EdgeKind::Calls,
            Provenance::TreeSitter,
        );
        // one.rs and two.rs moved together into w: no majority, w is a regrouping.
        let mut head = review.clone();
        head.remove("component:dir/a/x")
            .remove("component:dir/a/y")
            .remove("file:a/x/one.rs")
            .remove("file:a/y/two.rs")
            .node(
                "component:dir/a/w",
                NodeKind::Component,
                Some("pkg:cargo/a"),
            );
        for (from, file) in [("x", "one"), ("y", "two")] {
            head.file(&format!("a/w/{file}.rs"), "component:dir/a/w");
            head.contents.insert(
                format!("file:a/w/{file}.rs"),
                review.contents[&format!("file:a/{from}/{file}.rs")],
            );
        }
        head.edge(
            "file:a/w/one.rs",
            "file:a/z/three.rs",
            EdgeKind::Calls,
            Provenance::TreeSitter,
        );
        let w_to_z = head.clone();
        // four.rs, edited, calls two.rs: the edit closes a cycle.
        head.edit("a/z/four.rs", 1).edge(
            "file:a/z/four.rs",
            "file:a/w/two.rs",
            EdgeKind::Calls,
            Provenance::TreeSitter,
        );
        let diff = diff(&review, &head);
        assert!(diff.regrouped.contains(&id("component:dir/a/w")));
        assert_eq!(
            diff.dependencies_added,
            BTreeSet::from([dependency("component:dir/a/z", "component:dir/a/w")])
        );
        let cycles: Vec<Vec<&str>> = diff.cycles_added.iter().map(ids).collect();
        assert_eq!(cycles, [["component:dir/a/w", "component:dir/a/z"]]);
        // The same call there at the review, from two.rs in y: the moves alone close the cycle
        // in w, a regrouping (Q33).
        let mut review = review.clone();
        review.edge(
            "file:a/z/three.rs",
            "file:a/y/two.rs",
            EdgeKind::Calls,
            Provenance::TreeSitter,
        );
        let mut moved = w_to_z;
        moved.edge(
            "file:a/z/three.rs",
            "file:a/w/two.rs",
            EdgeKind::Calls,
            Provenance::TreeSitter,
        );
        // A comment in three.rs too: no call changed (Q36).
        moved.edit("a/z/three.rs", 1);
        let regrouping = super::compare(
            &review.graph(),
            &moved.graph(),
            &review.contents,
            &moved.contents,
        );
        assert!(
            regrouping.cycles_added.is_empty(),
            "{:?}",
            regrouping.cycles_added
        );
        assert!(
            regrouping.dependencies_added.is_empty() && regrouping.dependencies_removed.is_empty(),
            "{regrouping:?}"
        );
    }

    #[test]
    fn a_regrouping_makes_no_jump() {
        // Four files of x call z.
        let mut review = Fixture::new();
        review
            .node(
                "component:dir/a/x",
                NodeKind::Component,
                Some("pkg:cargo/a"),
            )
            .node(
                "component:dir/a/z",
                NodeKind::Component,
                Some("pkg:cargo/a"),
            )
            .file("a/z/target.rs", "component:dir/a/z");
        for n in 0..4 {
            review
                .file(&format!("a/x/f{n}.rs"), "component:dir/a/x")
                .edge(
                    &format!("file:a/x/f{n}.rs"),
                    "file:a/z/target.rs",
                    EdgeKind::Calls,
                    Provenance::TreeSitter,
                );
        }
        // Each moved into a folder of its own: four boxes now call z, by moves alone.
        let mut head = review.clone();
        head.remove("component:dir/a/x");
        for n in 0..4 {
            let component = format!("component:dir/a/w{n}");
            head.remove(&format!("file:a/x/f{n}.rs"))
                .unlink(&format!("file:a/x/f{n}.rs"), "file:a/z/target.rs")
                .node(&component, NodeKind::Component, Some("pkg:cargo/a"))
                .file(&format!("a/w{n}/f{n}.rs"), &component)
                .edge(
                    &format!("file:a/w{n}/f{n}.rs"),
                    "file:a/z/target.rs",
                    EdgeKind::Calls,
                    Provenance::TreeSitter,
                );
            head.contents.insert(
                format!("file:a/w{n}/f{n}.rs"),
                review.contents[&format!("file:a/x/f{n}.rs")],
            );
        }
        let diff = diff(&review, &head);
        assert_eq!(diff.regrouped.len(), 5, "{:?}", diff.regrouped);
        assert!(diff.fan_in.is_empty(), "{:?}", diff.fan_in);
    }

    #[test]
    fn a_regrouping_of_packages_brings_no_dependency() {
        // Packages d and e under t1, f under t2; d calls f.
        let mut review = Fixture::new();
        for top in ["t1", "t2"] {
            review.node(
                &format!("component:cluster/{top}"),
                NodeKind::Component,
                Some("repo:r"),
            );
        }
        for (p, top) in [("d", "t1"), ("e", "t1"), ("f", "t2")] {
            review.package(p).node(
                &format!("pkg:cargo/{p}"),
                NodeKind::Package,
                Some(&format!("component:cluster/{top}")),
            );
        }
        review.call("d", "f").edge(
            "pkg:cargo/d",
            "pkg:cargo/f",
            EdgeKind::DependsOn,
            Provenance::Manifest,
        );
        // The commit clusters again: d joins f, with nothing changed; e, edited, calls f now.
        let mut head = review.clone();
        head.node(
            "pkg:cargo/d",
            NodeKind::Package,
            Some("component:cluster/t2"),
        )
        .call("e", "f")
        .edit("e/src/lib.rs", 1);
        let diff = diff(&review, &head);
        // t1 → t2 still holds through e; d → f is under t2 now: nothing went, nothing came but
        // e's call, under t1 → t2 already there.
        assert!(
            diff.dependencies_added.is_empty() && diff.dependencies_removed.is_empty(),
            "{diff:?}"
        );
        // Without e's call, t1 → t2 is gone: a regrouping, no dependency removed (Q36).
        let mut alone = review.clone();
        alone.node(
            "pkg:cargo/d",
            NodeKind::Package,
            Some("component:cluster/t2"),
        );
        let alone = super::compare(
            &review.graph(),
            &alone.graph(),
            &review.contents,
            &alone.contents,
        );
        assert!(alone.dependencies_removed.is_empty(), "{alone:?}");
        // A dependency written in a manifest still counts, wherever it lands: f → e, where t2
        // never depended on t1.
        let mut written = review.clone();
        written
            .node(
                "pkg:cargo/d",
                NodeKind::Package,
                Some("component:cluster/t2"),
            )
            .edge(
                "pkg:cargo/f",
                "pkg:cargo/e",
                EdgeKind::DependsOn,
                Provenance::Manifest,
            );
        let written = super::compare(
            &review.graph(),
            &written.graph(),
            &review.contents,
            &written.contents,
        );
        assert_eq!(
            written.dependencies_added,
            BTreeSet::from([dependency("component:cluster/t2", "component:cluster/t1")])
        );
    }

    #[test]
    fn a_file_moved_between_two_boxes_moves_its_dependencies() {
        let mut review = Fixture::new();
        for (folder, file) in [("x", "one"), ("x", "two"), ("y", "three"), ("z", "four")] {
            let component = format!("component:dir/a/{folder}");
            review
                .node(&component, NodeKind::Component, Some("pkg:cargo/a"))
                .file(&format!("a/{folder}/{file}.rs"), &component);
        }
        review.edge(
            "file:a/x/two.rs",
            "file:a/z/four.rs",
            EdgeKind::Calls,
            Provenance::TreeSitter,
        );
        // git mv x/two.rs y/two.rs: x and y stay, the call goes with the file.
        let mut head = review.clone();
        head.remove("file:a/x/two.rs")
            .unlink("file:a/x/two.rs", "file:a/z/four.rs")
            .file("a/y/two.rs", "component:dir/a/y")
            .edge(
                "file:a/y/two.rs",
                "file:a/z/four.rs",
                EdgeKind::Calls,
                Provenance::TreeSitter,
            );
        head.contents
            .insert("file:a/y/two.rs".into(), review.contents["file:a/x/two.rs"]);
        let diff = diff(&review, &head);
        assert!(diff.regrouped.is_empty(), "{diff:?}");
        assert_eq!(
            diff.dependencies_removed,
            BTreeSet::from([dependency("component:dir/a/x", "component:dir/a/z")])
        );
        assert_eq!(
            diff.dependencies_added,
            BTreeSet::from([dependency("component:dir/a/y", "component:dir/a/z")])
        );
        // Carried by a call between symbols alone, as the language server gives them: the
        // symbols follow their file (review n° 6 of Phase 5, m-1).
        let symbols = |fixture: &Fixture, file: &str| {
            let mut fixture = fixture.clone();
            let function = format!("rust:{}::f", file.replace('/', "_"));
            fixture
                .unlink(&format!("file:{file}"), "file:a/z/four.rs")
                .edge(
                    &function,
                    "rust:a_z_four.rs::f",
                    EdgeKind::Calls,
                    Provenance::Lsp,
                );
            fixture
        };
        let (by_symbols, moved_symbols) =
            (symbols(&review, "a/x/two.rs"), symbols(&head, "a/y/two.rs"));
        let called = super::compare(
            &by_symbols.graph(),
            &moved_symbols.graph(),
            &by_symbols.contents,
            &moved_symbols.contents,
        );
        assert_eq!(called.dependencies_removed, diff.dependencies_removed);
        assert_eq!(called.dependencies_added, diff.dependencies_added);
        // The language server alone finds a call from the moved file to b: neither the move nor
        // an edit brought it (Q28).
        head.edge(
            "rust:a_y_two.rs::f",
            "rust:b_src_lib.rs::f",
            EdgeKind::Calls,
            Provenance::Lsp,
        );
        let verified = super::compare(
            &review.graph(),
            &head.graph(),
            &review.contents,
            &head.contents,
        );
        assert_eq!(verified.dependencies_added, diff.dependencies_added);
    }

    #[test]
    fn the_crate_that_gives_or_takes_the_files_is_modified() {
        // A root crate, and c inside it.
        let mut review = Fixture::new();
        review
            .node("pkg:cargo/root", NodeKind::Package, Some("repo:r"))
            .file("src/main.rs", "pkg:cargo/root");
        // c/Cargo.toml deleted: c's file, unchanged, joins the root crate (Q37).
        let mut head = review.clone();
        head.remove("pkg:cargo/c")
            .node("file:c/src/lib.rs", NodeKind::File, Some("pkg:cargo/root"))
            .node(
                "rust:c_src_lib.rs::f",
                NodeKind::Symbol(SymbolKind::Function),
                Some("file:c/src/lib.rs"),
            );
        let gone = diff(&review, &head);
        assert_eq!(ids(&gone.removed), ["pkg:cargo/c"]);
        assert_eq!(
            gone.changes.get(&id("pkg:cargo/root")),
            Some(&Change::Modified)
        );
        // A manifest written for it again: the root crate gives the file.
        let made = super::compare(
            &head.graph(),
            &review.graph(),
            &head.contents,
            &review.contents,
        );
        assert_eq!(made.changes.get(&id("pkg:cargo/c")), Some(&Change::Added));
        assert_eq!(
            made.changes.get(&id("pkg:cargo/root")),
            Some(&Change::Modified)
        );
    }

    /// Review n° 5 of Phase 5, I-1: the dependencies of a crate whose manifest went follow its
    /// files into the root crate (Q36, Q37), its symbols found again under the root's names.
    #[test]
    fn the_dependencies_follow_the_files_into_the_root_crate() {
        let mut review = Fixture::new();
        review
            .node("pkg:cargo/root", NodeKind::Package, Some("repo:r"))
            .file("src/main.rs", "pkg:cargo/root")
            .edge(
                "file:c/src/lib.rs",
                "file:b/src/lib.rs",
                EdgeKind::Imports,
                Provenance::TreeSitter,
            )
            .edge(
                "rust:c_src_lib.rs::f",
                "rust:b_src_lib.rs::f",
                EdgeKind::Calls,
                Provenance::Lsp,
            );
        // c/Cargo.toml deleted: c's file, unchanged, in the root crate; its function named
        // after the root now.
        let mut head = review.clone();
        head.remove("pkg:cargo/c")
            .remove("rust:c_src_lib.rs::f")
            .unlink("rust:c_src_lib.rs::f", "rust:b_src_lib.rs::f")
            .node("file:c/src/lib.rs", NodeKind::File, Some("pkg:cargo/root"))
            .node(
                "rust:root_c_src_lib.rs::f",
                NodeKind::Symbol(SymbolKind::Function),
                Some("file:c/src/lib.rs"),
            )
            .edge(
                "rust:root_c_src_lib.rs::f",
                "rust:b_src_lib.rs::f",
                EdgeKind::Calls,
                Provenance::Lsp,
            );
        let diff = diff(&review, &head);
        assert_eq!(
            diff.renamed.get(&id("rust:c_src_lib.rs::f")),
            Some(&id("rust:root_c_src_lib.rs::f"))
        );
        assert_eq!(
            diff.dependencies_removed,
            BTreeSet::from([dependency("pkg:cargo/c", "pkg:cargo/b")])
        );
        assert_eq!(
            diff.dependencies_added,
            BTreeSet::from([dependency("pkg:cargo/root", "pkg:cargo/b")])
        );
        let visible: HashSet<NodeId> = ["pkg:cargo/a", "pkg:cargo/b", "pkg:cargo/root"]
            .into_iter()
            .map(id)
            .collect();
        assert_eq!(
            new_links(&review.graph(), &head.graph(), &diff, &visible),
            HashSet::from([(id("pkg:cargo/root"), id("pkg:cargo/b"))])
        );
        // Its file renamed in its folder meanwhile: the dependency follows it all the same
        // (review n° 8 of Phase 5, m-1).
        let mut renamed = head.clone();
        renamed.move_file("c/src/lib.rs", "c/src/core.rs", "pkg:cargo/root");
        let renamed = super::compare(
            &review.graph(),
            &renamed.graph(),
            &review.contents,
            &renamed.contents,
        );
        assert_eq!(renamed.dependencies_added, diff.dependencies_added);
        assert_eq!(renamed.dependencies_removed, diff.dependencies_removed);
        // Its manifest back: c → b again, root → b gone.
        let back = super::compare(
            &head.graph(),
            &review.graph(),
            &head.contents,
            &review.contents,
        );
        assert_eq!(
            back.dependencies_added,
            BTreeSet::from([dependency("pkg:cargo/c", "pkg:cargo/b")])
        );
        assert_eq!(
            back.dependencies_removed,
            BTreeSet::from([dependency("pkg:cargo/root", "pkg:cargo/b")])
        );
    }

    /// Q38: a move counts when neither the box the file leaves nor the one it joins is a
    /// regrouping.
    #[test]
    fn a_move_into_or_out_of_a_regrouping_brings_no_dependency() {
        let mut review = Fixture::new();
        for (folder, file) in [("x", "one"), ("x", "two"), ("y", "three"), ("z", "four")] {
            let component = format!("component:dir/a/{folder}");
            review
                .node(&component, NodeKind::Component, Some("pkg:cargo/a"))
                .file(&format!("a/{folder}/{file}.rs"), &component);
        }
        review.edge(
            "file:a/x/two.rs",
            "file:a/z/four.rs",
            EdgeKind::Calls,
            Provenance::TreeSitter,
        );
        let moved = |review: &Fixture, from: &str, to: &str, folder: &str| {
            let mut head = review.clone();
            head.remove(&format!("file:a/{from}/two.rs"))
                .unlink(&format!("file:a/{from}/two.rs"), "file:a/z/four.rs")
                .file(
                    &format!("a/{to}/two.rs"),
                    &format!("component:dir/a/{folder}"),
                )
                .edge(
                    &format!("file:a/{to}/two.rs"),
                    "file:a/z/four.rs",
                    EdgeKind::Calls,
                    Provenance::TreeSitter,
                );
            head.contents.insert(
                format!("file:a/{to}/two.rs"),
                review.contents[&format!("file:a/{from}/two.rs")],
            );
            head
        };
        // Into a new folder w, a regrouping: x → z stays, w → z comes with no change of the
        // code.
        let mut into = moved(&review, "x", "w", "w");
        into.node(
            "component:dir/a/w",
            NodeKind::Component,
            Some("pkg:cargo/a"),
        );
        let diff = diff(&review, &into);
        assert!(diff.regrouped.contains(&id("component:dir/a/w")));
        assert!(
            diff.dependencies_added.is_empty() && diff.dependencies_removed.is_empty(),
            "{diff:?}"
        );
        // x's only file into y, x gone, a regrouping: y → z is no dependency added.
        let mut alone = review.clone();
        alone.remove("file:a/x/one.rs");
        let mut out = moved(&alone, "x", "y", "y");
        out.remove("component:dir/a/x");
        let diff = super::compare(&alone.graph(), &out.graph(), &alone.contents, &out.contents);
        assert!(
            diff.regrouped.contains(&id("component:dir/a/x")),
            "{diff:?}"
        );
        assert!(
            diff.dependencies_added.is_empty() && diff.dependencies_removed.is_empty(),
            "{diff:?}"
        );
    }

    /// Review n° 6 of Phase 5, I-1: a crate renamed in its manifest while the packages are
    /// clustered again is the same box (spec « Stable identity »); the clusters' new shares
    /// change no dependency (Q36).
    #[test]
    fn a_crate_renamed_while_the_packages_regroup_changes_no_dependency() {
        // t1 holds d and e, t2 holds f and g; e depends on d, f on e.
        let mut review = Fixture::new();
        for top in ["t1", "t2"] {
            review.node(
                &format!("component:cluster/{top}"),
                NodeKind::Component,
                Some("repo:r"),
            );
        }
        for (p, top) in [("d", "t1"), ("e", "t1"), ("f", "t2"), ("g", "t2")] {
            review.package(p).node(
                &format!("pkg:cargo/{p}"),
                NodeKind::Package,
                Some(&format!("component:cluster/{top}")),
            );
        }
        let depends = |fixture: &mut Fixture, src: &str, dst: &str| {
            fixture.edge(
                &format!("pkg:cargo/{src}"),
                &format!("pkg:cargo/{dst}"),
                EdgeKind::DependsOn,
                Provenance::Manifest,
            );
        };
        depends(&mut review, "e", "d");
        depends(&mut review, "f", "e");
        review.call("e", "d");
        // d renamed dx, and e's manifest follows; the packages are clustered again: e and f in
        // t1, dx and g in t2.
        let mut head = review.clone();
        head.remove("pkg:cargo/d")
            .unlink("pkg:cargo/e", "pkg:cargo/d")
            .node(
                "pkg:cargo/dx",
                NodeKind::Package,
                Some("component:cluster/t2"),
            )
            .node("file:d/src/lib.rs", NodeKind::File, Some("pkg:cargo/dx"))
            .node(
                "rust:d_src_lib.rs::f",
                NodeKind::Symbol(SymbolKind::Function),
                Some("file:d/src/lib.rs"),
            )
            .node(
                "pkg:cargo/f",
                NodeKind::Package,
                Some("component:cluster/t1"),
            );
        depends(&mut head, "e", "dx");
        let diff = diff(&review, &head);
        assert_eq!(
            diff.renamed.get(&id("pkg:cargo/d")),
            Some(&id("pkg:cargo/dx"))
        );
        assert!(
            diff.dependencies_added.is_empty() && diff.dependencies_removed.is_empty(),
            "{diff:?}"
        );
        assert!(diff.cycles_added.is_empty());
        let visible: HashSet<NodeId> = ["component:cluster/t1", "component:cluster/t2"]
            .into_iter()
            .chain(["pkg:cargo/a", "pkg:cargo/b", "pkg:cargo/c"])
            .map(id)
            .collect();
        assert!(new_links(&review.graph(), &head.graph(), &diff, &visible).is_empty());
    }

    /// Review n° 6 of Phase 5, I-1: a file renamed in its folder does not change box; the file it
    /// calls, moved into a regrouping, brings nothing either (Q38).
    #[test]
    fn a_file_renamed_in_its_folder_moves_no_dependency() {
        let mut review = Fixture::new();
        for (folder, file) in [("x", "one"), ("x", "two"), ("z", "four"), ("z", "five")] {
            let component = format!("component:dir/a/{folder}");
            review
                .node(&component, NodeKind::Component, Some("pkg:cargo/a"))
                .file(&format!("a/{folder}/{file}.rs"), &component);
        }
        review.edge(
            "file:a/x/one.rs",
            "file:a/z/four.rs",
            EdgeKind::Calls,
            Provenance::TreeSitter,
        );
        // one.rs renamed uno.rs in x; four.rs moved into a new folder w.
        let mut head = review.clone();
        head.remove("file:a/x/one.rs")
            .remove("file:a/z/four.rs")
            .unlink("file:a/x/one.rs", "file:a/z/four.rs")
            .node(
                "component:dir/a/w",
                NodeKind::Component,
                Some("pkg:cargo/a"),
            )
            .file("a/x/uno.rs", "component:dir/a/x")
            .file("a/w/four.rs", "component:dir/a/w")
            .edge(
                "file:a/x/uno.rs",
                "file:a/w/four.rs",
                EdgeKind::Calls,
                Provenance::TreeSitter,
            );
        for (from, to) in [("a/x/one.rs", "a/x/uno.rs"), ("a/z/four.rs", "a/w/four.rs")] {
            head.contents.insert(
                format!("file:{to}"),
                review.contents[&format!("file:{from}")],
            );
        }
        let diff = diff(&review, &head);
        assert!(diff.regrouped.contains(&id("component:dir/a/w")));
        assert!(
            diff.dependencies_added.is_empty() && diff.dependencies_removed.is_empty(),
            "{diff:?}"
        );
        // The other way round: the called file moved between two boxes that stay (x to z), its
        // caller moved into the regrouping. Each end by its own move (Q38): the called one's
        // counts.
        let mut both = Fixture::new();
        for (folder, file) in [
            ("x", "one"),
            ("x", "two"),
            ("y", "three"),
            ("z", "four"),
            ("z", "five"),
        ] {
            let component = format!("component:dir/a/{folder}");
            both.node(&component, NodeKind::Component, Some("pkg:cargo/a"))
                .file(&format!("a/{folder}/{file}.rs"), &component);
        }
        both.edge(
            "file:a/x/one.rs",
            "file:a/z/four.rs",
            EdgeKind::Calls,
            Provenance::TreeSitter,
        );
        let mut moved = both.clone();
        moved
            .remove("file:a/x/one.rs")
            .remove("file:a/z/four.rs")
            .unlink("file:a/x/one.rs", "file:a/z/four.rs")
            .node(
                "component:dir/a/w",
                NodeKind::Component,
                Some("pkg:cargo/a"),
            )
            .file("a/w/one.rs", "component:dir/a/w")
            .file("a/y/four.rs", "component:dir/a/y")
            .edge(
                "file:a/w/one.rs",
                "file:a/y/four.rs",
                EdgeKind::Calls,
                Provenance::TreeSitter,
            );
        for (from, to) in [("a/x/one.rs", "a/w/one.rs"), ("a/z/four.rs", "a/y/four.rs")] {
            moved
                .contents
                .insert(format!("file:{to}"), both.contents[&format!("file:{from}")]);
        }
        let each = super::compare(
            &both.graph(),
            &moved.graph(),
            &both.contents,
            &moved.contents,
        );
        assert_eq!(
            each.dependencies_removed,
            BTreeSet::from([dependency("component:dir/a/x", "component:dir/a/z")])
        );
        assert_eq!(
            each.dependencies_added,
            BTreeSet::from([dependency("component:dir/a/w", "component:dir/a/y")])
        );
    }

    /// A folder moved from one crate to another is the same box under a new id; its
    /// dependencies follow it to the other crate (Q36, Q38).
    #[test]
    fn a_folder_moved_to_another_crate_takes_its_dependencies() {
        let mut review = Fixture::new();
        review
            .node(
                "component:dir/a/tools",
                NodeKind::Component,
                Some("pkg:cargo/a"),
            )
            .file("a/tools/one.rs", "component:dir/a/tools")
            .file("a/tools/two.rs", "component:dir/a/tools")
            .edge(
                "file:a/tools/one.rs",
                "file:c/src/lib.rs",
                EdgeKind::Calls,
                Provenance::TreeSitter,
            );
        // git mv a/tools b/tools.
        let mut head = review.clone();
        head.remove("component:dir/a/tools")
            .remove("file:a/tools/one.rs")
            .remove("file:a/tools/two.rs")
            .unlink("file:a/tools/one.rs", "file:c/src/lib.rs")
            .node(
                "component:dir/b/tools",
                NodeKind::Component,
                Some("pkg:cargo/b"),
            )
            .file("b/tools/one.rs", "component:dir/b/tools")
            .file("b/tools/two.rs", "component:dir/b/tools")
            .edge(
                "file:b/tools/one.rs",
                "file:c/src/lib.rs",
                EdgeKind::Calls,
                Provenance::TreeSitter,
            );
        for file in ["one", "two"] {
            head.contents.insert(
                format!("file:b/tools/{file}.rs"),
                review.contents[&format!("file:a/tools/{file}.rs")],
            );
        }
        let diff = diff(&review, &head);
        assert_eq!(
            diff.renamed.get(&id("component:dir/a/tools")),
            Some(&id("component:dir/b/tools"))
        );
        assert_eq!(
            diff.dependencies_removed,
            BTreeSet::from([dependency("pkg:cargo/a", "pkg:cargo/c")])
        );
        assert_eq!(
            diff.dependencies_added,
            BTreeSet::from([dependency("pkg:cargo/b", "pkg:cargo/c")])
        );
        let visible: HashSet<NodeId> = ["pkg:cargo/a", "pkg:cargo/b", "pkg:cargo/c"]
            .into_iter()
            .map(id)
            .collect();
        assert_eq!(
            new_links(&review.graph(), &head.graph(), &diff, &visible),
            HashSet::from([(id("pkg:cargo/b"), id("pkg:cargo/c"))])
        );
    }

    /// A file moved inside its crate while the crate is clustered elsewhere: the move changes
    /// the dependencies inside the crate, the regrouping none above it (Q36, Q38).
    #[test]
    fn a_move_inside_a_crate_that_regroups_stays_inside() {
        // t1 holds a, t2 holds b and c; a's file in x calls c.
        let mut review = Fixture::new();
        for top in ["t1", "t2"] {
            review.node(
                &format!("component:cluster/{top}"),
                NodeKind::Component,
                Some("repo:r"),
            );
        }
        for (p, top) in [("a", "t1"), ("b", "t2"), ("c", "t2")] {
            review.node(
                &format!("pkg:cargo/{p}"),
                NodeKind::Package,
                Some(&format!("component:cluster/{top}")),
            );
        }
        for folder in ["x", "y"] {
            review
                .node(
                    &format!("component:dir/a/{folder}"),
                    NodeKind::Component,
                    Some("pkg:cargo/a"),
                )
                .file(
                    &format!("a/{folder}/keep.rs"),
                    &format!("component:dir/a/{folder}"),
                );
        }
        review.file("a/x/one.rs", "component:dir/a/x").edge(
            "file:a/x/one.rs",
            "file:a/x/keep.rs",
            EdgeKind::Calls,
            Provenance::TreeSitter,
        );
        review.edge(
            "file:a/x/one.rs",
            "file:c/src/lib.rs",
            EdgeKind::Calls,
            Provenance::TreeSitter,
        );
        // one.rs moved to y; the commit clusters a with b and c, under t2.
        let mut head = review.clone();
        head.remove("file:a/x/one.rs")
            .unlink("file:a/x/one.rs", "file:a/x/keep.rs")
            .unlink("file:a/x/one.rs", "file:c/src/lib.rs")
            .node(
                "pkg:cargo/a",
                NodeKind::Package,
                Some("component:cluster/t2"),
            )
            .file("a/y/one.rs", "component:dir/a/y")
            .edge(
                "file:a/y/one.rs",
                "file:a/x/keep.rs",
                EdgeKind::Calls,
                Provenance::TreeSitter,
            )
            .edge(
                "file:a/y/one.rs",
                "file:c/src/lib.rs",
                EdgeKind::Calls,
                Provenance::TreeSitter,
            );
        head.contents
            .insert("file:a/y/one.rs".into(), review.contents["file:a/x/one.rs"]);
        let diff = diff(&review, &head);
        // Inside a: y calls x now, a real move.
        assert_eq!(
            diff.dependencies_added,
            BTreeSet::from([dependency("component:dir/a/y", "component:dir/a/x")])
        );
        // Above a: t1 → t2 went with the regrouping, a → c came with it; neither counts.
        assert!(diff.dependencies_removed.is_empty(), "{diff:?}");
        // a and c together in t2 at the review, a alone in t1 now: a → c went, t1 → t2 came,
        // with the regrouping alone.
        let mut together = review.clone();
        together.node(
            "pkg:cargo/a",
            NodeKind::Package,
            Some("component:cluster/t2"),
        );
        let mut apart = head.clone();
        apart.node(
            "pkg:cargo/a",
            NodeKind::Package,
            Some("component:cluster/t1"),
        );
        let diff = super::compare(
            &together.graph(),
            &apart.graph(),
            &together.contents,
            &apart.contents,
        );
        assert_eq!(
            diff.dependencies_added,
            BTreeSet::from([dependency("component:dir/a/y", "component:dir/a/x")])
        );
        assert!(diff.dependencies_removed.is_empty(), "{diff:?}");
    }

    /// An unchanged file clustered into another component of its crate does not move: an edge
    /// from a file renamed beside it brings no dependency (Q30, Q38).
    #[test]
    fn a_file_clustered_elsewhere_does_not_move() {
        let mut review = Fixture::new();
        for cluster in ["k1", "k2", "k3"] {
            review.node(
                &format!("component:cluster/a/{cluster}"),
                NodeKind::Component,
                Some("pkg:cargo/a"),
            );
        }
        review
            .file("a/v.rs", "component:cluster/a/k1")
            .file("a/w1.rs", "component:cluster/a/k1")
            .file("a/w2.rs", "component:cluster/a/k2")
            .file("a/x/one.rs", "component:cluster/a/k3")
            .edge(
                "file:a/x/one.rs",
                "file:a/v.rs",
                EdgeKind::Calls,
                Provenance::TreeSitter,
            );
        // one.rs renamed uno.rs beside itself; v.rs, unchanged, clustered into k2.
        let mut head = review.clone();
        head.remove("file:a/x/one.rs")
            .unlink("file:a/x/one.rs", "file:a/v.rs")
            .file("a/x/uno.rs", "component:cluster/a/k3")
            .node(
                "file:a/v.rs",
                NodeKind::File,
                Some("component:cluster/a/k2"),
            )
            .edge(
                "file:a/x/uno.rs",
                "file:a/v.rs",
                EdgeKind::Calls,
                Provenance::TreeSitter,
            );
        head.contents
            .insert("file:a/x/uno.rs".into(), review.contents["file:a/x/one.rs"]);
        let diff = diff(&review, &head);
        assert!(
            diff.dependencies_added.is_empty() && diff.dependencies_removed.is_empty(),
            "{diff:?}"
        );
    }

    /// A flat folder, a/src, split into three components: k1 holds p1 to p3, k2 q1 and q2, k3
    /// one.rs and r1.rs; one.rs and r1.rs call each other, q1.rs calls one.rs.
    fn flat_folder() -> Fixture {
        let mut review = Fixture::new();
        for cluster in ["k1", "k2", "k3"] {
            review.node(
                &format!("component:cluster/a/src/{cluster}"),
                NodeKind::Component,
                Some("pkg:cargo/a"),
            );
        }
        for (file, cluster) in [
            ("p1", "k1"),
            ("p2", "k1"),
            ("p3", "k1"),
            ("q1", "k2"),
            ("q2", "k2"),
            ("one", "k3"),
            ("r1", "k3"),
        ] {
            review.file(
                &format!("a/src/{file}.rs"),
                &format!("component:cluster/a/src/{cluster}"),
            );
        }
        for (src, dst) in [("one", "r1"), ("r1", "one"), ("q1", "one")] {
            review.edge(
                &format!("file:a/src/{src}.rs"),
                &format!("file:a/src/{dst}.rs"),
                EdgeKind::Calls,
                Provenance::TreeSitter,
            );
        }
        review
    }

    fn flat_folder_visible() -> HashSet<NodeId> {
        [
            "component:cluster/a/src/k1",
            "component:cluster/a/src/k2",
            "component:cluster/a/src/k3",
            "file:a/src/lib.rs",
        ]
        .into_iter()
        .map(id)
        .collect()
    }

    /// Q40: a file renamed in a flat folder joins the component most of its neighbours have,
    /// which is the regrouping's doing, not a move.
    #[test]
    fn a_file_renamed_in_its_flat_folder_does_not_move() {
        let review = flat_folder();
        // one.rs renamed uno.rs, before the commit: it joins k1, where most of its folder is.
        let mut head = review.clone();
        head.move_file("a/src/one.rs", "a/src/uno.rs", "component:cluster/a/src/k1")
            .edit("a/src/lib.rs", 1);
        let diff = diff(&review, &head);
        assert!(
            diff.dependencies_added.is_empty() && diff.dependencies_removed.is_empty(),
            "{diff:?}"
        );
        assert!(diff.cycles_added.is_empty(), "{:?}", diff.cycles_added);
        assert!(
            new_links(
                &review.graph(),
                &head.graph(),
                &diff,
                &flat_folder_visible()
            )
            .is_empty()
        );
        assert_eq!(
            diff.changes.get(&id("file:a/src/uno.rs")),
            Some(&Change::Modified)
        );
        // The language server alone finds a call from uno.rs: a rename is no edit (Q28).
        head.edge(
            "rust:a_src_uno.rs::f",
            "rust:c_src_lib.rs::f",
            EdgeKind::Calls,
            Provenance::Lsp,
        );
        let verified = super::compare(
            &review.graph(),
            &head.graph(),
            &review.contents,
            &head.contents,
        );
        assert!(verified.dependencies_added.is_empty(), "{verified:?}");
    }

    /// Review n° 8 of Phase 5, m-3: a file moved up from a subfolder, staying in its component,
    /// calls one renamed in place that changes component: neither end moves between boxes.
    #[test]
    fn a_move_that_keeps_its_component_brings_no_dependency() {
        let mut review = flat_folder();
        review
            .file("a/src/inner/solo.rs", "component:cluster/a/src/k1")
            .edge(
                "file:a/src/inner/solo.rs",
                "file:a/src/one.rs",
                EdgeKind::Calls,
                Provenance::TreeSitter,
            );
        let mut head = review.clone();
        head.move_file(
            "a/src/inner/solo.rs",
            "a/src/solo.rs",
            "component:cluster/a/src/k1",
        )
        .move_file("a/src/one.rs", "a/src/uno.rs", "component:cluster/a/src/k2")
        .edit("a/src/lib.rs", 1);
        let diff = diff(&review, &head);
        assert!(
            diff.dependencies_added.is_empty() && diff.dependencies_removed.is_empty(),
            "{diff:?}"
        );
        assert!(
            new_links(
                &review.graph(),
                &head.graph(),
                &diff,
                &flat_folder_visible()
            )
            .is_empty()
        );
    }

    /// Review n° 7 of Phase 5, m-1: a crate renamed, a file moved inside it, the packages
    /// clustered again: the move counts inside the crate, nothing above it.
    #[test]
    fn a_move_inside_a_renamed_crate_that_regroups_stays_inside() {
        let mut review = Fixture::new();
        for top in ["t1", "t2"] {
            review.node(
                &format!("component:cluster/{top}"),
                NodeKind::Component,
                Some("repo:r"),
            );
        }
        for (p, top) in [("d", "t1"), ("e", "t1"), ("g", "t2")] {
            review.package(p).node(
                &format!("pkg:cargo/{p}"),
                NodeKind::Package,
                Some(&format!("component:cluster/{top}")),
            );
        }
        for folder in ["x", "y"] {
            review
                .node(
                    &format!("component:dir/d/{folder}"),
                    NodeKind::Component,
                    Some("pkg:cargo/d"),
                )
                .file(
                    &format!("d/{folder}/keep.rs"),
                    &format!("component:dir/d/{folder}"),
                );
        }
        review
            .file("d/x/one.rs", "component:dir/d/x")
            .edge(
                "file:d/x/one.rs",
                "file:e/src/lib.rs",
                EdgeKind::Calls,
                Provenance::TreeSitter,
            )
            .edge(
                "file:d/x/one.rs",
                "file:d/x/keep.rs",
                EdgeKind::Calls,
                Provenance::TreeSitter,
            );
        // d renamed dx, clustered into t2; one.rs moved from x to y.
        let mut head = review.clone();
        head.remove("pkg:cargo/d")
            .node(
                "pkg:cargo/dx",
                NodeKind::Package,
                Some("component:cluster/t2"),
            )
            .node("file:d/src/lib.rs", NodeKind::File, Some("pkg:cargo/dx"))
            .node(
                "rust:d_src_lib.rs::f",
                NodeKind::Symbol(SymbolKind::Function),
                Some("file:d/src/lib.rs"),
            );
        for folder in ["x", "y"] {
            head.node(
                &format!("component:dir/d/{folder}"),
                NodeKind::Component,
                Some("pkg:cargo/dx"),
            );
        }
        head.move_file("d/x/one.rs", "d/y/one.rs", "component:dir/d/y");
        let diff = diff(&review, &head);
        assert_eq!(
            diff.dependencies_added,
            BTreeSet::from([dependency("component:dir/d/y", "component:dir/d/x")])
        );
        assert!(diff.dependencies_removed.is_empty(), "{diff:?}");
        let visible: HashSet<NodeId> = ["component:cluster/t1", "component:cluster/t2"]
            .into_iter()
            .chain(["pkg:cargo/a", "pkg:cargo/b", "pkg:cargo/c"])
            .map(id)
            .collect();
        assert!(new_links(&review.graph(), &head.graph(), &diff, &visible).is_empty());
    }

    /// Review n° 7 of Phase 5, m-1: a line of the map follows the called file too.
    #[test]
    fn a_line_follows_the_called_file() {
        let mut review = Fixture::new();
        for (folder, file) in [("x", "one"), ("y", "three"), ("z", "four"), ("z", "five")] {
            let component = format!("component:dir/a/{folder}");
            review
                .node(&component, NodeKind::Component, Some("pkg:cargo/a"))
                .file(&format!("a/{folder}/{file}.rs"), &component);
        }
        review.edge(
            "file:a/x/one.rs",
            "file:a/z/four.rs",
            EdgeKind::Calls,
            Provenance::TreeSitter,
        );
        let mut head = review.clone();
        head.move_file("a/z/four.rs", "a/y/four.rs", "component:dir/a/y");
        let diff = diff(&review, &head);
        let visible: HashSet<NodeId> = [
            "component:dir/a/x",
            "component:dir/a/y",
            "component:dir/a/z",
            "file:a/src/lib.rs",
        ]
        .into_iter()
        .map(id)
        .collect();
        assert_eq!(
            new_links(&review.graph(), &head.graph(), &diff, &visible),
            HashSet::from([(id("component:dir/a/x"), id("component:dir/a/y"))])
        );
    }

    #[test]
    fn a_crate_comes_and_goes_with_its_manifest() {
        let review = Fixture::new();
        // c/Cargo.toml deleted, and nothing depends on c: its file, unchanged, under the repo
        // (Q35).
        let mut head = review.clone();
        head.remove("pkg:cargo/c")
            .node("file:c/src/lib.rs", NodeKind::File, Some("repo:r"))
            .node(
                "rust:c_src_lib.rs::f",
                NodeKind::Symbol(SymbolKind::Function),
                Some("file:c/src/lib.rs"),
            );
        let gone = diff(&review, &head);
        assert!(gone.changed_files.is_empty());
        assert_eq!(ids(&gone.removed), ["pkg:cargo/c"]);
        // Its manifest written back around the same file: a new crate.
        let back = super::compare(
            &head.graph(),
            &review.graph(),
            &head.contents,
            &review.contents,
        );
        assert_eq!(back.changes.get(&id("pkg:cargo/c")), Some(&Change::Added));
        assert!(back.removed.is_empty(), "{back:?}");
        // Under a component that groups packages: the component is modified.
        let mut grouped = Fixture::new();
        grouped.node("component:cluster/top", NodeKind::Component, Some("repo:r"));
        for p in ["a", "b"] {
            grouped.node(
                &format!("pkg:cargo/{p}"),
                NodeKind::Package,
                Some("component:cluster/top"),
            );
        }
        let mut without = grouped.clone();
        without
            .remove("pkg:cargo/b")
            .node("file:b/src/lib.rs", NodeKind::File, Some("repo:r"))
            .node(
                "rust:b_src_lib.rs::f",
                NodeKind::Symbol(SymbolKind::Function),
                Some("file:b/src/lib.rs"),
            );
        let diff = super::compare(
            &grouped.graph(),
            &without.graph(),
            &grouped.contents,
            &without.contents,
        );
        assert_eq!(
            diff.changes.get(&id("component:cluster/top")),
            Some(&Change::Modified)
        );
        // And a crate made in it: the component is modified too.
        let made = super::compare(
            &without.graph(),
            &grouped.graph(),
            &without.contents,
            &grouped.contents,
        );
        assert_eq!(made.changes.get(&id("pkg:cargo/b")), Some(&Change::Added));
        assert_eq!(
            made.changes.get(&id("component:cluster/top")),
            Some(&Change::Modified)
        );
        // A crate that leaves it to join the external dependencies, its manifest gone: the
        // component it left is modified.
        let mut external = without.clone();
        external
            .node("component:external", NodeKind::Component, Some("repo:r"))
            .node("pkg:cargo/b", NodeKind::Package, Some("component:external"));
        let left = super::compare(
            &grouped.graph(),
            &external.graph(),
            &grouped.contents,
            &external.contents,
        );
        assert_eq!(
            left.changes.get(&id("pkg:cargo/b")),
            Some(&Change::Modified)
        );
        assert_eq!(
            left.changes.get(&id("component:cluster/top")),
            Some(&Change::Modified)
        );
    }

    #[test]
    fn a_new_box_of_moved_files_is_modified_not_new() {
        let mut review = Fixture::new();
        for (folder, file) in [("x", "one"), ("y", "two"), ("z", "three")] {
            let component = format!("component:dir/a/{folder}");
            review
                .node(&component, NodeKind::Component, Some("pkg:cargo/a"))
                .file(&format!("a/{folder}/{file}.rs"), &component);
        }
        review.edge(
            "file:a/x/one.rs",
            "file:a/z/three.rs",
            EdgeKind::Calls,
            Provenance::TreeSitter,
        );
        // Two files moved from two folders into a new one: no majority, no rename.
        let mut head = review.clone();
        head.remove("component:dir/a/x")
            .remove("component:dir/a/y")
            .remove("file:a/x/one.rs")
            .remove("file:a/y/two.rs")
            .node(
                "component:dir/a/w",
                NodeKind::Component,
                Some("pkg:cargo/a"),
            );
        for (from, file) in [("x", "one"), ("y", "two")] {
            head.file(&format!("a/w/{file}.rs"), "component:dir/a/w");
            head.contents.insert(
                format!("file:a/w/{file}.rs"),
                review.contents[&format!("file:a/{from}/{file}.rs")],
            );
        }
        head.unlink("file:a/x/one.rs", "file:a/z/three.rs").edge(
            "file:a/w/one.rs",
            "file:a/z/three.rs",
            EdgeKind::Calls,
            Provenance::TreeSitter,
        );
        let diff = diff(&review, &head);
        assert_eq!(
            diff.changes.get(&id("component:dir/a/w")),
            Some(&Change::Modified)
        );
        assert!(diff.removed.is_empty(), "{diff:?}");
        // x → z went with the regrouping, w → z came with it: no dependency changed (Q33).
        assert!(
            diff.dependencies_added.is_empty() && diff.dependencies_removed.is_empty(),
            "{diff:?}"
        );
        let visible: HashSet<NodeId> = [
            "pkg:cargo/a",
            "pkg:cargo/b",
            "pkg:cargo/c",
            "file:a/src/lib.rs",
            "component:dir/a/w",
            "component:dir/a/z",
        ]
        .into_iter()
        .map(id)
        .collect();
        assert!(new_links(&review.graph(), &head.graph(), &diff, &visible).is_empty());
        // With a new file in it, code came: the box is new.
        head.file("a/w/four.rs", "component:dir/a/w");
        let came = super::compare(
            &review.graph(),
            &head.graph(),
            &review.contents,
            &head.contents,
        );
        assert_eq!(
            came.changes.get(&id("component:dir/a/w")),
            Some(&Change::Added)
        );
    }

    #[test]
    fn a_crate_that_loses_its_manifest_is_modified() {
        let mut review = Fixture::new();
        review.call("a", "c");
        // Its manifest deleted: c is an external dependency of a now, its file unchanged
        // under the repo (Q34).
        let mut head = review.clone();
        head.remove("pkg:cargo/c")
            .node("component:external", NodeKind::Component, Some("repo:r"))
            .node("pkg:cargo/c", NodeKind::Package, Some("component:external"))
            .node("file:c/src/lib.rs", NodeKind::File, Some("repo:r"))
            .node(
                "rust:c_src_lib.rs::f",
                NodeKind::Symbol(SymbolKind::Function),
                Some("file:c/src/lib.rs"),
            );
        let diff = diff(&review, &head);
        assert!(diff.changed_files.is_empty());
        assert_eq!(
            diff.changes.get(&id("pkg:cargo/c")),
            Some(&Change::Modified)
        );
        // And back: an external package that gains its files.
        let back = super::compare(
            &head.graph(),
            &review.graph(),
            &head.contents,
            &review.contents,
        );
        assert_eq!(
            back.changes.get(&id("pkg:cargo/c")),
            Some(&Change::Modified)
        );
    }

    /// Review n° 3 of Phase 5, m-a: the external dependencies were there at the review; a crate
    /// that joins them, its manifest gone, is a change they hold.
    #[test]
    fn a_crate_changed_by_its_manifest_marks_the_boxes_above() {
        let mut review = Fixture::new();
        review
            .node("component:external", NodeKind::Component, Some("repo:r"))
            .node(
                "pkg:cargo/serde",
                NodeKind::Package,
                Some("component:external"),
            )
            .call("a", "c");
        let mut head = review.clone();
        head.remove("pkg:cargo/c")
            .node("pkg:cargo/c", NodeKind::Package, Some("component:external"))
            .node("file:c/src/lib.rs", NodeKind::File, Some("repo:r"))
            .node(
                "rust:c_src_lib.rs::f",
                NodeKind::Symbol(SymbolKind::Function),
                Some("file:c/src/lib.rs"),
            );
        let diff = diff(&review, &head);
        assert_eq!(
            diff.changes.get(&id("pkg:cargo/c")),
            Some(&Change::Modified)
        );
        assert_eq!(
            diff.changes.get(&id("component:external")),
            Some(&Change::Modified)
        );
        assert!(!diff.changes.contains_key(&id("pkg:cargo/serde")));
        // Its manifest moved it: a → c went, as with a file moved (Q34, Q36).
        assert!(
            diff.dependencies_removed
                .contains(&dependency("pkg:cargo/a", "pkg:cargo/c")),
            "{diff:?}"
        );
        // Carried by a's manifest alone: the package moves itself.
        let manifest = |fixture: &Fixture| {
            let mut fixture = fixture.clone();
            fixture
                .unlink("file:a/src/lib.rs", "file:c/src/lib.rs")
                .edge(
                    "pkg:cargo/a",
                    "pkg:cargo/c",
                    EdgeKind::DependsOn,
                    Provenance::Manifest,
                );
            fixture
        };
        let (review, head) = (manifest(&review), manifest(&head));
        let declared = super::compare(
            &review.graph(),
            &head.graph(),
            &review.contents,
            &head.contents,
        );
        assert_eq!(
            declared.dependencies_removed,
            BTreeSet::from([dependency("pkg:cargo/a", "pkg:cargo/c")])
        );
        assert_eq!(
            declared.dependencies_added,
            BTreeSet::from([dependency("pkg:cargo/a", "component:external")])
        );
    }

    #[test]
    fn the_symbols_of_a_moved_file_keep_their_links() {
        let mut review = Fixture::new();
        review
            .node(
                "component:dir/a/tests",
                NodeKind::Component,
                Some("pkg:cargo/a"),
            )
            .file("a/tests/one.rs", "component:dir/a/tests")
            .edge(
                "rust:a_tests_one.rs::f",
                "rust:b_src_lib.rs::f",
                EdgeKind::Calls,
                Provenance::Lsp,
            );
        let mut head = Fixture::new();
        head.node(
            "component:dir/a/checks",
            NodeKind::Component,
            Some("pkg:cargo/a"),
        )
        .file("a/checks/one.rs", "component:dir/a/checks")
        .edge(
            "rust:a_checks_one.rs::f",
            "rust:b_src_lib.rs::f",
            EdgeKind::Calls,
            Provenance::Lsp,
        );
        head.contents.insert(
            "file:a/checks/one.rs".into(),
            review.contents["file:a/tests/one.rs"],
        );
        let diff = diff(&review, &head);
        assert_eq!(
            diff.renamed.get(&id("rust:a_tests_one.rs::f")),
            Some(&id("rust:a_checks_one.rs::f"))
        );
        // Opened down to the symbols: the call is the review's, under its new name.
        let visible: HashSet<NodeId> = [
            "pkg:cargo/a",
            "pkg:cargo/b",
            "component:dir/a/checks",
            "file:a/checks/one.rs",
            "rust:a_checks_one.rs::f",
            "file:b/src/lib.rs",
            "rust:b_src_lib.rs::f",
        ]
        .into_iter()
        .map(id)
        .collect();
        assert!(new_links(&review.graph(), &head.graph(), &diff, &visible).is_empty());
    }

    #[test]
    fn two_symbols_of_one_name_are_told_apart_by_their_place() {
        use crate::{Position, Span};
        let span = |line: u32| Span::new(Position::new(line, 0), Position::new(line + 1, 0)).ok();
        // Two overloads of one name in a moved file.
        let symbols = |fixture: &mut Fixture, file: &str, prefix: &str| {
            for (n, line) in [(1, 2), (2, 7)] {
                let mut node = Node::new(
                    id(&format!("typescript:{prefix}#f{n}")),
                    NodeKind::Symbol(SymbolKind::Function),
                    "f",
                )
                .with_parent(id(file));
                node.span = span(line);
                fixture.nodes.push(node);
            }
        };
        let mut review = Fixture::new();
        review.file("a/src/one.ts", "pkg:cargo/a");
        symbols(&mut review, "file:a/src/one.ts", "a/src/one");
        let mut head = Fixture::new();
        head.file("a/lib/one.ts", "pkg:cargo/a");
        symbols(&mut head, "file:a/lib/one.ts", "a/lib/one");
        head.contents.insert(
            "file:a/lib/one.ts".into(),
            review.contents["file:a/src/one.ts"],
        );
        let diff = diff(&review, &head);
        for n in [1, 2] {
            assert_eq!(
                diff.renamed.get(&id(&format!("typescript:a/src/one#f{n}"))),
                Some(&id(&format!("typescript:a/lib/one#f{n}")))
            );
        }
    }

    #[test]
    fn a_box_is_renamed_only_into_one_of_its_kind() {
        let mut review = Fixture::new();
        review
            .node(
                "component:dir/a/tools",
                NodeKind::Component,
                Some("pkg:cargo/a"),
            )
            .file("a/tools/one.rs", "component:dir/a/tools")
            .file("a/tools/two.rs", "component:dir/a/tools");
        // The folder became a crate of its own: a component is no package.
        let mut head = review.clone();
        head.remove("component:dir/a/tools")
            .node("pkg:cargo/tools", NodeKind::Package, Some("repo:r"))
            .node(
                "file:a/tools/one.rs",
                NodeKind::File,
                Some("pkg:cargo/tools"),
            )
            .node(
                "file:a/tools/two.rs",
                NodeKind::File,
                Some("pkg:cargo/tools"),
            )
            .edit("a/tools/one.rs", 1);
        let diff = diff(&review, &head);
        assert!(diff.renamed.is_empty(), "{:?}", diff.renamed);
    }

    #[test]
    fn a_new_box_of_new_files_is_new_though_it_takes_a_gone_one_s() {
        let mut review = Fixture::new();
        review
            .node(
                "component:cluster/a/src/x.rs",
                NodeKind::Component,
                Some("pkg:cargo/a"),
            )
            .file("a/src/x.rs", "component:cluster/a/src/x.rs")
            .file("a/src/y.rs", "component:cluster/a/src/x.rs");
        // x and y, unchanged, end in a larger new component made of new files.
        let mut head = Fixture::new();
        head.node(
            "component:cluster/a/src/v.rs",
            NodeKind::Component,
            Some("pkg:cargo/a"),
        );
        for file in ["v", "w", "x", "y", "z"] {
            head.file(&format!("a/src/{file}.rs"), "component:cluster/a/src/v.rs");
        }
        let diff = diff(&review, &head);
        assert!(diff.renamed.is_empty(), "{:?}", diff.renamed);
        assert_eq!(
            diff.changes.get(&id("component:cluster/a/src/v.rs")),
            Some(&Change::Added)
        );
        assert!(
            diff.removed.is_empty(),
            "its files are still there, unchanged"
        );
    }

    #[test]
    fn a_package_renamed_is_modified_and_keeps_its_dependencies() {
        let mut review = Fixture::new();
        review.call("a", "b");
        let mut head = review.clone();
        // The manifest renames b: the package's id changes, its file does not.
        head.remove("pkg:cargo/b");
        head.node("pkg:cargo/bee", NodeKind::Package, Some("repo:r"))
            .node("file:b/src/lib.rs", NodeKind::File, Some("pkg:cargo/bee"))
            .node(
                "rust:b_src_lib.rs::f",
                NodeKind::Symbol(SymbolKind::Function),
                Some("file:b/src/lib.rs"),
            );
        let diff = diff(&review, &head);
        assert_eq!(
            diff.changes.get(&id("pkg:cargo/bee")),
            Some(&Change::Modified)
        );
        assert!(diff.removed.is_empty(), "{diff:?}");
        assert!(diff.dependencies_added.is_empty() && diff.dependencies_removed.is_empty());
    }

    #[test]
    fn an_external_package_comes_and_goes_with_the_manifests() {
        let review = Fixture::new();
        let mut head = review.clone();
        head.node("component:external", NodeKind::Component, Some("repo:r"))
            .node(
                "pkg:cargo/serde",
                NodeKind::Package,
                Some("component:external"),
            )
            .edge(
                "pkg:cargo/a",
                "pkg:cargo/serde",
                EdgeKind::DependsOn,
                Provenance::Manifest,
            );
        let diff = diff(&review, &head);
        assert_eq!(
            diff.changes.get(&id("pkg:cargo/serde")),
            Some(&Change::Added)
        );
        assert_eq!(
            diff.changes.get(&id("component:external")),
            Some(&Change::Added)
        );
        let back = super::compare(
            &head.graph(),
            &review.graph(),
            &head.contents,
            &review.contents,
        );
        assert_eq!(
            ids(&back.removed),
            ["component:external", "pkg:cargo/serde"]
        );
    }

    #[test]
    fn a_new_box_has_no_jump() {
        let mut review = Fixture::new();
        for p in ["d", "e"] {
            review.package(p);
        }
        let mut head = review.clone();
        head.package("n");
        for p in ["a", "b", "c", "d"] {
            head.call(p, "n").edit(&format!("{p}/src/lib.rs"), 1);
        }
        let diff = diff(&review, &head);
        assert_eq!(diff.changes.get(&id("pkg:cargo/n")), Some(&Change::Added));
        assert!(diff.fan_in.is_empty(), "{:?}", diff.fan_in);
    }

    #[test]
    fn new_links_are_those_the_map_shows_between_visible_nodes() {
        let mut review = Fixture::new();
        review.call("a", "b");
        let mut head = review.clone();
        head.call("a", "c").edit("a/src/lib.rs", 1);
        // Found by the server alone, on unchanged files.
        head.edge(
            "rust:b_src_lib.rs::f",
            "rust:c_src_lib.rs::f",
            EdgeKind::Calls,
            Provenance::Lsp,
        );
        let visible: HashSet<NodeId> = ["pkg:cargo/a", "pkg:cargo/b", "pkg:cargo/c"]
            .into_iter()
            .map(id)
            .collect();
        let diff = diff(&review, &head);
        let links = new_links(&review.graph(), &head.graph(), &diff, &visible);
        assert_eq!(
            links,
            HashSet::from([(id("pkg:cargo/a"), id("pkg:cargo/c"))])
        );
        // Package a open: the link leaves from its file.
        let visible: HashSet<NodeId> = visible
            .into_iter()
            .chain([id("file:a/src/lib.rs")])
            .collect();
        let links = new_links(&review.graph(), &head.graph(), &diff, &visible);
        assert_eq!(
            links,
            HashSet::from([(id("file:a/src/lib.rs"), id("pkg:cargo/c"))])
        );
    }
}
