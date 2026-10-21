use crate::tree::TreeViolation;
use crate::{NodeId, NodeKind, Provenance};

/// Errors of the graph model.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("invalid node id {id:?}: {reason}")]
    InvalidId { id: String, reason: &'static str },

    #[error("invalid language {language:?}: {reason}")]
    InvalidLanguage {
        language: String,
        reason: &'static str,
    },

    #[error("invalid scope key {key:?}: {reason}")]
    InvalidScope { key: String, reason: &'static str },

    #[error("unknown {what} {value:?}")]
    UnknownName { what: &'static str, value: String },

    #[error("a confidence is a number between 0 and 1, got {0}")]
    InvalidConfidence(f32),

    #[error("invalid content hash: {0}")]
    InvalidHash(&'static str),

    #[error("a span cannot end before it starts")]
    InvalidSpan,

    #[error("the intended architecture (provenance `intended`) never enters the actual graph")]
    IntendedInActualGraph,

    #[error("{id} cannot be a {kind}: its prefix designates another kind of node (ADR 0008)")]
    KindMismatch { id: NodeId, kind: NodeKind },

    #[error(
        "{provenance} reports a call between {src} and {dst}: function → function calls come only from \
         the LSP or SCIP; other sources report calls between files (spec « Technical decisions »)"
    )]
    ApproximateSymbolCall {
        src: NodeId,
        dst: NodeId,
        provenance: Provenance,
    },

    #[error(
        "{id} is a computed box: edges between components are aggregated, never stored (ADR 0009)"
    )]
    ComputedEndpoint { id: NodeId },

    #[error(transparent)]
    Tree(#[from] TreeViolation),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
