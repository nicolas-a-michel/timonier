//! Timonier indexing: tree-sitter extraction, language packs, LSP access, components and roles.
//!
//! See the `language-pack` and `graph-model` skills.

#[cfg(test)]
mod tests {
    #[test]
    fn shares_the_workspace_version() {
        assert_eq!(timonier_core::VERSION, env!("CARGO_PKG_VERSION"));
    }
}
