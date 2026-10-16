//! Conversions between table rows and model types.

use rusqlite::Row;
use timonier_core::{
    Confidence, ContentHash, Edge, EdgeKind, Language, Node, NodeId, NodeKind, Position,
    Provenance, Snapshot, SnapshotKind, Span,
};

use crate::{Error, Result};

/// Columns read by [`RawNode::read`], in order.
pub(crate) const NODE_COLUMNS: &str = "id, kind, name, language, parent, signature, body_hash, start_line, start_column, end_line, end_column";

/// Columns read by [`RawEdge::read`], in order.
pub(crate) const EDGE_COLUMNS: &str = "src, dst, kind, provenance, confidence, weight";

pub(crate) const SNAPSHOT_COLUMNS: &str = "seq, kind, commit_sha, timestamp_ms, frozen";

/// A node row as stored; converted to a [`Node`] outside the SQLite callback, so that model
/// errors stay typed.
pub(crate) struct RawNode {
    id: String,
    kind: String,
    name: String,
    language: Option<String>,
    parent: Option<String>,
    signature: Option<String>,
    body_hash: Option<[u8; 32]>,
    span: [Option<i64>; 4],
}

impl RawNode {
    /// Reads the [`NODE_COLUMNS`] starting at column `at`.
    pub(crate) fn read(row: &Row<'_>, at: usize) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(at)?,
            kind: row.get(at + 1)?,
            name: row.get(at + 2)?,
            language: row.get(at + 3)?,
            parent: row.get(at + 4)?,
            signature: row.get(at + 5)?,
            body_hash: row.get(at + 6)?,
            span: [
                row.get(at + 7)?,
                row.get(at + 8)?,
                row.get(at + 9)?,
                row.get(at + 10)?,
            ],
        })
    }

    pub(crate) fn into_node(self) -> Result<Node> {
        let span = match self.span {
            [
                Some(start_line),
                Some(start_column),
                Some(end_line),
                Some(end_column),
            ] => Some(Span::new(
                Position::new(
                    to_u32("start_line", start_line)?,
                    to_u32("start_column", start_column)?,
                ),
                Position::new(
                    to_u32("end_line", end_line)?,
                    to_u32("end_column", end_column)?,
                ),
            )?),
            [None, None, None, None] => None,
            _ => {
                return Err(Error::Corrupt {
                    column: "start_line",
                    value: format!("{:?}", self.span),
                    expected: "a complete span or none",
                });
            }
        };
        Ok(Node {
            id: NodeId::parse(self.id)?,
            kind: NodeKind::parse(&self.kind)?,
            name: self.name,
            language: self.language.map(Language::new).transpose()?,
            span,
            parent: self.parent.map(NodeId::parse).transpose()?,
            signature: self.signature,
            body_hash: self.body_hash.map(ContentHash::from_bytes),
        })
    }
}

/// The span columns of a node, in [`NODE_COLUMNS`] order.
pub(crate) fn span_columns(span: Option<&Span>) -> [Option<i64>; 4] {
    match span {
        Some(span) => [
            Some(i64::from(span.start().line)),
            Some(i64::from(span.start().column)),
            Some(i64::from(span.end().line)),
            Some(i64::from(span.end().column)),
        ],
        None => [None; 4],
    }
}

pub(crate) struct RawEdge {
    src: String,
    dst: String,
    kind: String,
    provenance: String,
    confidence: f64,
    weight: i64,
}

impl RawEdge {
    /// Reads the [`EDGE_COLUMNS`] starting at column `at`.
    pub(crate) fn read(row: &Row<'_>, at: usize) -> rusqlite::Result<Self> {
        Ok(Self {
            src: row.get(at)?,
            dst: row.get(at + 1)?,
            kind: row.get(at + 2)?,
            provenance: row.get(at + 3)?,
            confidence: row.get(at + 4)?,
            weight: row.get(at + 5)?,
        })
    }

    pub(crate) fn into_edge(self) -> Result<Edge> {
        #[allow(clippy::cast_possible_truncation)] // Stored from an f32: the round trip is exact.
        let confidence = Confidence::new(self.confidence as f32)?;
        Ok(Edge {
            src: NodeId::parse(self.src)?,
            dst: NodeId::parse(self.dst)?,
            kind: EdgeKind::parse(&self.kind)?,
            provenance: Provenance::parse(&self.provenance)?,
            confidence,
            weight: to_u32("weight", self.weight)?,
        })
    }
}

pub(crate) struct RawSnapshot {
    seq: i64,
    kind: String,
    commit_sha: Option<String>,
    timestamp_ms: i64,
    frozen: bool,
}

impl RawSnapshot {
    pub(crate) fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            seq: row.get(0)?,
            kind: row.get(1)?,
            commit_sha: row.get(2)?,
            timestamp_ms: row.get(3)?,
            frozen: row.get(4)?,
        })
    }

    pub(crate) fn into_snapshot(self) -> Result<Snapshot> {
        Ok(Snapshot {
            seq: to_seq(self.seq)?,
            kind: SnapshotKind::parse(&self.kind)?,
            commit_sha: self.commit_sha,
            timestamp_ms: self.timestamp_ms,
            frozen: self.frozen,
        })
    }
}

pub(crate) fn to_u32(column: &'static str, value: i64) -> Result<u32> {
    u32::try_from(value).map_err(|_| Error::Corrupt {
        column,
        value: value.to_string(),
        expected: "a 32-bit unsigned integer",
    })
}

pub(crate) fn to_seq(value: i64) -> Result<u64> {
    u64::try_from(value).map_err(|_| Error::Corrupt {
        column: "seq",
        value: value.to_string(),
        expected: "a non-negative snapshot number",
    })
}

/// Snapshot numbers as SQLite integers; they stay far below `i64::MAX`.
pub(crate) fn seq_param(seq: u64) -> i64 {
    i64::try_from(seq).unwrap_or(i64::MAX)
}
