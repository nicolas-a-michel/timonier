//! Timonier storage: the SQLite schema, migrations, snapshots and snapshot diffs.
//!
//! There is exactly one writer, in WAL mode. See the `graph-model` and `rust-daemon` skills.

#[cfg(test)]
mod tests {
    #[test]
    fn shares_the_workspace_version() {
        assert_eq!(timonier_core::VERSION, env!("CARGO_PKG_VERSION"));
    }
}
