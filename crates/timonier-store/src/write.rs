//! Writes: scope updates (ADR 0009) and freezing the head (ADR 0010). They run on the writer
//! thread, one transaction per call.
//!
//! Ownership (ADR 0009): when several scopes provide the same node, the scope with the smallest
//! key owns it; for an edge, the most reliable provenance wins, then the smallest key. The other
//! contributions wait in `node_claim` / `edge_claim`, and the next one takes over when the owner
//! lets go. The head therefore depends only on what every scope currently provides, never on the
//! order of the updates.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use timonier_core::tree::{self, CheckError};
use timonier_core::{
    ContentHash, Edge, EdgeKey, EdgeKind, Freeze, Node, NodeId, NodeKind, Observation, Provenance,
    Role, ScopeKey, ScopeUpdate, Snapshot, SourceObservations, TreeViolation, fusion,
};

use crate::codec::{
    EDGE_COLUMNS, NODE_COLUMNS, RawEdge, RawNode, seq_param, span_columns, to_seq, to_u32,
};
use crate::{Error, Result};

/// What a write changed. Re-applying an identical update changes nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ApplyStats {
    pub nodes_added: u32,
    pub nodes_removed: u32,
    pub nodes_changed: u32,
    /// Nodes whose span moved with no other change (spans are not versioned, ADR 0010).
    pub spans_moved: u32,
    pub edges_added: u32,
    pub edges_removed: u32,
    pub edges_changed: u32,
    /// Nodes and edges whose owner scope changed (ADR 0009).
    pub nodes_taken_over: u32,
    pub edges_taken_over: u32,
    /// Contributions recorded as waiting: another scope, with a smaller key, provides the same
    /// node or edge (ADR 0009).
    pub nodes_waiting: u32,
    pub edges_waiting: u32,
    /// Observation sets ignored because they were computed on another content than the scope's
    /// current one (ADR 0009).
    pub stale_observations: u32,
}

