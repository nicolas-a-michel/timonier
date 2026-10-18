//! Criterion of PLAN.md 3.9: on Timonier itself, the expected roles (ADR 0017). The repository
//! is only read: the database lives in a temporary directory.
//!
//! With its JSON-RPC server (Phase 4), the daemon is an entry point. `timonier-mcp` has no MCP
//! SDK before Phase 6: a utility until then.

use std::collections::BTreeMap;
use std::path::Path;

use timonier_core::{NodeKind, Role};
use timonier_daemon::Pipeline;
use timonier_store::Store;

#[test]
fn timonier_s_packages_get_the_expected_roles() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let db = tempfile::tempdir().unwrap();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(&root, store.clone()).unwrap();
    pipeline.index_all().unwrap();
    let mut reader = store.reader().unwrap();
    let head = reader.head().unwrap().seq;
    let nodes = reader.nodes_at(head).unwrap();
    let roles: BTreeMap<&str, Role> = nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Package && n.id.as_str().contains("timonier"))
        .filter_map(|n| Some((n.id.as_str(), n.role?)))
        .collect();
    for (package, role) in [
        ("pkg:cargo/timonier-cli", Role::EntryPoint),
        ("pkg:cargo/timonier-core", Role::DataModel),
        ("pkg:cargo/timonier-daemon", Role::EntryPoint),
        ("pkg:cargo/timonier-index", Role::Utility),
        ("pkg:cargo/timonier-store", Role::Persistence),
        ("pkg:npm/@timonier/webview", Role::Interface),
        ("pkg:npm/timonier", Role::Interface),
    ] {
        assert_eq!(roles.get(package), Some(&role), "{package} in {roles:?}");
    }
    // External libraries, grouped by usage.
    let parent = |id: &str| {
        nodes
            .iter()
            .find(|n| n.id.as_str() == id)
            .and_then(|n| n.parent.as_ref())
            .map(|p| p.to_string())
    };
    assert_eq!(
        parent("pkg:cargo/rusqlite").as_deref(),
        Some("component:external/database")
    );
    assert_eq!(
        parent("pkg:cargo/serde").as_deref(),
        Some("component:external/serialization")
    );
    assert_eq!(
        parent("pkg:npm/react").as_deref(),
        Some("component:external/ui")
    );
    assert_eq!(
        parent("component:external/database").as_deref(),
        Some("component:external")
    );
}
