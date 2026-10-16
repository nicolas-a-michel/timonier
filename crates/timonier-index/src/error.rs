use std::path::PathBuf;

/// Errors of the indexing crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("language pack `{pack}`: invalid manifest: {source}")]
    Manifest {
        pack: String,
        #[source]
        source: Box<toml::de::Error>,
    },

    #[error("language pack `{pack}`: {reason}")]
    Pack { pack: String, reason: String },

    #[error("language pack `{pack}`, grammar `{grammar}`, query `{query}`: {message}")]
    Query {
        pack: String,
        grammar: String,
        query: &'static str,
        message: String,
    },

    #[error("cannot read `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("manifest `{path}`: {reason}")]
    PackageManifest { path: PathBuf, reason: String },

    #[error("`{path}`: no language pack reads this file")]
    NoPack { path: String },

    #[error("`{path}` could not be parsed with the `{grammar}` grammar")]
    Parse { path: String, grammar: String },

    #[error(transparent)]
    Model(#[from] timonier_core::Error),
}

impl Error {
    /// The file the error is about, when there is one.
    pub fn path(&self) -> Option<&std::path::Path> {
        match self {
            Self::Io { path, .. } | Self::PackageManifest { path, .. } => Some(path),
            _ => None,
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
