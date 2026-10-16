//! Timonier indexing: tree-sitter extraction, language packs, LSP access, components and roles.
//!
//! See the `language-pack` and `graph-model` skills, and ADR 0012 and 0013.

pub mod cluster;
pub mod components;
pub mod ecosystem;
mod error;
pub mod extract;
mod grammar;
pub mod indexer;
pub mod lsp;
pub mod pack;
pub mod project;
mod structure;
mod tokens;

pub use error::{Error, Result};
pub use extract::{
    EXTRACTOR_VERSION, FileFacts, ImportFact, ReferenceFact, SignalFact, SymbolFact,
};
pub use indexer::{Batch, BatchStats, CacheKey, Cached, Indexer};
pub use pack::{Pack, Packs};

impl Packs {
    /// Extracts the facts of one file with a fresh parser.
    pub fn extract(&self, path: &str, source: &[u8]) -> Result<FileFacts> {
        let (pack, grammar) = self
            .for_path(path)
            .ok_or_else(|| Error::NoPack { path: path.into() })?;
        let mut parser = tree_sitter::Parser::new();
        extract::extract(pack, grammar, &mut parser, source, None, path).map(|(facts, _)| facts)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn shares_the_workspace_version() {
        assert_eq!(timonier_core::VERSION, env!("CARGO_PKG_VERSION"));
    }
}
