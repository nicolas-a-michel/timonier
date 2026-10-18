/// Errors of the daemon.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Store(#[from] timonier_store::Error),

    #[error(transparent)]
    Index(#[from] timonier_index::Error),

    #[error("cannot watch `{path}`: {source}")]
    Watch {
        path: std::path::PathBuf,
        #[source]
        source: notify_debouncer_full::notify::Error,
    },

    #[error("cannot start the indexing thread: {0}")]
    Thread(#[source] std::io::Error),

    #[error("the file watcher's lock is poisoned: a thread panicked while holding it")]
    Poisoned,

    #[error("`{path}`: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(
        "the runtime directory `{path}` must belong to this user and be closed to others (mode 0700)"
    )]
    RuntimeDir { path: std::path::PathBuf },

    #[error("cannot start the async runtime: {0}")]
    Runtime(#[source] std::io::Error),

    #[error("a daemon task failed: {0}")]
    Join(String),

    #[error("the cached extraction of `{path}` cannot be encoded: {source}")]
    Cache {
        path: String,
        #[source]
        source: serde_json::Error,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
