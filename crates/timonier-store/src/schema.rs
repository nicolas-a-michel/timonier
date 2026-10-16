//! Opening the database: pragmas and migrations (ADR 0011).

use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};
use rusqlite_migration::{M, Migrations};

use crate::{Error, Result};

/// Forward-only migrations; never edit one that has been committed (skill `graph-model`).
const MIGRATION_SLICE: &[M<'_>] = &[M::up(include_str!("../migrations/0001_initial.sql"))];

pub(crate) const MIGRATIONS: Migrations<'_> = Migrations::from_slice(MIGRATION_SLICE);

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Opens the only read-write connection, in WAL mode, and brings the schema up to date.
pub(crate) fn open_writer(path: &Path) -> Result<Connection> {
    let mut connection = Connection::open(path)?;
    connection.busy_timeout(BUSY_TIMEOUT)?;
    let mode: String =
        connection.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(Error::NotWal(mode));
    }
    // Safe with WAL; the database is a rebuildable cache (ADR 0011).
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    MIGRATIONS.to_latest(&mut connection)?;
    Ok(connection)
}

/// Opens a read-only connection; WAL gives it a consistent view while the writer works.
pub(crate) fn open_reader(path: &Path) -> Result<Connection> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_URI,
    )?;
    connection.busy_timeout(BUSY_TIMEOUT)?;
    Ok(connection)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_valid() {
        MIGRATIONS.validate().unwrap();
    }

    #[test]
    fn a_new_database_is_in_wal_mode_with_one_open_head() {
        let dir = tempfile::tempdir().unwrap();
        let connection = open_writer(&dir.path().join("graph.sqlite")).unwrap();
        let mode: String = connection
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
        let heads: i64 = connection
            .query_row("SELECT count(*) FROM snapshot WHERE frozen = 0", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(heads, 1);
    }

    #[test]
    fn opening_twice_keeps_the_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("graph.sqlite");
        drop(open_writer(&path).unwrap());
        let connection = open_writer(&path).unwrap();
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn readers_cannot_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("graph.sqlite");
        let _writer = open_writer(&path).unwrap();
        let reader = open_reader(&path).unwrap();
        assert!(reader.execute("DELETE FROM snapshot", []).is_err());
    }
}
