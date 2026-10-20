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
