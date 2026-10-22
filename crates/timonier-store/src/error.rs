use std::path::PathBuf;

use timonier_core::{NodeId, Provenance, ScopeKey, TreeViolation};

/// Errors of the graph store.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot open the graph store at {path}")]
    Open {
        path: PathBuf,
        #[source]
        source: Box<Error>,
    },

    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),

    #[error(transparent)]
    Migration(#[from] rusqlite_migration::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Model(#[from] timonier_core::Error),

    #[error(transparent)]
    Tree(#[from] TreeViolation),

    #[error("{id} appears twice in the update of scope {scope}")]
    DuplicateNode { id: NodeId, scope: ScopeKey },

    #[error(
        "{provenance} observations for {scope} do not say which content they were computed on; \
         a precise source must give it (ADR 0009)"
    )]
    UnanchoredObservations {
        scope: ScopeKey,
        provenance: Provenance,
    },

    #[error(
        "{provenance} cannot set the content hash of {scope}: the content comes from the pass that \
         reads it; precise sources say which content they analysed with `computed_on` (ADR 0009)"
    )]
    PreciseSourceSetsContent {
        scope: ScopeKey,
        provenance: Provenance,
    },

    #[error("the removal of scope {scope} cannot also bring a content hash, nodes or observations")]
    RemovalWithContent { scope: ScopeKey },

    #[error("{id} does not belong in scope {scope}: {reason} (ADR 0009)")]
    OutOfScope {
        scope: ScopeKey,
        id: NodeId,
        reason: &'static str,
    },

    #[error("scope {scope} observes an edge leaving {src}, which it does not provide (ADR 0009)")]
    ForeignSource { scope: ScopeKey, src: NodeId },

    #[error("run {0} does not exist")]
    UnknownRun(u64),

    #[error("run {0} is already finished")]
    RunFinished(u64),

    #[error("snapshot {0} does not exist")]
    UnknownSnapshot(u64),

    #[error("a diff goes forward in time, from {from} to {to}")]
    BackwardDiff { from: u64, to: u64 },

    #[error("the database holds {0} open snapshots instead of exactly one")]
    HeadCount(u64),

    #[error("the database refused WAL mode (journal mode is {0})")]
    NotWal(String),

    /// After the migrations, rows reference rows that are gone.
    #[error("the migrations broke the database: {0}")]
    MigrationBroke(String),

    #[error("the writer thread has stopped")]
    WriterStopped,

    #[error("a write panicked and was rolled back: {0}")]
    WritePanicked(String),

    #[error("column {column} holds {value}, which is not {expected}")]
    Corrupt {
        column: &'static str,
        value: String,
        expected: &'static str,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
