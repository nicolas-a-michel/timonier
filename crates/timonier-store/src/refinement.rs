//! What the LSP refinement still owes when the daemon stops (ADR 0022, migration 0004): the
//! verified files a changed signature made stale, and the lookups of dependents not answered
//! yet. Without it, a restart would forget them, and stale verifications would pass for sure.

use rusqlite::{Connection, Transaction, params};
use timonier_core::{ContentHash, Position};

use crate::{Error, Result};

/// A lookup of the dependents of a symbol, as the daemon left it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingLookup {
    pub path: String,
    /// The symbol's id.
    pub symbol: String,
    pub at: Position,
    pub content_hash: ContentHash,
    /// The files citing its name.
    pub by_name: Vec<String>,
}

/// What the refinement owes, sorted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RefinementDebt {
    pub dependents: Vec<String>,
    pub lookups: Vec<PendingLookup>,
}

/// What changed in what is owed: the rows to write, not the whole debt (review n° 10, m10-2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DebtChange {
    pub dependents_added: Vec<String>,
    pub dependents_removed: Vec<String>,
    /// New lookups, or changed ones, written again whole.
    pub lookups_put: Vec<PendingLookup>,
    /// (path, symbol) of the lookups gone.
    pub lookups_removed: Vec<(String, String)>,
}

impl DebtChange {
    pub fn is_empty(&self) -> bool {
        self.dependents_added.is_empty()
            && self.dependents_removed.is_empty()
            && self.lookups_put.is_empty()
            && self.lookups_removed.is_empty()
    }
}

impl RefinementDebt {
    /// What to write to go from `before` to this debt.
    pub fn changes_from(&self, before: &RefinementDebt) -> DebtChange {
        use std::collections::{BTreeMap, BTreeSet};
        let old: BTreeSet<&String> = before.dependents.iter().collect();
        let new: BTreeSet<&String> = self.dependents.iter().collect();
        let key = |l: &PendingLookup| (l.path.clone(), l.symbol.clone());
        let old_lookups: BTreeMap<(String, String), &PendingLookup> =
            before.lookups.iter().map(|l| (key(l), l)).collect();
        let new_lookups: BTreeMap<(String, String), &PendingLookup> =
            self.lookups.iter().map(|l| (key(l), l)).collect();
        DebtChange {
            dependents_added: new.difference(&old).map(|p| (*p).clone()).collect(),
            dependents_removed: old.difference(&new).map(|p| (*p).clone()).collect(),
            lookups_put: new_lookups
                .iter()
                .filter(|(k, l)| old_lookups.get(*k) != Some(*l))
                .map(|(_, l)| (*l).clone())
                .collect(),
            lookups_removed: old_lookups
                .keys()
                .filter(|k| !new_lookups.contains_key(*k))
                .cloned()
                .collect(),
        }
    }
}

/// Writes what changed in what is owed, in one transaction.
pub(crate) fn update(connection: &mut Connection, change: &DebtChange) -> Result<()> {
    let tx = connection.transaction()?;
    write_change(&tx, change)?;
    tx.commit()?;
    Ok(())
}

/// Writes what changed in what is owed, in the transaction of the caller.
pub(crate) fn write_change(tx: &Transaction<'_>, change: &DebtChange) -> Result<()> {
    {
        let mut remove = tx.prepare_cached("DELETE FROM refinement_dependent WHERE path = ?1")?;
        for path in &change.dependents_removed {
            remove.execute([path])?;
        }
        let mut add =
            tx.prepare_cached("INSERT OR IGNORE INTO refinement_dependent (path) VALUES (?1)")?;
        for path in &change.dependents_added {
            add.execute([path])?;
        }
        // Its citers go with it (`ON DELETE CASCADE`).
        let mut remove =
            tx.prepare_cached("DELETE FROM refinement_lookup WHERE path = ?1 AND symbol = ?2")?;
        for (path, symbol) in &change.lookups_removed {
            remove.execute(params![path, symbol])?;
        }
        let mut lookup = tx.prepare_cached(
            "INSERT INTO refinement_lookup (path, symbol, line, col, content_hash)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        let mut citer = tx.prepare_cached(
            "INSERT OR IGNORE INTO refinement_lookup_citer (path, symbol, citer)
             VALUES (?1, ?2, ?3)",
        )?;
        for pending in &change.lookups_put {
            remove.execute(params![pending.path, pending.symbol])?;
            lookup.execute(params![
                pending.path,
                pending.symbol,
                pending.at.line,
                pending.at.column,
                pending.content_hash.as_bytes(),
            ])?;
            for name in &pending.by_name {
                citer.execute(params![pending.path, pending.symbol, name])?;
            }
        }
    }
    Ok(())
}

