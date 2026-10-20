//! The Timonier daemon: file watcher, work queue and JSON-RPC 2.0 server.
//!
//! One daemon per project; it wires the other crates together. See the `rust-daemon` skill.

pub mod bridge;
pub mod client;
pub mod debt;
mod error;
mod git;
pub mod indexing;
pub mod journal;
mod relay;
pub mod runtime;
pub mod server;
mod subscriptions;
pub mod tracking;
pub mod view;
pub mod watch;

pub use client::{Client, ClientError};
pub use error::{Error, Result};
pub use indexing::{Consolidated, Consolidation, Pipeline, Refinement, Report};
pub use runtime::{Lock, Paths};
pub use server::{Daemon, Options};
pub use watch::{Event, PendingReview, Watch};

#[cfg(test)]
mod tests {
    #[test]
    fn shares_the_workspace_version() {
        assert_eq!(timonier_core::VERSION, env!("CARGO_PKG_VERSION"));
    }
}
