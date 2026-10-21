//! The Timonier daemon: file watcher, work queue and JSON-RPC 2.0 server.
//!
//! One daemon per project; it wires the other crates together. See the `rust-daemon` skill.

pub mod agents;
mod attach;
pub mod bridge;
pub mod client;
pub mod debt;
mod error;
pub mod forge;
mod git;
pub mod indexing;
pub mod inventory;
pub mod journal;
pub mod osv;
mod process;
mod relay;
pub mod runs;
pub mod runtime;
pub mod sbom;
pub mod server;
pub mod sketch;
mod subscriptions;
pub mod tracking;
pub mod view;
pub mod watch;

pub use client::{Client, ClientError};
pub use error::{Error, Result};
pub use indexing::{Consolidated, Consolidation, Pipeline, Refinement, Report};
pub use runtime::{Lock, Paths};
pub use server::{Daemon, Options};
/// The language packs, for the CLI's `init` (ADR 0034): the CLI depends on the daemon, not on
/// the indexer (ADR 0003).
pub use timonier_index::pack::{
    ConfigBlock, InstrumentationRules, RunnerRule, SdkDependency, SmokeRule, TemplateFile,
};
pub use timonier_index::{Pack, Packs};
/// The header a run's senders carry its token in (audit of 2026-10-21, finding 2).
pub use timonier_otlp::TOKEN_HEADER as OTLP_TOKEN_HEADER;
pub use watch::{Event, PendingReview, Watch};

#[cfg(test)]
mod tests {
    #[test]
    fn shares_the_workspace_version() {
        assert_eq!(timonier_core::VERSION, env!("CARGO_PKG_VERSION"));
    }
}