pub(crate) fn load(connection: &Connection) -> Result<RefinementDebt> {
    let mut statement =
        connection.prepare_cached("SELECT path FROM refinement_dependent ORDER BY path")?;
    let dependents = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut statement = connection.prepare_cached(
        "SELECT path, symbol, line, col, content_hash FROM refinement_lookup ORDER BY path, symbol",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, u32>(2)?,
                row.get::<_, u32>(3)?,
                row.get::<_, Vec<u8>>(4)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut citers = connection.prepare_cached(
        "SELECT citer FROM refinement_lookup_citer WHERE path = ?1 AND symbol = ?2 ORDER BY citer",
    )?;
    let mut lookups = Vec::with_capacity(rows.len());
    for (path, symbol, line, column, hash) in rows {
        let content_hash = <[u8; 32]>::try_from(hash.as_slice())
            .map(ContentHash::from_bytes)
            .map_err(|_| Error::Corrupt {
                column: "refinement_lookup.content_hash",
                value: format!("{} bytes", hash.len()),
                expected: "32 bytes",
            })?;
        let by_name = citers
            .query_map(params![path, symbol], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        lookups.push(PendingLookup {
            path,
            symbol,
            at: Position::new(line, column),
            content_hash,
            by_name,
        });
    }
    Ok(RefinementDebt {
        dependents,
        lookups,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Store;

    fn lookup(symbol: &str, by_name: &[&str]) -> PendingLookup {
        PendingLookup {
            path: "src/util.rs".into(),
            symbol: format!("rust:demo::util::{symbol}"),
            at: Position::new(3, 7),
            content_hash: ContentHash::from_bytes([9; 32]),
            by_name: by_name.iter().map(|p| (*p).to_owned()).collect(),
        }
    }

    /// Saved, read back as it was; changed, only the change is written.
    #[test]
    fn what_is_owed_is_kept_and_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("graph.sqlite")).unwrap();
        assert_eq!(
            store.reader().unwrap().refinement_debt().unwrap(),
            RefinementDebt::default()
        );
        let debt = RefinementDebt {
            dependents: vec!["src/a.rs".into(), "src/b.rs".into()],
            lookups: vec![
                lookup("helper", &["src/a.rs", "src/c.rs"]),
                lookup("other", &[]),
            ],
        };
        store
            .update_refinement_debt(debt.changes_from(&RefinementDebt::default()))
            .unwrap();
        assert_eq!(store.reader().unwrap().refinement_debt().unwrap(), debt);
        let less = RefinementDebt {
            dependents: vec!["src/b.rs".into()],
            lookups: vec![lookup("other", &["src/d.rs"])],
        };
        let change = less.changes_from(&debt);
        // Only what changed is written: a dependent gone, a lookup gone, a lookup changed.
        assert_eq!(change.dependents_added, Vec::<String>::new());
        assert_eq!(change.dependents_removed, ["src/a.rs"]);
        assert_eq!(change.lookups_put.len(), 1);
        assert_eq!(
            change.lookups_removed,
            [(
                "src/util.rs".to_owned(),
                "rust:demo::util::helper".to_owned()
            )]
        );
        store.update_refinement_debt(change).unwrap();
        assert_eq!(store.reader().unwrap().refinement_debt().unwrap(), less);
        assert!(less.changes_from(&less).is_empty());
        // A lookup that only moved: nothing else changes, and it is written (review n° 11, S6).
        let mut moved = less.clone();
        moved.lookups[0].at = Position::new(8, 7);
        let change = moved.changes_from(&less);
        assert!(!change.is_empty());
        store.update_refinement_debt(change).unwrap();
        assert_eq!(store.reader().unwrap().refinement_debt().unwrap(), moved);
    }
}
