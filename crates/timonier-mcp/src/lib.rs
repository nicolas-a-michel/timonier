//! The Timonier MCP server: a thin proxy to the daemon, with no business logic and no storage access.
//!
//! See the `mcp-tools` skill.

#[cfg(test)]
mod tests {
    #[test]
    fn shares_the_workspace_version() {
        assert_eq!(timonier_core::VERSION, env!("CARGO_PKG_VERSION"));
    }
}
