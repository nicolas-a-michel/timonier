//! The review debt (ADR 0030): the files changed since the last review, written in sessions or
//! with none open, and how many each box holds. The views thread computes it with the head.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use timonier_core::graph::Graph;
use timonier_core::protocol::NodeDebt;
use timonier_core::{IdForm, NodeId, NodeKind, SnapshotKind};
use timonier_store::Reader;

use crate::tracking::{Tracker, changed_files};

/// The files to read again: all of them, and those changed with no session open.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Debt {
    pub files: BTreeSet<String>,
    pub untracked: BTreeSet<String>,
}

/// The files of the windows already closed, by their snapshots: they never change.
pub type Windows = HashMap<(u64, u64), Vec<String>>;

/// The debt since the last review (or the first snapshot), read in `root`'s `.timonier/`. A
/// tracking file that does not read counts for nothing: the tools say why.
pub fn debt(reader: &mut Reader, root: &Path, closed: &mut Windows) -> crate::Result<Debt> {
    let head = reader.head()?.seq;
    let Some(first) = reader.snapshots()?.first().map(|s| s.seq) else {
        return Ok(Debt::default());
    };
    let since = reader
        .latest(SnapshotKind::ReviewMark)?
        .map_or(first, |mark| mark.seq);
    let tracker = Tracker::new(root);
    let sessions = tracker.sessions();
    let mut debt = Debt::default();
    let mut window = |from: u64, to: u64, untracked: bool| -> crate::Result<()> {
        let from = from.max(since);
        if to <= from {
            return Ok(());
        }
        let files = match closed.get(&(from, to)) {
            Some(files) => files.clone(),
            None => {
                let files = changed_files(reader, Some(from), to)?;
                if to != head {
                    closed.insert((from, to), files.clone());
                }
                files
            }
        };
        if untracked {
            debt.untracked.extend(files.iter().cloned());
        }
        debt.files.extend(files);
        Ok(())
    };
    let mut open = false;
    for session in &sessions {
        let end = match session.snapshots.end {
            Some(end) => end,
            None => {
                open = true;
                head
            }
        };
        window(session.snapshots.start, end, false)?;
    }
    if let Ok(untracked) = tracker.untracked() {
        for gap in untracked.gaps {
            window(gap.from.max(first), gap.to, true)?;
        }
    }
    // No session open: what changes now is untracked, since the last one ended.
    if !open {
        let last = sessions
            .iter()
            .filter_map(|s| s.snapshots.end)
            .max()
            .unwrap_or(first);
        window(last, head, true)?;
    }
    Ok(debt)
}

/// How many of each box's files are to read again: every node above a file in debt, and the
/// file itself.
pub fn counts(graph: &Graph, debt: &Debt) -> HashMap<NodeId, NodeDebt> {
    let mut counts: HashMap<NodeId, NodeDebt> = HashMap::new();
    if debt.files.is_empty() {
        return counts;
    }
    for file in graph.nodes().filter(|n| n.kind == NodeKind::File) {
        let IdForm::File { path } = file.id.form() else {
            continue;
        };
        let (owed, untracked) = (debt.files.contains(path), debt.untracked.contains(path));
        for above in graph.ancestors(&file.id) {
            let count = counts.entry(above.clone()).or_insert(NodeDebt {
                files: 0,
                of: 0,
                untracked: 0,
            });
            count.of += 1;
            count.files += u32::from(owed);
            count.untracked += u32::from(untracked);
        }
    }
    counts.retain(|_, count| count.files > 0);
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use timonier_core::Node;

    fn id(s: &str) -> NodeId {
        NodeId::parse(s).unwrap()
    }

    fn node(i: &str, kind: NodeKind, parent: Option<&str>) -> Node {
        let mut node = Node::new(id(i), kind, i);
        if let Some(parent) = parent {
            node = node.with_parent(id(parent));
        }
        node
    }

    #[test]
    fn each_box_counts_its_files_to_read_again() {
        let graph = Graph::new(
            vec![
                node("repo:r", NodeKind::Repo, None),
                node("pkg:cargo/a", NodeKind::Package, Some("repo:r")),
                node("file:a/one.rs", NodeKind::File, Some("pkg:cargo/a")),
                node("file:a/two.rs", NodeKind::File, Some("pkg:cargo/a")),
                node("file:a/three.rs", NodeKind::File, Some("pkg:cargo/a")),
                node("pkg:cargo/b", NodeKind::Package, Some("repo:r")),
                node("file:b/one.rs", NodeKind::File, Some("pkg:cargo/b")),
            ],
            Vec::new(),
        );
        let debt = Debt {
            files: ["a/one.rs", "a/two.rs", "gone.rs"].map(String::from).into(),
            untracked: ["a/two.rs"].map(String::from).into(),
        };
        let counts = counts(&graph, &debt);
        assert_eq!(
            counts.get(&id("pkg:cargo/a")),
            Some(&NodeDebt {
                files: 2,
                of: 3,
                untracked: 1
            })
        );
        assert_eq!(counts.get(&id("file:a/one.rs")).map(|c| c.files), Some(1));
        // Nothing to read in b: no debt, rather than zero.
        assert!(!counts.contains_key(&id("pkg:cargo/b")));
        // A file gone from the head counts nowhere.
        assert_eq!(
            counts.get(&id("repo:r")).map(|c| (c.files, c.of)),
            Some((2, 4))
        );
    }
}
