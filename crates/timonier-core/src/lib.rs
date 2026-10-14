//! Timonier's graph model: node and edge types, stable identity and fusion rules.
//!
//! This crate is pure: no I/O and no async runtime. See the `graph-model` skill.

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
