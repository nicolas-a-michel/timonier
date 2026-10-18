//! Reads, on read-only connections: state at a snapshot, diffs, aggregation (ADR 0010).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use timonier_core::diff::{self, RawDiff};
use timonier_core::{
    AggregatedEdge, ContentHash, Edge, EdgeKey, EdgeKind, GraphDiff, Node, NodeId, NodeKind,
    Provenance, ScopeKey, Snapshot, SnapshotKind, aggregate,
};

use crate::codec::{
    EDGE_COLUMNS, NODE_COLUMNS, RawEdge, RawNode, RawSnapshot, SNAPSHOT_COLUMNS, seq_param, to_seq,
};
use crate::{Error, Result, schema};

/// Valid at snapshot ?1 (ADR 0010).
const VALID_AT: &str = "valid_from <= ?1 AND (valid_to IS NULL OR valid_to > ?1)";

/// A read-only view of the store. Each method sees one consistent state, even while the writer
/// is working; open one reader per thread.
pub struct Reader {
    connection: Connection,
}

impl Reader {
    pub(crate) fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            connection: schema::open_reader(path)?,
        })
    }

    /// The scopes that hold something: `structure` and the files whose content was recorded.
    pub fn scopes(&self) -> Result<Vec<timonier_core::ScopeKey>> {
        let mut statement = self.connection.prepare_cached(
            "SELECT key FROM scope UNION SELECT DISTINCT scope FROM node WHERE valid_to IS NULL ORDER BY 1",
        )?;
        let keys = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        keys.into_iter()
            .map(|key| {
                timonier_core::ScopeKey::new(key.clone()).map_err(|_| Error::Corrupt {
                    column: "scope.key",
                    value: key,
                    expected: "`structure` or a file id",
                })
            })
            .collect()
    }

    /// The innermost symbol of a file's scope whose span contains `at`, at the head: where a
    /// language server's answer lands in the graph (ADR 0014).
    pub fn symbol_at(&self, file: &NodeId, at: timonier_core::Position) -> Result<Option<NodeId>> {
        let mut statement = self.connection.prepare_cached(
            "SELECT id FROM node
             WHERE scope = ?1 AND valid_to IS NULL AND start_line IS NOT NULL
               AND (start_line, start_column) <= (?2, ?3) AND (end_line, end_column) >= (?2, ?3)
             ORDER BY start_line DESC, start_column DESC LIMIT 1",
        )?;
        let id: Option<String> = statement
            .query_row(params![file.as_str(), at.line, at.column], |row| row.get(0))
            .optional()?;
        id.map(|id| {
            NodeId::parse(id.clone()).map_err(|_| Error::Corrupt {
                column: "node.id",
                value: id,
                expected: "a node id",
            })
        })
        .transpose()
    }

    /// What one source observed in every scope, with the kinds it verified there: what the
    /// daemon reloads after a restart, for the LSP (ADR 0014, 0018).
    pub fn observations(
        &self,
        source: Provenance,
    ) -> Result<Vec<(ScopeKey, timonier_core::SourceObservations)>> {
        let corrupt = |column: &'static str, value: String| Error::Corrupt {
            column,
            value,
            expected: "a valid value",
        };
        let mut by_scope: BTreeMap<String, timonier_core::SourceObservations> = BTreeMap::new();
        let mut statement = self.connection.prepare_cached(
            "SELECT scope, src, dst, kind, confidence, weight FROM observation WHERE source = ?1 ORDER BY scope, src, dst, kind",
        )?;
        let rows = statement
            .query_map([source.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, f64>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (scope, src, dst, kind, confidence, weight) in rows {
            #[allow(clippy::cast_possible_truncation)]
            // Stored from an f32: the round trip is exact.
            let confidence = timonier_core::Confidence::new(confidence as f32)?;
            let weight = u32::try_from(weight)
                .map_err(|_| corrupt("observation.weight", weight.to_string()))?;
            let observation = timonier_core::Observation::new(
                NodeId::parse(src)?,
                NodeId::parse(dst)?,
                EdgeKind::parse(&kind)?,
            )
            .with_confidence(confidence)
            .with_weight(weight);
            by_scope
                .entry(scope)
                .or_insert_with(|| timonier_core::SourceObservations::new(source, Vec::new()))
                .observations
                .push(observation);
        }
        let mut statement = self.connection.prepare_cached(
            "SELECT scope, kind FROM verification WHERE source = ?1 ORDER BY scope, kind",
        )?;
        let rows = statement
            .query_map([source.as_str()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (scope, kind) in rows {
            by_scope
                .entry(scope)
                .or_insert_with(|| timonier_core::SourceObservations::new(source, Vec::new()))
                .verified
                .push(EdgeKind::parse(&kind)?);
        }
        by_scope
            .into_iter()
            .map(|(scope, observations)| {
                let key = ScopeKey::new(scope.clone())
                    .map_err(|_| corrupt("observation.scope", scope))?;
                Ok((key, observations))
            })
            .collect()
    }

    /// The whole extraction cache, by path (ADR 0013).
    pub fn extractions(&self) -> Result<Vec<crate::CachedExtraction>> {
        crate::extraction::load(&self.connection)
    }

    /// What the LSP refinement owed when it was last saved (ADR 0022).
    pub fn refinement_debt(&self) -> Result<crate::RefinementDebt> {
        crate::refinement::load(&self.connection)
    }

    /// The open snapshot every change is recorded in.
    pub fn head(&self) -> Result<Snapshot> {
        let raw = self
            .connection
            .query_row(
                &format!("SELECT {SNAPSHOT_COLUMNS} FROM snapshot WHERE frozen = 0"),
                [],
                RawSnapshot::read,
            )
            .optional()?;
        raw.ok_or(Error::HeadCount(0))?.into_snapshot()
    }

    pub fn snapshot(&self, seq: u64) -> Result<Snapshot> {
        let raw = self
            .connection
            .query_row(
                &format!("SELECT {SNAPSHOT_COLUMNS} FROM snapshot WHERE seq = ?1"),
                [seq_param(seq)],
                RawSnapshot::read,
            )
            .optional()?;
        raw.ok_or(Error::UnknownSnapshot(seq))?.into_snapshot()
    }

    /// Every snapshot, oldest first; the last one is the head.
    pub fn snapshots(&self) -> Result<Vec<Snapshot>> {
        let mut statement = self.connection.prepare_cached(&format!(
            "SELECT {SNAPSHOT_COLUMNS} FROM snapshot ORDER BY seq"
        ))?;
        let raws = statement
            .query_map([], RawSnapshot::read)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        raws.into_iter().map(RawSnapshot::into_snapshot).collect()
    }

    /// The most recent frozen snapshot of a kind, such as the last review mark.
    pub fn latest(&self, kind: SnapshotKind) -> Result<Option<Snapshot>> {
        let raw = self
            .connection
            .query_row(
                &format!("SELECT {SNAPSHOT_COLUMNS} FROM snapshot WHERE frozen = 1 AND kind = ?1 ORDER BY seq DESC LIMIT 1"),
                [kind.as_str()],
                RawSnapshot::read,
            )
            .optional()?;
        raw.map(RawSnapshot::into_snapshot).transpose()
    }

    /// Every node valid at `seq`, sorted by id.
    pub fn nodes_at(&mut self, seq: u64) -> Result<Vec<Node>> {
        let tx = self.connection.transaction()?;
        let sql = format!("SELECT {NODE_COLUMNS} FROM node WHERE {{valid}} ORDER BY id");
        let raws = rows_at(&tx, &sql, seq, |row| RawNode::read(row, 0))?;
        raws.into_iter().map(RawNode::into_node).collect()
    }

    /// Every merged edge valid at `seq`, sorted by key. Dangling edges are included: views
    /// filter them (ADR 0009).
    pub fn edges_at(&mut self, seq: u64) -> Result<Vec<Edge>> {
        let tx = self.connection.transaction()?;
        edges_valid_at(&tx, seq)
    }

    pub fn node_at(&self, id: &NodeId, seq: u64) -> Result<Option<Node>> {
        node_valid_at(&self.connection, id, seq)
    }

    /// The content hash a scope's observations were computed on.
    pub fn content_hash(&self, scope: &ScopeKey) -> Result<Option<ContentHash>> {
        let hash: Option<Option<[u8; 32]>> = self
            .connection
            .query_row(
                "SELECT content_hash FROM scope WHERE key = ?1",
                [scope.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        Ok(hash.flatten().map(ContentHash::from_bytes))
    }

    /// Edge kinds `source` analysed completely in `scope`, at its current content hash: the
    /// spec's « vérifié par LSP au hash X ».
    pub fn verified(&self, scope: &ScopeKey, source: Provenance) -> Result<Vec<EdgeKind>> {
        let mut statement = self.connection.prepare_cached(
            "SELECT kind FROM verification WHERE scope = ?1 AND source = ?2 ORDER BY kind",
        )?;
        let kinds = statement
            .query_map(params![scope.as_str(), source.as_str()], |row| {
                row.get::<_, String>(0)
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        kinds
            .iter()
            .map(|kind| Ok(EdgeKind::parse(kind)?))
            .collect()
    }

    /// The structural diff from `from` to `to`. Only the rows that changed in between are read:
    /// the cost follows the size of the change, not of the graph.
    pub fn diff(&mut self, from: u64, to: u64) -> Result<GraphDiff> {
        if to < from {
            return Err(Error::BackwardDiff { from, to });
        }
        let tx = self.connection.transaction()?;
        known(&tx, from)?;
        known(&tx, to)?;

        let mut raw = RawDiff::default();
        let (before, after) = changed_nodes(&tx, from, to)?;
        let mut after: HashMap<NodeId, Node> =
            after.into_iter().map(|n| (n.id.clone(), n)).collect();
        for node in before {
            match after.remove(&node.id) {
                Some(new) if new.same_version(&node) => {}
                Some(new) => raw.modified_nodes.push((node, new)),
                None => raw.removed_nodes.push(node),
            }
        }
        raw.added_nodes.extend(after.into_values());

        let (before, after) = changed_edges(&tx, from, to)?;
        let mut after: HashMap<EdgeKey, Edge> = after.into_iter().map(|e| (e.key(), e)).collect();
        for edge in before {
            match after.remove(&edge.key()) {
                Some(new) if new.same_version(&edge) => {}
                Some(new) => raw.modified_edges.push((edge, new)),
                None => raw.removed_edges.push(edge),
            }
        }
        raw.added_edges.extend(after.into_values());

        // Where removed and modified symbols were, and where added and modified symbols are:
        // move detection needs both sides (ADR 0010).
        let before_side = raw
            .removed_nodes
            .iter()
            .chain(raw.modified_nodes.iter().map(|(before, _)| before));
        for node in before_side.filter(|n| n.kind.is_symbol()) {
            if let Some(file) = containing_file(&tx, node, from)? {
                raw.file_before.insert(node.id.clone(), file);
            }
        }
        let after_side = raw
            .added_nodes
            .iter()
            .chain(raw.modified_nodes.iter().map(|(_, after)| after));
        for node in after_side.filter(|n| n.kind.is_symbol()) {
            if let Some(file) = containing_file(&tx, node, to)? {
                raw.file_after.insert(node.id.clone(), file);
            }
        }
        Ok(diff::assemble(from, to, raw))
    }

    /// Edges at a zoom level: every edge valid at `seq`, re-attached to the nearest visible
    /// ancestor of its endpoints (ADR 0009). Nothing is stored.
    pub fn aggregate(&mut self, seq: u64, visible: &[NodeId]) -> Result<Vec<AggregatedEdge>> {
        let tx = self.connection.transaction()?;
        let rows = rows_at(
            &tx,
            "SELECT id, parent FROM node WHERE {valid}",
            seq,
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )?;
        let parents = rows
            .into_iter()
            .map(|(id, parent)| Ok((NodeId::parse(id)?, parent.map(NodeId::parse).transpose()?)))
            .collect::<Result<HashMap<_, _>>>()?;
        let edges = edges_valid_at(&tx, seq)?;
        let visible: HashSet<NodeId> = visible.iter().cloned().collect();
        Ok(aggregate::aggregate(&parents, &edges, &visible))
    }
}

fn known(connection: &Connection, seq: u64) -> Result<()> {
    let exists: Option<i64> = connection
        .query_row(
            "SELECT 1 FROM snapshot WHERE seq = ?1",
            [seq_param(seq)],
            |row| row.get(0),
        )
        .optional()?;
    exists.map(|_| ()).ok_or(Error::UnknownSnapshot(seq))
}

fn node_valid_at(connection: &Connection, id: &NodeId, seq: u64) -> Result<Option<Node>> {
    let mut statement = connection.prepare_cached(&format!(
        "SELECT {NODE_COLUMNS} FROM node WHERE id = ?2 AND {VALID_AT}"
    ))?;
    let raw = statement
        .query_row(params![seq_param(seq), id.as_str()], |row| {
            RawNode::read(row, 0)
        })
        .optional()?;
    raw.map(RawNode::into_node).transpose()
}

fn edges_valid_at(connection: &Connection, seq: u64) -> Result<Vec<Edge>> {
    let sql = format!("SELECT {EDGE_COLUMNS} FROM edge WHERE {{valid}} ORDER BY src, dst, kind");
    let raws = rows_at(connection, &sql, seq, |row| RawEdge::read(row, 0))?;
    raws.into_iter().map(RawEdge::into_edge).collect()
}

/// Rows of `sql` valid at `seq`; `sql` holds `{valid}` where the condition goes. At the head,
/// the condition is `valid_to IS NULL`, which the partial indexes of current rows serve without
/// walking through the history.
fn rows_at<T>(
    connection: &Connection,
    sql: &str,
    seq: u64,
    read: impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
) -> Result<Vec<T>> {
    let rows = if is_head(connection, seq)? {
        let mut statement =
            connection.prepare_cached(&sql.replace("{valid}", "valid_to IS NULL"))?;
        statement
            .query_map([], read)?
            .collect::<rusqlite::Result<Vec<_>>>()?
    } else {
        let mut statement = connection.prepare_cached(&sql.replace("{valid}", VALID_AT))?;
        statement
            .query_map([seq_param(seq)], read)?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    Ok(rows)
}

/// Whether `seq` is the open head; an error if no such snapshot exists.
fn is_head(connection: &Connection, seq: u64) -> Result<bool> {
    let frozen: Option<bool> = connection
        .query_row(
            "SELECT frozen FROM snapshot WHERE seq = ?1",
            [seq_param(seq)],
            |row| row.get(0),
        )
        .optional()?;
    frozen
        .map(|frozen| !frozen)
        .ok_or(Error::UnknownSnapshot(seq))
}

/// Rows that changed in ]from, to]: the versions valid at `from` that ended there, and the
/// versions valid at `to` that started there. Versions born and gone in between are skipped.
const CHANGED: &str =
    "(valid_from > ?1 AND valid_from <= ?2) OR (valid_to > ?1 AND valid_to <= ?2)";

/// A row's interval decides on which side of the diff it is.
fn side(valid_from: u64, valid_to: Option<u64>, from: u64, to: u64) -> Option<bool> {
    let valid_at = |s: u64| valid_from <= s && valid_to.is_none_or(|end| end > s);
    match (valid_at(from), valid_at(to)) {
        (true, false) => Some(false),
        (false, true) => Some(true),
        _ => None,
    }
}

fn changed_nodes(connection: &Connection, from: u64, to: u64) -> Result<(Vec<Node>, Vec<Node>)> {
    let mut statement = connection.prepare_cached(&format!(
        "SELECT valid_from, valid_to, {NODE_COLUMNS} FROM node WHERE {CHANGED} ORDER BY id, valid_from"
    ))?;
    let rows = statement
        .query_map(params![seq_param(from), seq_param(to)], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<i64>>(1)?,
                RawNode::read(row, 2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let (mut before, mut after) = (Vec::new(), Vec::new());
    for (valid_from, valid_to, raw) in rows {
        match side(
            to_seq(valid_from)?,
            valid_to.map(to_seq).transpose()?,
            from,
            to,
        ) {
            Some(false) => before.push(raw.into_node()?),
            Some(true) => after.push(raw.into_node()?),
            None => {}
        }
    }
    Ok((before, after))
}

fn changed_edges(connection: &Connection, from: u64, to: u64) -> Result<(Vec<Edge>, Vec<Edge>)> {
    let mut statement = connection.prepare_cached(&format!(
        "SELECT valid_from, valid_to, {EDGE_COLUMNS} FROM edge WHERE {CHANGED} ORDER BY src, dst, kind, valid_from"
    ))?;
    let rows = statement
        .query_map(params![seq_param(from), seq_param(to)], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<i64>>(1)?,
                RawEdge::read(row, 2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let (mut before, mut after) = (Vec::new(), Vec::new());
    for (valid_from, valid_to, raw) in rows {
        match side(
            to_seq(valid_from)?,
            valid_to.map(to_seq).transpose()?,
            from,
            to,
        ) {
            Some(false) => before.push(raw.into_edge()?),
            Some(true) => after.push(raw.into_edge()?),
            None => {}
        }
    }
    Ok((before, after))
}

/// The file a symbol is in at `seq`, by walking up its parents.
fn containing_file(connection: &Connection, symbol: &Node, seq: u64) -> Result<Option<NodeId>> {
    let mut parent = symbol.parent.clone();
    for _ in 0..256 {
        let Some(id) = parent else { return Ok(None) };
        let Some(node) = node_valid_at(connection, &id, seq)? else {
            return Ok(None);
        };
        match node.kind {
            NodeKind::File => return Ok(Some(node.id)),
            NodeKind::Symbol(_) => parent = node.parent,
            _ => return Ok(None),
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::{CHANGED, VALID_AT};
    use crate::codec::{EDGE_COLUMNS, NODE_COLUMNS};
    use crate::schema;

    /// The query plan SQLite (the bundled one) picks for `sql`, with literal parameters.
    fn plan(connection: &Connection, sql: &str) -> String {
        let sql = sql.replace("?1", "1").replace("?2", "2");
        let mut statement = connection
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap();
        let details: Vec<String> = statement
            .query_map([], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        details.join(" | ")
    }

    #[test]
    fn reads_use_the_indexes_they_were_designed_for() {
        let dir = tempfile::tempdir().unwrap();
        let connection = schema::open_writer(&dir.path().join("graph.sqlite")).unwrap();

        // At the head: the partial indexes of current rows, no walk through the history.
        let nodes = plan(
            &connection,
            &format!("SELECT {NODE_COLUMNS} FROM node WHERE valid_to IS NULL ORDER BY id"),
        );
        assert!(nodes.contains("node_current_id"), "{nodes}");
        let edges = plan(
            &connection,
            &format!(
                "SELECT {EDGE_COLUMNS} FROM edge WHERE valid_to IS NULL ORDER BY src, dst, kind"
            ),
        );
        assert!(edges.contains("edge_current_key"), "{edges}");

        // A diff reads the rows whose interval starts or ends in ]from, to], through both indexes.
        let changed = plan(
            &connection,
            &format!("SELECT valid_from FROM node WHERE {CHANGED}"),
        );
        assert!(
            changed.contains("node_valid_from") && changed.contains("node_valid_to"),
            "{changed}"
        );
        let changed = plan(
            &connection,
            &format!("SELECT valid_from FROM edge WHERE {CHANGED}"),
        );
        assert!(
            changed.contains("edge_valid_from") && changed.contains("edge_valid_to"),
            "{changed}"
        );

        // In the past: an index, not a full scan.
        let past = plan(
            &connection,
            &format!("SELECT {NODE_COLUMNS} FROM node WHERE {VALID_AT} ORDER BY id"),
        );
        assert!(past.contains("USING INDEX"), "{past}");
    }
}