impl ApplyStats {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// A current row and the snapshot it became valid at.
struct Stored<T> {
    version_id: i64,
    valid_from: u64,
    value: T,
}

/// A current row and the scope that owns it.
struct Owned<T> {
    scope: String,
    stored: Stored<T>,
}

/// Tree checks deferred to the end of the batch, so that its updates may come in any order.
#[derive(Default)]
struct PendingChecks {
    /// Inserted or changed nodes.
    placed: Vec<NodeId>,
    /// Existing nodes whose kind or parent changed: their whole subtree is checked again.
    restructured: Vec<NodeId>,
    /// Nodes that disappeared: nothing may stay under them.
    removed: Vec<NodeId>,
    /// Nodes retired in this batch: if another scope brings one back, its subtree is re-checked.
    retired: HashSet<NodeId>,
    repo_placed: bool,
}

pub(crate) fn head_seq(connection: &Connection) -> Result<u64> {
    let mut statement = connection.prepare_cached("SELECT seq FROM snapshot WHERE frozen = 0")?;
    let heads: Vec<i64> = statement
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    match heads.as_slice() {
        [seq] => to_seq(*seq),
        other => Err(Error::HeadCount(other.len() as u64)),
    }
}

/// Applies a batch of scope updates in one transaction.
pub(crate) fn apply(connection: &mut Connection, updates: &[ScopeUpdate]) -> Result<ApplyStats> {
    apply_owing(connection, updates, &crate::DebtChange::default())
}

/// [`apply`], and what the LSP refinement owes after it, in the same transaction (ADR 0022).
pub(crate) fn apply_owing(
    connection: &mut Connection,
    updates: &[ScopeUpdate],
    debt: &crate::DebtChange,
) -> Result<ApplyStats> {
    let tx = connection.transaction()?;
    let head = head_seq(&tx)?;
    let mut stats = ApplyStats::default();
    let mut checks = PendingChecks::default();
    for update in updates {
        apply_one(&tx, head, update, &mut stats, &mut checks)?;
    }
    check_tree(&tx, &checks)?;
    crate::refinement::write_change(&tx, debt)?;
    tx.commit()?;
    Ok(stats)
}

fn apply_one(
    tx: &Transaction<'_>,
    head: u64,
    update: &ScopeUpdate,
    stats: &mut ApplyStats,
    checks: &mut PendingChecks,
) -> Result<()> {
    let key = &update.scope;
    let scope = key.as_str();
    let mut edges_dirty = false;
    if update.remove {
        if update.content_hash.is_some() || update.nodes.is_some() || update.observations.is_some()
        {
            return Err(Error::RemovalWithContent { scope: key.clone() });
        }
        tx.execute("DELETE FROM observation WHERE scope = ?1", [scope])?;
        tx.execute("DELETE FROM verification WHERE scope = ?1", [scope])?;
        tx.execute("DELETE FROM scope WHERE key = ?1", [scope])?;
        replace_nodes(tx, head, key, &[], stats, checks)?;
        edges_dirty = true;
    } else {
        if let Some(observations) = &update.observations {
            observations.validate()?;
            // The content of a scope comes from the pass that reads it (tree-sitter, the
            // structure): a precise source brings neither the content hash nor nodes, it only
            // says what it saw on a given content.
            let brings_content = update.content_hash.is_some() || update.nodes.is_some();
            if brings_content && observations.source.is_precise() {
                return Err(Error::PreciseSourceSetsContent {
                    scope: key.clone(),
                    provenance: observations.source,
                });
            }
        }
        if let Some(hash) = &update.content_hash {
            // The source of this update keeps its observations if they describe the new content.
            let keep = update
                .observations
                .as_ref()
                .filter(|o| o.computed_on.is_none_or(|on| on == *hash))
                .map(|o| o.source);
            edges_dirty |= set_content_hash(tx, scope, hash, keep)?;
        }
        if let Some(nodes) = &update.nodes {
            replace_nodes(tx, head, key, nodes, stats, checks)?;
        }
        if let Some(observations) = &update.observations {
            if anchored(tx, key, observations)? {
                check_sources(tx, key, observations)?;
                edges_dirty |= replace_observations(tx, scope, observations)?;
            } else {
                stats.stale_observations += 1;
            }
        }
    }
    if edges_dirty {
        recompute_edges(tx, head, scope, stats)?;
    }
    Ok(())
}

fn stored_hash(tx: &Transaction<'_>, scope: &str) -> Result<Option<ContentHash>> {
    let stored: Option<Option<[u8; 32]>> = tx
        .query_row(
            "SELECT content_hash FROM scope WHERE key = ?1",
            [scope],
            |row| row.get(0),
        )
        .optional()?;
    Ok(stored.flatten().map(ContentHash::from_bytes))
}

/// Whether observations describe the scope's current content (ADR 0009). Observations computed
/// on another content are stale (a slow LSP answer arriving after the file changed), and so are
/// observations about a scope that has no content (removed, or not read yet). A precise source
/// must always say which content it analysed; an approximate one may omit it, since it comes
/// with the content hash, from the same pass.
fn anchored(
    tx: &Transaction<'_>,
    scope: &ScopeKey,
    observations: &SourceObservations,
) -> Result<bool> {
    if observations.source.is_precise() && observations.computed_on.is_none() {
        return Err(Error::UnanchoredObservations {
            scope: scope.clone(),
            provenance: observations.source,
        });
    }
    Ok(
        match (observations.computed_on, stored_hash(tx, scope.as_str())?) {
            (Some(on), Some(current)) => on == current,
            // A file scope without content (removed, or not read yet) takes no observation;
            // the structure has no content hash and takes the manifests' observations.
            (_, None) => scope.file().is_none() && observations.computed_on.is_none(),
            (None, Some(_)) => true,
        },
    )
}

/// An observed edge leaves something the scope provides: its file, or one of its nodes (owned or
/// waiting). Edges from two scopes can then only meet on a node both provide.
fn check_sources(
    tx: &Transaction<'_>,
    scope: &ScopeKey,
    observations: &SourceObservations,
) -> Result<()> {
    if observations.observations.is_empty() {
        return Ok(());
    }
    let mut statement = tx.prepare_cached(
        "SELECT id FROM node WHERE scope = ?1 AND valid_to IS NULL UNION SELECT id FROM node_claim WHERE scope = ?1",
    )?;
    let mut provided: HashSet<String> = statement
        .query_map([scope.as_str()], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    if let Some(file) = scope.file() {
        provided.insert(file.to_string());
    }
    match observations
        .observations
        .iter()
        .find(|o| !provided.contains(o.src.as_str()))
    {
        Some(foreign) => Err(Error::ForeignSource {
            scope: scope.clone(),
            src: foreign.src.clone(),
        }),
        None => Ok(()),
    }
}

/// Records a new content hash. The observations of the other sources were computed on the old
/// content: they become stale and are dropped (ADR 0009). Returns whether any was dropped.
fn set_content_hash(
    tx: &Transaction<'_>,
    scope: &str,
    hash: &ContentHash,
    keep: Option<Provenance>,
) -> Result<bool> {
    if stored_hash(tx, scope)? == Some(*hash) {
        return Ok(false);
    }
    tx.execute(
        "INSERT INTO scope (key, content_hash) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET content_hash = excluded.content_hash",
        params![scope, hash.as_bytes()],
    )?;
    let keep = keep.map_or("", Provenance::as_str);
    let stale = tx.execute(
        "DELETE FROM observation WHERE scope = ?1 AND source <> ?2",
        params![scope, keep],
    )? + tx.execute(
        "DELETE FROM verification WHERE scope = ?1 AND source <> ?2",
        params![scope, keep],
    )?;
    Ok(stale > 0)
}

// ---------------------------------------------------------------------------------------------
// Nodes
// ---------------------------------------------------------------------------------------------

/// What a scope may hold (ADR 0009): the structure holds the repo, packages, components and
/// files; a file scope holds the symbols of its file, under the file or under one another.
fn check_scope_content(scope: &ScopeKey, nodes: &[Node]) -> Result<()> {
    let out_of_scope = |node: &Node, reason| Error::OutOfScope {
        scope: scope.clone(),
        id: node.id.clone(),
        reason,
    };
    let Some(file) = scope.file() else {
        return match nodes.iter().find(|n| n.kind.is_symbol()) {
            Some(symbol) => Err(out_of_scope(
                symbol,
                "the structure holds the repo, packages, components and files; symbols belong to their file's scope",
            )),
            None => Ok(()),
        };
    };
    let ids: HashSet<&NodeId> = nodes.iter().map(|n| &n.id).collect();
    for node in nodes {
        if !node.kind.is_symbol() {
            return Err(out_of_scope(
                node,
                "a file scope holds the symbols of its file only",
            ));
        }
        if !node
            .parent
            .as_ref()
            .is_some_and(|p| *p == file || ids.contains(p))
        {
            return Err(out_of_scope(
                node,
                "a symbol sits under its file or under another symbol of the same file",
            ));
        }
    }
    Ok(())
}

fn replace_nodes(
    tx: &Transaction<'_>,
    head: u64,
    key: &ScopeKey,
    nodes: &[Node],
    stats: &mut ApplyStats,
    checks: &mut PendingChecks,
) -> Result<()> {
    let scope = key.as_str();
    check_scope_content(key, nodes)?;
    let mut wanted: HashSet<&NodeId> = HashSet::with_capacity(nodes.len());
    for node in nodes {
        node.check_identity()?;
        if !wanted.insert(&node.id) {
            return Err(Error::DuplicateNode {
                id: node.id.clone(),
                scope: key.clone(),
            });
        }
    }
    let owned = current_nodes(tx, scope)?;
    let waiting = node_claims(tx, scope)?;

    // What the scope no longer provides: the next contribution takes over, or it disappears.
    let mut gone: Vec<(&NodeId, &Stored<Node>)> = owned
        .iter()
        .filter(|(id, _)| !wanted.contains(id))
        .collect();
    gone.sort_by(|a, b| a.0.cmp(b.0));
    for (id, stored) in gone {
        match next_node_claim(tx, id)? {
            Some((claimant, node)) => {
                delete_node_claim(tx, &claimant, id)?;
                hand_node_over(tx, head, stored, &claimant, &node, stats, checks)?;
            }
            None => {
                retire(tx, Table::Node, stored, head)?;
                stats.nodes_removed += 1;
                checks.removed.push(id.clone());
                checks.retired.insert(id.clone());
            }
        }
    }
    for id in waiting.keys().filter(|id| !wanted.contains(id)) {
        delete_node_claim(tx, scope, id)?;
    }

    for node in nodes {
        checks.repo_placed |= node.kind == NodeKind::Repo;
        if let Some(stored) = owned.get(&node.id) {
            update_owned_node(tx, head, scope, stored, node, stats, checks)?;
            continue;
        }
        match node_anywhere(tx, &node.id)? {
            None => {
                insert_node(tx, scope, node, head)?;
                stats.nodes_added += 1;
                checks.placed.push(node.id.clone());
                if checks.retired.contains(&node.id) {
                    // Retired by another scope earlier in the batch: what is under it must fit.
                    checks.restructured.push(node.id.clone());
                }
            }
            Some(owner) if scope < owner.scope.as_str() => {
                // The smallest key owns: the current owner's version now waits.
                upsert_node_claim(tx, &owner.scope, &owner.stored.value)?;
                stats.nodes_waiting += 1;
                if waiting.contains_key(&node.id) {
                    delete_node_claim(tx, scope, &node.id)?;
                }
                hand_node_over(tx, head, &owner.stored, scope, node, stats, checks)?;
            }
            Some(_) => {
                if waiting.get(&node.id) != Some(node) {
                    upsert_node_claim(tx, scope, node)?;
                    stats.nodes_waiting += 1;
                }
            }
        }
    }
    Ok(())
}

/// Brings an owned row in line with the scope's version of the node.
fn update_owned_node(
    tx: &Transaction<'_>,
    head: u64,
    scope: &str,
    stored: &Stored<Node>,
    node: &Node,
    stats: &mut ApplyStats,
    checks: &mut PendingChecks,
) -> Result<()> {
    if !stored.value.same_version(node) {
        if stored.valid_from == head {
            update_node_in_place(tx, stored.version_id, node)?;
        } else {
            close(tx, Table::Node, stored.version_id, head)?;
            insert_node(tx, scope, node, head)?;
        }
        stats.nodes_changed += 1;
        checks.placed.push(node.id.clone());
        if stored.value.kind != node.kind || stored.value.parent != node.parent {
            checks.restructured.push(node.id.clone());
        }
    } else if stored.value.span != node.span {
        update_span(tx, stored.version_id, node)?;
        stats.spans_moved += 1;
    }
    Ok(())
}

/// Gives a node to another scope, with that scope's version of it.
fn hand_node_over(
    tx: &Transaction<'_>,
    head: u64,
    stored: &Stored<Node>,
    new_owner: &str,
    node: &Node,
    stats: &mut ApplyStats,
    checks: &mut PendingChecks,
) -> Result<()> {
    stats.nodes_taken_over += 1;
    // A version that gets closed keeps the scope that owned it then; only a row that stays the
    // current version changes owner.
    if stored.value.same_version(node) || stored.valid_from == head {
        set_scope(tx, Table::Node, stored.version_id, new_owner)?;
    }
    update_owned_node(tx, head, new_owner, stored, node, stats, checks)
}

fn read_stored_nodes(
    tx: &Transaction<'_>,
    sql: &str,
    param: &str,
) -> Result<Vec<(String, Stored<Node>)>> {
    let mut statement = tx.prepare_cached(sql)?;
    let rows = statement
        .query_map([param], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                RawNode::read(row, 3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter()
        .map(|(scope, version_id, valid_from, raw)| {
            Ok((
                scope,
                Stored {
                    version_id,
                    valid_from: to_seq(valid_from)?,
                    value: raw.into_node()?,
                },
            ))
        })
        .collect()
}

fn current_nodes(tx: &Transaction<'_>, scope: &str) -> Result<HashMap<NodeId, Stored<Node>>> {
    let sql = format!(
        "SELECT scope, version_id, valid_from, {NODE_COLUMNS} FROM node WHERE scope = ?1 AND valid_to IS NULL"
    );
    Ok(read_stored_nodes(tx, &sql, scope)?
        .into_iter()
        .map(|(_, stored)| (stored.value.id.clone(), stored))
        .collect())
}

/// The current version of a node and its owner, whatever scope that is.
fn node_anywhere(tx: &Transaction<'_>, id: &NodeId) -> Result<Option<Owned<Node>>> {
    let sql = format!(
        "SELECT scope, version_id, valid_from, {NODE_COLUMNS} FROM node WHERE id = ?1 AND valid_to IS NULL"
    );
    Ok(read_stored_nodes(tx, &sql, id.as_str())?
        .into_iter()
        .next()
        .map(|(scope, stored)| Owned { scope, stored }))
}

fn node_claims(tx: &Transaction<'_>, scope: &str) -> Result<HashMap<NodeId, Node>> {
    let mut statement = tx.prepare_cached(&format!(
        "SELECT {NODE_COLUMNS} FROM node_claim WHERE scope = ?1"
    ))?;
    let raws = statement
        .query_map([scope], |row| RawNode::read(row, 0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    raws.into_iter()
        .map(|raw| {
            let node = raw.into_node()?;
            Ok((node.id.clone(), node))
        })
        .collect()
}

/// The waiting contribution with the smallest scope key: the next owner.
fn next_node_claim(tx: &Transaction<'_>, id: &NodeId) -> Result<Option<(String, Node)>> {
    let mut statement = tx.prepare_cached(&format!(
        "SELECT scope, {NODE_COLUMNS} FROM node_claim WHERE id = ?1 ORDER BY scope LIMIT 1"
    ))?;
    let row = statement
        .query_row([id.as_str()], |row| {
            Ok((row.get::<_, String>(0)?, RawNode::read(row, 1)?))
        })
        .optional()?;
    row.map(|(scope, raw)| Ok((scope, raw.into_node()?)))
        .transpose()
}

fn upsert_node_claim(tx: &Transaction<'_>, scope: &str, node: &Node) -> Result<()> {
    let [start_line, start_column, end_line, end_column] = span_columns(node.span.as_ref());
    tx.prepare_cached(
        "INSERT INTO node_claim (scope, id, kind, name, language, parent, signature, body_hash,
                                 start_line, start_column, end_line, end_column, role)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT (id, scope) DO UPDATE SET
             kind = excluded.kind, name = excluded.name, language = excluded.language,
             parent = excluded.parent, signature = excluded.signature, body_hash = excluded.body_hash,
             start_line = excluded.start_line, start_column = excluded.start_column,
             end_line = excluded.end_line, end_column = excluded.end_column, role = excluded.role",
    )?
    .execute(params![
        scope,
        node.id.as_str(),
        node.kind.as_str(),
        node.name,
        node.language.as_ref().map(|l| l.as_str()),
        node.parent.as_ref().map(NodeId::as_str),
        node.signature,
        node.body_hash.as_ref().map(ContentHash::as_bytes),
        start_line,
        start_column,
        end_line,
        end_column,
        node.role.map(Role::as_str),
    ])?;
    Ok(())
}

fn delete_node_claim(tx: &Transaction<'_>, scope: &str, id: &NodeId) -> Result<()> {
    tx.prepare_cached("DELETE FROM node_claim WHERE scope = ?1 AND id = ?2")?
        .execute(params![scope, id.as_str()])?;
    Ok(())
}

fn insert_node(tx: &Transaction<'_>, scope: &str, node: &Node, head: u64) -> Result<()> {
    let [start_line, start_column, end_line, end_column] = span_columns(node.span.as_ref());
    tx.prepare_cached(
        "INSERT INTO node (id, scope, kind, name, language, parent, signature, body_hash,
                           start_line, start_column, end_line, end_column, valid_from, role)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
    )?
    .execute(params![
        node.id.as_str(),
        scope,
        node.kind.as_str(),
        node.name,
        node.language.as_ref().map(|l| l.as_str()),
        node.parent.as_ref().map(NodeId::as_str),
        node.signature,
        node.body_hash.as_ref().map(ContentHash::as_bytes),
        start_line,
        start_column,
        end_line,
        end_column,
        seq_param(head),
        node.role.map(Role::as_str),
    ])?;
    Ok(())
}

fn update_node_in_place(tx: &Transaction<'_>, version_id: i64, node: &Node) -> Result<()> {
    let [start_line, start_column, end_line, end_column] = span_columns(node.span.as_ref());
    tx.prepare_cached(
        "UPDATE node SET kind = ?2, name = ?3, language = ?4, parent = ?5, signature = ?6, body_hash = ?7,
                         start_line = ?8, start_column = ?9, end_line = ?10, end_column = ?11, role = ?12
         WHERE version_id = ?1",
    )?
    .execute(params![
        version_id,
        node.kind.as_str(),
        node.name,
        node.language.as_ref().map(|l| l.as_str()),
        node.parent.as_ref().map(NodeId::as_str),
        node.signature,
        node.body_hash.as_ref().map(ContentHash::as_bytes),
        start_line,
        start_column,
        end_line,
        end_column,
        node.role.map(Role::as_str),
    ])?;
    Ok(())
}

fn update_span(tx: &Transaction<'_>, version_id: i64, node: &Node) -> Result<()> {
    let [start_line, start_column, end_line, end_column] = span_columns(node.span.as_ref());
    tx.prepare_cached(
        "UPDATE node SET start_line = ?2, start_column = ?3, end_line = ?4, end_column = ?5 WHERE version_id = ?1",
    )?
    .execute(params![version_id, start_line, start_column, end_line, end_column])?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Observations and merged edges
// ---------------------------------------------------------------------------------------------

/// Replaces what one source observed in a scope, writing only the differences. Returns whether
/// anything changed.
fn replace_observations(
    tx: &Transaction<'_>,
    scope: &str,
    given: &SourceObservations,
) -> Result<bool> {
    let source = given.source.as_str();

    // Duplicates add up, as in fusion.
    let mut wanted: BTreeMap<EdgeKey, (f32, u32)> = BTreeMap::new();
    for observation in &given.observations {
        let entry = wanted
            .entry(observation.key())
            .or_insert((observation.confidence.get(), 0));
        entry.0 = entry.0.max(observation.confidence.get());
        entry.1 = entry.1.saturating_add(observation.weight);
    }
    let current: BTreeMap<EdgeKey, (f32, u32)> = load_observations(tx, scope, Some(given.source))?
        .remove(&given.source)
        .map(|observations| {
            observations
                .observations
                .into_iter()
                .map(|o| (o.key(), (o.confidence.get(), o.weight)))
                .collect()
        })
        .unwrap_or_default();

    let mut changed = false;
    for key in current.keys().filter(|key| !wanted.contains_key(*key)) {
        tx.prepare_cached(
            "DELETE FROM observation WHERE scope = ?1 AND source = ?2 AND src = ?3 AND dst = ?4 AND kind = ?5",
        )?
        .execute(params![scope, source, key.src.as_str(), key.dst.as_str(), key.kind.as_str()])?;
        changed = true;
    }
    for (key, value) in &wanted {
        if current.get(key) == Some(value) {
            continue;
        }
        tx.prepare_cached(
            "INSERT INTO observation (scope, source, src, dst, kind, confidence, weight)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT (scope, source, src, dst, kind)
             DO UPDATE SET confidence = excluded.confidence, weight = excluded.weight",
        )?
        .execute(params![
            scope,
            source,
            key.src.as_str(),
            key.dst.as_str(),
            key.kind.as_str(),
            f64::from(value.0),
            value.1,
        ])?;
        changed = true;
    }

    let wanted_kinds: BTreeSet<EdgeKind> = given.verified.iter().copied().collect();
    let current_kinds = verified_kinds(tx, scope, given.source)?;
    for kind in current_kinds.difference(&wanted_kinds) {
        tx.prepare_cached(
            "DELETE FROM verification WHERE scope = ?1 AND source = ?2 AND kind = ?3",
        )?
        .execute(params![scope, source, kind.as_str()])?;
        changed = true;
    }
    for kind in wanted_kinds.difference(&current_kinds) {
        tx.prepare_cached("INSERT INTO verification (scope, source, kind) VALUES (?1, ?2, ?3)")?
            .execute(params![scope, source, kind.as_str()])?;
        changed = true;
    }
    Ok(changed)
}

/// Observations of a scope by source, with their verified kinds; only `only` when given.
fn load_observations(
    tx: &Transaction<'_>,
    scope: &str,
    only: Option<Provenance>,
) -> Result<BTreeMap<Provenance, SourceObservations>> {
    let mut statement = tx.prepare_cached(
        "SELECT source, src, dst, kind, confidence, weight FROM observation
         WHERE scope = ?1 AND (?2 IS NULL OR source = ?2)",
    )?;
    let only = only.map(Provenance::as_str);
    let rows = statement
        .query_map(params![scope, only], |row| {
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
    let mut by_source: BTreeMap<Provenance, SourceObservations> = BTreeMap::new();
    for (source, src, dst, kind, confidence, weight) in rows {
        let source = Provenance::parse(&source)?;
        #[allow(clippy::cast_possible_truncation)] // Stored from an f32: the round trip is exact.
        let confidence = timonier_core::Confidence::new(confidence as f32)?;
        let observation = Observation::new(
            NodeId::parse(src)?,
            NodeId::parse(dst)?,
            EdgeKind::parse(&kind)?,
        )
        .with_confidence(confidence)
        .with_weight(to_u32("weight", weight)?);
        by_source
            .entry(source)
            .or_insert_with(|| SourceObservations::new(source, Vec::new()))
            .observations
            .push(observation);
    }
    let mut statement = tx.prepare_cached(
        "SELECT source, kind FROM verification WHERE scope = ?1 AND (?2 IS NULL OR source = ?2)",
    )?;
    let rows = statement
        .query_map(params![scope, only], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (source, kind) in rows {
        let source = Provenance::parse(&source)?;
        by_source
            .entry(source)
            .or_insert_with(|| SourceObservations::new(source, Vec::new()))
            .verified
            .push(EdgeKind::parse(&kind)?);
    }
    Ok(by_source)
}

fn verified_kinds(
    tx: &Transaction<'_>,
    scope: &str,
    source: Provenance,
) -> Result<BTreeSet<EdgeKind>> {
    let mut statement =
        tx.prepare_cached("SELECT kind FROM verification WHERE scope = ?1 AND source = ?2")?;
    let kinds = statement
        .query_map(params![scope, source.as_str()], |row| {
            row.get::<_, String>(0)
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    kinds
        .iter()
        .map(|kind| Ok(EdgeKind::parse(kind)?))
        .collect()
}

/// Fuses the scope's observations and brings its merged edges in line, writing only changes.
fn recompute_edges(
    tx: &Transaction<'_>,
    head: u64,
    scope: &str,
    stats: &mut ApplyStats,
) -> Result<()> {
    let sources: Vec<SourceObservations> =
        load_observations(tx, scope, None)?.into_values().collect();
    let fused = fusion::fuse(&sources)?;
    let owned = current_edges(tx, scope)?;
    let waiting = edge_claims(tx, scope)?;
    let wanted: HashSet<EdgeKey> = fused.iter().map(Edge::key).collect();

    let mut gone: Vec<(&EdgeKey, &Stored<Edge>)> = owned
        .iter()
        .filter(|(key, _)| !wanted.contains(key))
        .collect();
    gone.sort_by(|a, b| a.0.cmp(b.0));
    for (key, stored) in gone {
        match next_edge_claim(tx, key)? {
            Some((claimant, edge)) => {
                delete_edge_claim(tx, &claimant, key)?;
                hand_edge_over(tx, head, stored, &claimant, &edge, stats)?;
            }
            None => {
                retire(tx, Table::Edge, stored, head)?;
                stats.edges_removed += 1;
            }
        }
    }
    for key in waiting.keys().filter(|key| !wanted.contains(key)) {
        delete_edge_claim(tx, scope, key)?;
    }

    for edge in &fused {
        let key = edge.key();
        if let Some(stored) = owned.get(&key) {
            match next_edge_claim(tx, &key)? {
                // Our new version is less reliable than one waiting: that one takes over.
                Some((claimant, claim)) if edge_precedes((&claim, &claimant), (edge, scope)) => {
                    delete_edge_claim(tx, &claimant, &key)?;
                    upsert_edge_claim(tx, scope, edge)?;
                    stats.edges_waiting += 1;
                    hand_edge_over(tx, head, stored, &claimant, &claim, stats)?;
                }
                _ => update_owned_edge(tx, head, scope, stored, edge, stats)?,
            }
            continue;
        }
        match edge_anywhere(tx, &key)? {
            None => {
                insert_edge(tx, scope, edge, head)?;
                stats.edges_added += 1;
            }
            Some(owner) if edge_precedes((edge, scope), (&owner.stored.value, &owner.scope)) => {
                upsert_edge_claim(tx, &owner.scope, &owner.stored.value)?;
                stats.edges_waiting += 1;
                if waiting.contains_key(&key) {
                    delete_edge_claim(tx, scope, &key)?;
                }
                hand_edge_over(tx, head, &owner.stored, scope, edge, stats)?;
            }
            Some(_) => {
                if waiting.get(&key) != Some(edge) {
                    upsert_edge_claim(tx, scope, edge)?;
                    stats.edges_waiting += 1;
                }
            }
        }
    }
    Ok(())
}

fn update_owned_edge(
    tx: &Transaction<'_>,
    head: u64,
    scope: &str,
    stored: &Stored<Edge>,
    edge: &Edge,
    stats: &mut ApplyStats,
) -> Result<()> {
    if stored.value.same_version(edge) {
        return Ok(());
    }
    if stored.valid_from == head {
        tx.prepare_cached(
            "UPDATE edge SET provenance = ?2, confidence = ?3, weight = ?4 WHERE version_id = ?1",
        )?
        .execute(params![
            stored.version_id,
            edge.provenance.as_str(),
            f64::from(edge.confidence.get()),
            edge.weight
        ])?;
    } else {
        close(tx, Table::Edge, stored.version_id, head)?;
        insert_edge(tx, scope, edge, head)?;
    }
    stats.edges_changed += 1;
    Ok(())
}

fn hand_edge_over(
    tx: &Transaction<'_>,
    head: u64,
    stored: &Stored<Edge>,
    new_owner: &str,
    edge: &Edge,
    stats: &mut ApplyStats,
) -> Result<()> {
    stats.edges_taken_over += 1;
    if stored.value.same_version(edge) || stored.valid_from == head {
        set_scope(tx, Table::Edge, stored.version_id, new_owner)?;
    }
    update_owned_edge(tx, head, new_owner, stored, edge, stats)
}

/// Which of two versions of an edge, provided by two scopes, is in the graph: the most reliable
/// provenance (spec « Fusion »), then the smallest scope key.
fn edge_precedes(a: (&Edge, &str), b: (&Edge, &str)) -> bool {
    (a.0.provenance.rank(), std::cmp::Reverse(a.1))
        > (b.0.provenance.rank(), std::cmp::Reverse(b.1))
}

fn read_stored_edges(
    tx: &Transaction<'_>,
    sql: &str,
    params: impl rusqlite::Params,
) -> Result<Vec<(String, Stored<Edge>)>> {
    let mut statement = tx.prepare_cached(sql)?;
    let rows = statement
        .query_map(params, |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                RawEdge::read(row, 3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter()
        .map(|(scope, version_id, valid_from, raw)| {
            Ok((
                scope,
                Stored {
                    version_id,
                    valid_from: to_seq(valid_from)?,
                    value: raw.into_edge()?,
                },
            ))
        })
        .collect()
}

fn current_edges(tx: &Transaction<'_>, scope: &str) -> Result<HashMap<EdgeKey, Stored<Edge>>> {
    let sql = format!(
        "SELECT scope, version_id, valid_from, {EDGE_COLUMNS} FROM edge WHERE scope = ?1 AND valid_to IS NULL"
    );
    Ok(read_stored_edges(tx, &sql, [scope])?
        .into_iter()
        .map(|(_, stored)| (stored.value.key(), stored))
        .collect())
}

/// The current version of an edge and its owner, whatever scope that is.
fn edge_anywhere(tx: &Transaction<'_>, key: &EdgeKey) -> Result<Option<Owned<Edge>>> {
    let sql = format!(
        "SELECT scope, version_id, valid_from, {EDGE_COLUMNS} FROM edge
         WHERE src = ?1 AND dst = ?2 AND kind = ?3 AND valid_to IS NULL"
    );
    Ok(read_stored_edges(
        tx,
        &sql,
        params![key.src.as_str(), key.dst.as_str(), key.kind.as_str()],
    )?
    .into_iter()
    .next()
    .map(|(scope, stored)| Owned { scope, stored }))
}

fn edge_claims(tx: &Transaction<'_>, scope: &str) -> Result<HashMap<EdgeKey, Edge>> {
    let mut statement = tx.prepare_cached(&format!(
        "SELECT {EDGE_COLUMNS} FROM edge_claim WHERE scope = ?1"
    ))?;
    let raws = statement
        .query_map([scope], |row| RawEdge::read(row, 0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    raws.into_iter()
        .map(|raw| {
            let edge = raw.into_edge()?;
            Ok((edge.key(), edge))
        })
        .collect()
}

/// The waiting version that takes over next: the most reliable, then the smallest scope key.
fn next_edge_claim(tx: &Transaction<'_>, key: &EdgeKey) -> Result<Option<(String, Edge)>> {
    let mut statement = tx.prepare_cached(&format!(
        "SELECT scope, {EDGE_COLUMNS} FROM edge_claim WHERE src = ?1 AND dst = ?2 AND kind = ?3"
    ))?;
    let rows = statement
        .query_map(
            params![key.src.as_str(), key.dst.as_str(), key.kind.as_str()],
            |row| Ok((row.get::<_, String>(0)?, RawEdge::read(row, 1)?)),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut best: Option<(String, Edge)> = None;
    for (scope, raw) in rows {
        let edge = raw.into_edge()?;
        let better = best.as_ref().is_none_or(|(best_scope, best_edge)| {
            edge_precedes((&edge, &scope), (best_edge, best_scope))
        });
        if better {
            best = Some((scope, edge));
        }
    }
    Ok(best)
}

fn upsert_edge_claim(tx: &Transaction<'_>, scope: &str, edge: &Edge) -> Result<()> {
    tx.prepare_cached(
        "INSERT INTO edge_claim (scope, src, dst, kind, provenance, confidence, weight)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT (src, dst, kind, scope) DO UPDATE SET
             provenance = excluded.provenance, confidence = excluded.confidence, weight = excluded.weight",
    )?
    .execute(params![
        scope,
        edge.src.as_str(),
        edge.dst.as_str(),
        edge.kind.as_str(),
        edge.provenance.as_str(),
        f64::from(edge.confidence.get()),
        edge.weight,
    ])?;
    Ok(())
}

fn delete_edge_claim(tx: &Transaction<'_>, scope: &str, key: &EdgeKey) -> Result<()> {
    tx.prepare_cached(
        "DELETE FROM edge_claim WHERE scope = ?1 AND src = ?2 AND dst = ?3 AND kind = ?4",
    )?
    .execute(params![
        scope,
        key.src.as_str(),
        key.dst.as_str(),
        key.kind.as_str()
    ])?;
    Ok(())
}

fn insert_edge(tx: &Transaction<'_>, scope: &str, edge: &Edge, head: u64) -> Result<()> {
    tx.prepare_cached(
        "INSERT INTO edge (src, dst, kind, scope, provenance, confidence, weight, valid_from)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?
    .execute(params![
        edge.src.as_str(),
        edge.dst.as_str(),
        edge.kind.as_str(),
        scope,
        edge.provenance.as_str(),
        f64::from(edge.confidence.get()),
        edge.weight,
        seq_param(head),
    ])?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Versioning helpers (ADR 0010)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Table {
    Node,
    Edge,
}

impl Table {
    const fn name(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Edge => "edge",
        }
    }
}

/// A row that disappears: deleted if no frozen snapshot saw it, closed otherwise.
fn retire<T>(tx: &Transaction<'_>, table: Table, stored: &Stored<T>, head: u64) -> Result<()> {
    if stored.valid_from == head {
        tx.prepare_cached(&format!(
            "DELETE FROM {} WHERE version_id = ?1",
            table.name()
        ))?
        .execute([stored.version_id])?;
        Ok(())
    } else {
        close(tx, table, stored.version_id, head)
    }
}

/// Moves a row to another scope. The owner scope is not a versioned attribute: no new version.
fn set_scope(tx: &Transaction<'_>, table: Table, version_id: i64, scope: &str) -> Result<()> {
    tx.prepare_cached(&format!(
        "UPDATE {} SET scope = ?2 WHERE version_id = ?1",
        table.name()
    ))?
    .execute(params![version_id, scope])?;
    Ok(())
}

fn close(tx: &Transaction<'_>, table: Table, version_id: i64, head: u64) -> Result<()> {
    tx.prepare_cached(&format!(
        "UPDATE {} SET valid_to = ?2 WHERE version_id = ?1",
        table.name()
    ))?
    .execute(params![version_id, seq_param(head)])?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Tree checks, at the end of a batch
// ---------------------------------------------------------------------------------------------

fn placement(tx: &Transaction<'_>, id: &NodeId) -> Result<Option<(NodeKind, Option<NodeId>)>> {
    let mut statement =
        tx.prepare_cached("SELECT kind, parent FROM node WHERE id = ?1 AND valid_to IS NULL")?;
    let row: Option<(String, Option<String>)> = statement
        .query_row([id.as_str()], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?;
    row.map(|(kind, parent)| {
        Ok((
            NodeKind::parse(&kind)?,
            parent.map(NodeId::parse).transpose()?,
        ))
    })
    .transpose()
}

fn subtree(tx: &Transaction<'_>, root: &NodeId) -> Result<Vec<NodeId>> {
    // UNION, not UNION ALL: it terminates even if a cycle slipped in (the checks then report it).
    let mut statement = tx.prepare_cached(
        "WITH RECURSIVE sub(id) AS (
             SELECT ?1
             UNION
             SELECT node.id FROM node JOIN sub ON node.parent = sub.id WHERE node.valid_to IS NULL
         )
         SELECT id FROM sub",
    )?;
    let ids = statement
        .query_map([root.as_str()], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ids.into_iter().map(|id| Ok(NodeId::parse(id)?)).collect()
}

fn check_tree(tx: &Transaction<'_>, checks: &PendingChecks) -> Result<()> {
    let mut to_check: BTreeSet<NodeId> = checks.placed.iter().cloned().collect();
    for id in &checks.restructured {
        to_check.extend(subtree(tx, id)?);
    }
    for id in &to_check {
        // It may have disappeared later in the batch.
        let Some((kind, parent)) = placement(tx, id)? else {
            continue;
        };
        match tree::check_placement(id, kind, parent.as_ref(), |n| placement(tx, n)) {
            Ok(()) => {}
            Err(CheckError::Violation(violation)) => return Err(violation.into()),
            Err(CheckError::Lookup(error)) => return Err(error),
        }
    }
    for id in &checks.removed {
        if placement(tx, id)?.is_some() {
            continue; // It came back in another scope of the batch.
        }
        let child: Option<String> = tx
            // Waiting contributions count too: one of them may take over later.
            .prepare_cached(
                "SELECT id FROM (
                     SELECT id FROM node WHERE parent = ?1 AND valid_to IS NULL
                     UNION ALL SELECT id FROM node_claim WHERE parent = ?1
                 ) ORDER BY id LIMIT 1",
            )?
            .query_row([id.as_str()], |row| row.get(0))
            .optional()?;
        if let Some(child) = child {
            return Err(TreeViolation::OrphanedChild {
                node: id.clone(),
                child: NodeId::parse(child)?,
            }
            .into());
        }
    }
    if checks.repo_placed {
        let repos: Vec<String> = tx
            .prepare_cached(
                "SELECT id FROM node WHERE kind = 'repo' AND valid_to IS NULL ORDER BY id LIMIT 2",
            )?
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        if let [existing, second] = repos.as_slice() {
            return Err(TreeViolation::SecondRepo {
                node: NodeId::parse(second.clone())?,
                existing: NodeId::parse(existing.clone())?,
            }
            .into());
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Snapshots
// ---------------------------------------------------------------------------------------------

/// Freezes the head with the given kind, then opens a new head (ADR 0010).
/// Forgets the contents kept for snapshots other than review marks: once a session ended, its
/// diff is in its journal (ADR 0031).
pub(crate) fn forget_contents(connection: &mut Connection) -> Result<()> {
    connection.execute(
        "DELETE FROM snapshot_content WHERE seq NOT IN \
         (SELECT seq FROM snapshot WHERE kind = 'review_mark')",
        [],
    )?;
    Ok(())
}

/// With `keep_contents`, the content of each file is kept for the snapshot: the review diff and
/// a session's diff compare them (ADR 0024, 0031). A review mark always keeps them.
pub(crate) fn freeze(
    connection: &mut Connection,
    freeze: &Freeze,
    timestamp_ms: i64,
    keep_contents: bool,
) -> Result<Snapshot> {
    let tx = connection.transaction()?;
    let head = head_seq(&tx)?;
    if matches!(freeze, Freeze::ReviewMark) {
        // Only the last review keeps its contents; a session's stay until it ends (ADR 0031).
        tx.execute(
            "DELETE FROM snapshot_content WHERE seq IN \
             (SELECT seq FROM snapshot WHERE kind = 'review_mark')",
            [],
        )?;
    }
    let commit_sha = match freeze {
        Freeze::Commit { sha } => Some(sha.clone()),
        Freeze::ReviewMark | Freeze::WorkingTree => None,
    };
    tx.execute(
        "UPDATE snapshot SET kind = ?1, commit_sha = ?2, timestamp_ms = ?3, frozen = 1 WHERE seq = ?4",
        params![freeze.kind().as_str(), commit_sha, timestamp_ms, seq_param(head)],
    )?;
    tx.execute(
        "INSERT INTO snapshot (seq, kind, commit_sha, timestamp_ms, frozen) VALUES (?1, 'working_tree', NULL, ?2, 0)",
        params![seq_param(head + 1), timestamp_ms],
    )?;
    if keep_contents || matches!(freeze, Freeze::ReviewMark) {
        tx.execute(
            "INSERT INTO snapshot_content (seq, scope, content_hash) \
             SELECT ?1, key, content_hash FROM scope \
             WHERE key LIKE 'file:%' AND content_hash IS NOT NULL",
            params![seq_param(head)],
        )?;
    }
    tx.commit()?;
    Ok(Snapshot {
        seq: head,
        kind: freeze.kind(),
        commit_sha,
        timestamp_ms,
        frozen: true,
    })
}
