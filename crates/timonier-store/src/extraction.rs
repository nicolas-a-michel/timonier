//! The extraction cache (ADR 0013, migration 0002). The store does not read the facts: they are
//! the indexer's JSON, valid for the key they were computed with.

use rusqlite::{Connection, params};
use timonier_core::ContentHash;

use crate::{Error, Result};

/// One cached extraction: the facts of a file and what they were computed from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedExtraction {
    pub path: String,
    pub language: String,
    pub pack_version: String,
    pub extractor: u32,
    pub content_hash: ContentHash,
    pub facts: String,
}

/// Replaces the entries of `put` and drops the paths of `remove`, in one transaction.
pub(crate) fn save(
    connection: &mut Connection,
    put: &[CachedExtraction],
    remove: &[String],
) -> Result<()> {
    let tx = connection.transaction()?;
    {
        let mut insert = tx.prepare_cached(
            "INSERT INTO extraction (path, language, pack_version, extractor, content_hash, facts)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (path) DO UPDATE SET
                 language = excluded.language, pack_version = excluded.pack_version,
                 extractor = excluded.extractor, content_hash = excluded.content_hash,
                 facts = excluded.facts",
        )?;
        for entry in put {
            insert.execute(params![
                entry.path,
                entry.language,
                entry.pack_version,
                entry.extractor,
                entry.content_hash.as_bytes(),
                entry.facts,
            ])?;
        }
        let mut delete = tx.prepare_cached("DELETE FROM extraction WHERE path = ?1")?;
        for path in remove {
            delete.execute([path])?;
        }
    }
    tx.commit()?;
    Ok(())
}

pub(crate) fn load(connection: &Connection) -> Result<Vec<CachedExtraction>> {
    let mut statement = connection.prepare_cached(
        "SELECT path, language, pack_version, extractor, content_hash, facts FROM extraction ORDER BY path",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, Vec<u8>>(4)?,
            row.get::<_, String>(5)?,
        ))
    })?;
    rows.map(|row| {
        let (path, language, pack_version, extractor, hash, facts) = row?;
        let content_hash = <[u8; 32]>::try_from(hash.as_slice())
            .map(ContentHash::from_bytes)
            .map_err(|_| Error::Corrupt {
                column: "extraction.content_hash",
                value: format!("{} bytes", hash.len()),
                expected: "32 bytes",
            })?;
        let extractor = u32::try_from(extractor).map_err(|_| Error::Corrupt {
            column: "extraction.extractor",
            value: extractor.to_string(),
            expected: "a 32-bit version",
        })?;
        Ok(CachedExtraction {
            path,
            language,
            pack_version,
            extractor,
            content_hash,
            facts,
        })
    })
    .collect()
}
