//! Opening the database: pragmas and migrations (ADR 0011).

use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};
use rusqlite_migration::{M, Migrations};

use crate::{Error, Result};

/// Forward-only migrations; never edit one that has been committed (skill `graph-model`).
const MIGRATION_SLICE: &[M<'_>] = &[
    M::up(include_str!("../migrations/0001_initial.sql")),
    M::up(include_str!("../migrations/0002_extraction_cache.sql")),
    M::up(include_str!("../migrations/0003_node_role.sql")),
    M::up(include_str!("../migrations/0004_refinement_debt.sql")),
    M::up(include_str!("../migrations/0005_snapshot_content.sql")),
];

pub(crate) const MIGRATIONS: Migrations<'_> = Migrations::from_slice(MIGRATION_SLICE);

/// The measurements' own migrations (ADR 0043), forward-only too.
const PERF_MIGRATION_SLICE: &[M<'_>] = &[
    M::up(include_str!("../migrations/perf/0001_runs.sql")),
    M::up(include_str!(
        "../migrations/perf/0002_worktree_and_aggregates.sql"
    )),
    M::up(include_str!("../migrations/perf/0003_smoke.sql")),
    M::up(include_str!("../migrations/perf/0004_wall_time.sql")),
];

pub(crate) const PERF_MIGRATIONS: Migrations<'_> = Migrations::from_slice(PERF_MIGRATION_SLICE);

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Opens the only read-write connection, in WAL mode, and brings the schema up to date.
pub(crate) fn open_writer(path: &Path) -> Result<Connection> {
    open_writer_with(path, &MIGRATIONS)
}

/// [`open_writer`], for a database with other migrations.
pub(crate) fn open_writer_with(path: &Path, migrations: &Migrations<'_>) -> Result<Connection> {
    let mut connection = Connection::open(path)?;
    connection.busy_timeout(BUSY_TIMEOUT)?;
    let mode: String =
        connection.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(Error::NotWal(mode));
    }
    // Safe with WAL; the database is a rebuildable cache (ADR 0011).
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    // Off while migrating: a table rebuilt to change a constraint is dropped, and its rows
    // must not take the rows that reference it along (SQLite, « ALTER TABLE », other changes).
    // Checked once done, then on for good.
    connection.pragma_update(None, "foreign_keys", "OFF")?;
    migrations.to_latest(&mut connection)?;
    let broken: i64 =
        connection.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })?;
    if broken > 0 {
        return Err(Error::MigrationBroke(format!(
            "{broken} rows reference rows the migration lost"
        )));
    }
    connection.pragma_update(None, "foreign_keys", "ON")?;
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
        PERF_MIGRATIONS.validate().unwrap();
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
        assert_eq!(version, 5);
    }

    #[test]
    fn a_database_of_the_first_schema_migrates_and_keeps_its_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("graph.sqlite");
        {
            let mut connection = Connection::open(&path).unwrap();
            Migrations::from_slice(&MIGRATION_SLICE[..1])
                .to_latest(&mut connection)
                .unwrap();
            connection
                .execute(
                    "INSERT INTO scope (key, content_hash) VALUES ('structure', NULL)",
                    [],
                )
                .unwrap();
        }
        let connection = open_writer(&path).unwrap();
        let scopes: i64 = connection
            .query_row("SELECT count(*) FROM scope", [], |r| r.get(0))
            .unwrap();
        assert_eq!(scopes, 1);
        let cached: i64 = connection
            .query_row("SELECT count(*) FROM extraction", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cached, 0);
    }

    /// T-125: the runs' table rebuilt for the wall time keeps every run, and the rows that
    /// reference them (a drop with the foreign keys on would have taken them along).
    #[test]
    fn the_runs_rebuilt_for_the_wall_time_keep_their_tests_and_aggregates() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("perf.sqlite");
        {
            let mut connection = Connection::open(&path).unwrap();
            connection
                .pragma_update(None, "foreign_keys", "ON")
                .unwrap();
            Migrations::from_slice(&PERF_MIGRATION_SLICE[..3])
                .to_latest(&mut connection)
                .unwrap();
            connection
                .execute_batch(
                    "INSERT INTO run (id, started_at_ms, command, snapshot, cpu_source, worktree)
                       VALUES (1, 10, '[\"true\"]', 3, 'cpu_time', 'w');
                     INSERT INTO test (run, ord, name, status, duration_us)
                       VALUES (1, 0, 'parses', 'passed', 12);
                     INSERT INTO aggregate (run, node, calls, self_ns, entered_ns)
                       VALUES (1, 'rust:demo::run', 2, 5, 7);",
                )
                .unwrap();
        }
        let connection = open_writer_with(&path, &PERF_MIGRATIONS).unwrap();
        let count = |table: &str| -> i64 {
            connection
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
                .unwrap()
        };
        assert_eq!((count("run"), count("test"), count("aggregate")), (1, 1, 1));
        let worktree: String = connection
            .query_row("SELECT worktree FROM run WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(worktree, "w");
        connection
            .execute(
                "INSERT INTO run (id, started_at_ms, command, snapshot, cpu_source, accounting)
                   VALUES (2, 20, '[]', 4, 'wall_time', 'wall_time')",
                [],
            )
            .unwrap();
        // The foreign keys are on again: a test of no run is refused.
        assert!(
            connection
                .execute(
                    "INSERT INTO test (run, ord, name, status, duration_us)
                       VALUES (9, 0, 'x', 'passed', 1)",
                    [],
                )
                .is_err()
        );
    }

    /// Review n° 9, I9-2: a database of the previous schema keeps its rows, and owes nothing.
    #[test]
    fn a_database_of_the_third_schema_migrates_and_owes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("graph.sqlite");
        {
            let mut connection = Connection::open(&path).unwrap();
            Migrations::from_slice(&MIGRATION_SLICE[..3])
                .to_latest(&mut connection)
                .unwrap();
            connection
                .execute(
                    "INSERT INTO scope (key, content_hash) VALUES ('structure', NULL)",
                    [],
                )
                .unwrap();
        }
        let connection = open_writer(&path).unwrap();
        let scopes: i64 = connection
            .query_row("SELECT count(*) FROM scope", [], |r| r.get(0))
            .unwrap();
        assert_eq!(scopes, 1);
        assert_eq!(
            crate::refinement::load(&connection).unwrap(),
            crate::RefinementDebt::default()
        );
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
