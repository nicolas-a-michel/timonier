//! The Timonier daemon: file watcher, work queue and JSON-RPC 2.0 server.
//!
//! One daemon per project; it wires the other crates together. See the `rust-daemon` skill.

mod error;
mod git;
pub mod indexing;
pub mod watch;

pub use error::{Error, Result};
pub use indexing::{Consolidation, Pipeline, Refinement, Report};
pub use watch::{Event, Watch};

#[cfg(test)]
mod tests {
    #[test]
    fn shares_the_workspace_version() {
        assert_eq!(timonier_core::VERSION, env!("CARGO_PKG_VERSION"));
    }
}
