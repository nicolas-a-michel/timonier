//! Timonier's graph model: node and edge types, stable identity, the strict tree, fusion of
//! sources, snapshot diffs with move detection, aggregation at a zoom level.
//!
//! This crate is pure: no I/O and no async runtime. See the `graph-model` skill and
//! ADR 0008 to 0010.

pub mod aggregate;
pub mod diff;
mod edge;
mod error;
pub mod fusion;
mod id;
mod node;
mod snapshot;
pub mod tree;
mod update;

pub use aggregate::AggregatedEdge;
pub use diff::{EdgeChange, GraphDiff, MoveEvidence, NodeChange};
pub use edge::{Confidence, Edge, EdgeKey, EdgeKind, Observation, Provenance};
pub use error::{Error, Result};
pub use fusion::SourceObservations;
pub use id::{IdForm, Language, NodeId, RESERVED_PREFIXES};
pub use node::{ContentHash, Node, NodeKind, Position, Role, Span, SymbolKind};
pub use snapshot::{Freeze, Snapshot, SnapshotKind};
pub use tree::TreeViolation;
pub use update::{ScopeKey, ScopeUpdate};

/// Version shared by every Timonier crate and binary (the workspace version).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn version_is_major_minor_patch() {
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "{VERSION}");
        assert!(
            parts.iter().all(|part| part.parse::<u64>().is_ok()),
            "{VERSION}"
        );
    }
}
