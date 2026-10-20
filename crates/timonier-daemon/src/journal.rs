//! What a session changed in the code (ADR 0031): the review diff (ADR 0024) between its start
//! and its end, or the head while it is open. The snapshots of a session keep the contents of
//! their files until it ends, as a review mark does.

use std::collections::HashMap;

use timonier_core::protocol::{ReviewBox, ReviewMark, ReviewSummary};
use timonier_core::review;
use timonier_core::tracking::{ChangedBox, ChangedLink, SessionChanges};
use timonier_core::{ContentHash, Snapshot};
use timonier_store::Reader;

use crate::view::{self, Graph};

/// A session's start, as the diff needs it: read once per session.
pub struct Started {
    pub snapshot: Snapshot,
    pub graph: Graph,
    pub contents: HashMap<String, ContentHash>,
}

impl Started {
    pub fn read(reader: &mut Reader, seq: u64) -> crate::Result<Self> {
        Ok(Self {
            snapshot: reader.snapshot(seq)?,
            graph: Graph::new(reader.nodes_at(seq)?, reader.edges_at(seq)?),
            contents: reader.kept_contents(seq)?,
        })
    }
}

/// The changes from `started` to `now`, whose files have `contents`.
pub fn changes(
    started: &Started,
    now: &Graph,
    contents: &HashMap<String, ContentHash>,
) -> SessionChanges {
    let diff = review::compare(&started.graph, now, &started.contents, contents);
    let summary = view::summary(
        ReviewMark::from(&started.snapshot),
        &started.graph,
        now,
        &diff,
    );
    from_summary(
        u32::try_from(diff.changed_files.len()).unwrap_or(u32::MAX),
        summary,
    )
}

/// The changes between two snapshots of a session, read in the store.
pub fn between(reader: &mut Reader, start: u64, end: u64) -> crate::Result<SessionChanges> {
    reader.consistent(|reader| -> crate::Result<_> {
        let started = Started::read(reader, start)?;
        let ended = Graph::new(reader.nodes_at(end)?, reader.edges_at(end)?);
        Ok(changes(&started, &ended, &reader.kept_contents(end)?))
    })
}

fn from_summary(files: u32, summary: ReviewSummary) -> SessionChanges {
    let named = |item: ReviewBox| ChangedBox {
        id: item.id.to_string(),
        name: item.name,
        within: item.within,
    };
    let boxes = |items: Vec<ReviewBox>| items.into_iter().map(named).collect();
    let links = |items: Vec<timonier_core::protocol::ReviewDependency>| {
        items
            .into_iter()
            .map(|d| ChangedLink {
                from: named(d.src),
                to: named(d.dst),
            })
            .collect()
    };
    SessionChanges {
        files,
        added: boxes(summary.added),
        modified: boxes(summary.modified),
        removed: boxes(summary.removed),
        linked: links(summary.dependencies_added),
        unlinked: links(summary.dependencies_removed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use timonier_core::{Node, NodeId, NodeKind, SnapshotKind};

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

    fn hash(byte: u8) -> ContentHash {
        ContentHash::from_bytes([byte; 32])
    }

    /// Review n° 2 of Phase 6: the journal's changes are the review diff between the session's
    /// start and now, the boxes named as the summary names them.
    #[test]
    fn a_session_s_changes_name_the_boxes_it_touched() {
        let before = vec![
            node("repo:r", NodeKind::Repo, None),
            node("pkg:cargo/a", NodeKind::Package, Some("repo:r")),
            node("file:a/x.rs", NodeKind::File, Some("pkg:cargo/a")),
            node("pkg:cargo/b", NodeKind::Package, Some("repo:r")),
            node("file:b/y.rs", NodeKind::File, Some("pkg:cargo/b")),
        ];
        let started = Started {
            snapshot: Snapshot {
                seq: 3,
                commit_sha: None,
                timestamp_ms: 0,
                kind: SnapshotKind::WorkingTree,
                frozen: true,
            },
            graph: Graph::new(before.clone(), Vec::new()),
            contents: [("file:a/x.rs", hash(1)), ("file:b/y.rs", hash(2))]
                .map(|(k, v)| (k.to_owned(), v))
                .into(),
        };
        // a/x.rs changed; b untouched.
        let now = Graph::new(before, Vec::new());
        let contents: HashMap<String, ContentHash> =
            [("file:a/x.rs", hash(9)), ("file:b/y.rs", hash(2))]
                .map(|(k, v)| (k.to_owned(), v))
                .into();
        let changes = changes(&started, &now, &contents);
        assert_eq!(changes.files, 1);
        let modified: Vec<&str> = changes.modified.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(modified, ["pkg:cargo/a"]);
        assert!(changes.added.is_empty() && changes.removed.is_empty());
        // Nothing changed: nothing to say.
        let same = changes_of(&started);
        assert!(same.is_empty(), "{same:?}");
    }

    fn changes_of(started: &Started) -> SessionChanges {
        changes(started, &started.graph, &started.contents)
    }
}
