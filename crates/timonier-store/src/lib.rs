//! Timonier storage: the SQLite schema, migrations, snapshots and snapshot diffs.
//!
//! One writer thread owns the only read-write connection, in WAL mode; readers use their own
//! read-only connections (ADR 0011). Every node and edge row carries a `valid_from`/`valid_to`
//! interval of snapshots; the diff between two snapshots is a query (ADR 0010). See the
//! `graph-model` and `rust-daemon` skills.

mod codec;
mod error;
mod extraction;
mod read;
mod schema;
mod write;
mod writer;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use timonier_core::{Freeze, ScopeUpdate, Snapshot};

pub use error::{Error, Result};
pub use extraction::CachedExtraction;
pub use read::Reader;
pub use write::ApplyStats;

use writer::Writer;

/// The graph store of one project. Cheap to clone; every clone shares the same writer thread.
#[derive(Clone)]
pub struct Store {
    inner: Arc<Inner>,
}

struct Inner {
    path: PathBuf,
    writer: Writer,
}

impl Store {
    /// Opens (or creates) the database at `path` and brings its schema up to date.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let opened = schema::open_writer(&path).and_then(Writer::spawn);
        let writer = opened.map_err(|source| Error::Open {
            path: path.clone(),
            source: Box::new(source),
        })?;
        Ok(Self {
            inner: Arc::new(Inner { path, writer }),
        })
    }

    pub fn path(&self) -> &Path {
        &self.inner.path
    }

    /// A new read-only view; open one per thread.
    pub fn reader(&self) -> Result<Reader> {
        Reader::open(&self.inner.path)
    }

    /// Applies one scope update in its own transaction.
    pub fn apply(&self, update: ScopeUpdate) -> Result<ApplyStats> {
        self.apply_all(vec![update])
    }

    /// Applies several scope updates in one transaction: all or nothing. The tree is checked once
    /// at the end, so the updates may come in any order. When several scopes provide the same
    /// node or edge, one owns it by a fixed rule and the others wait for their turn (ADR 0009):
    /// the result never depends on the order of the updates.
    pub fn apply_all(&self, updates: Vec<ScopeUpdate>) -> Result<ApplyStats> {
        self.inner
            .writer
            .run(move |connection| write::apply(connection, &updates))
    }

    /// Freezes the head as a commit, a review mark or a working-tree snapshot, and opens a new
    /// head. Returns the frozen snapshot.
    pub fn freeze(&self, freeze: Freeze) -> Result<Snapshot> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| {
                i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
            });
        self.inner
            .writer
            .run(move |connection| write::freeze(connection, &freeze, now))
    }

    /// Saves extractions of the indexer and forgets the paths of `remove` (ADR 0013), in one
    /// transaction.
    pub fn save_extractions(&self, put: Vec<CachedExtraction>, remove: Vec<String>) -> Result<()> {
        self.inner
            .writer
            .run(move |connection| extraction::save(connection, &put, &remove))
    }

    /// Rows written since the store was opened (SQLite's `total_changes`): lets tests prove that
    /// re-applying an identical update writes nothing.
    pub fn total_changes(&self) -> Result<u64> {
        self.inner
            .writer
            .run(|connection| Ok(connection.total_changes()))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn shares_the_workspace_version() {
        assert_eq!(timonier_core::VERSION, env!("CARGO_PKG_VERSION"));
    }
}
