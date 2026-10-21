//! The indexer on a temporary repository: walk, cache, refresh (ADR 0013).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use timonier_core::{NodeKind, ScopeKey, ScopeUpdate};
use timonier_index::{Batch, Cached, Indexer, Packs};

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().expect("a file has a parent")).expect("create dirs");
    std::fs::write(path, text).expect("write file");
}

/// A Cargo workspace with two crates, an npm workspace with one package, and files that must
/// not be indexed.
fn repository() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    for (path, text) in [
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
        (
            "crates/core/Cargo.toml",
            "[package]\nname = \"demo-core\"\n[dependencies]\nserde = \"1\"\n",
        ),
        (
            "crates/core/src/lib.rs",
            "pub mod node;\npub use node::Node;\n",
        ),
        (
            "crates/core/src/node.rs",
            "pub struct Node;\nimpl Node {\n    pub fn new() -> Self { Node }\n}\n",
        ),
        (
            "crates/store/Cargo.toml",
            "[package]\nname = \"demo-store\"\n[dependencies]\ndemo-core = { path = \"../core\" }\n",
        ),
        (
            "crates/store/src/lib.rs",
            "use demo_core::node::Node;\npub fn save() -> Node { Node::new() }\n",
        ),
        (
            "package.json",
            r#"{"name": "ws", "private": true, "workspaces": ["web"]}"#,
        ),
        (
            "web/package.json",
            r#"{"name": "@demo/web", "dependencies": {"react": "19"}}"#,
        ),
        (
            "web/src/main.tsx",
            "import { App } from \"./App\";\nexport const root = <App />;\n",
        ),
        (
            "web/src/App.tsx",
            "export function App() { return null; }\n",
        ),
        // Not indexed: ignored, hidden, or read by no pack.
        (".gitignore", "target/\nnode_modules/\n"),
        ("target/debug/build.rs", "fn main() {}\n"),
        (
            "web/node_modules/react/index.ts",
            "export const React = 1;\n",
        ),
        (".hidden/x.rs", "fn x() {}\n"),
        ("README.md", "# demo\n"),
    ] {
        write(root, path, text);
    }
    dir
}

fn cache_of(batch: &Batch) -> HashMap<String, Cached> {
    batch.cache_put.iter().cloned().collect()
}

fn scopes(batch: &Batch) -> Vec<String> {
    batch
        .updates
        .iter()
        .map(|u| format!("{}{}", if u.remove { "-" } else { "" }, u.scope))
        .collect()
}

fn update<'a>(batch: &'a Batch, scope: &str) -> &'a ScopeUpdate {
    batch
        .updates
        .iter()
        .find(|u| u.scope.as_str() == scope)
        .unwrap_or_else(|| panic!("no update for {scope}: {:?}", scopes(batch)))
}

fn indexer(root: &Path) -> Indexer {
    Indexer::new(root, Packs::builtin().expect("built-in packs")).expect("indexer")
}

#[test]
fn indexes_what_the_packs_read_and_the_gitignore_allows() {
    let dir = repository();
    let mut indexer = indexer(dir.path());
    let batch = indexer.index_all(&HashMap::new(), &[], None).unwrap();
    assert_eq!(
        scopes(&batch),
        [
            "structure",
            "file:crates/core/src/lib.rs",
            "file:crates/core/src/node.rs",
            "file:crates/store/src/lib.rs",
            "file:web/src/App.tsx",
            "file:web/src/main.tsx",
        ]
    );
    assert_eq!(
        (batch.stats.files, batch.stats.parsed, batch.stats.cached),
        (5, 5, 0)
    );
    let structure = update(&batch, "structure");
    let packages: Vec<String> = structure
        .nodes
        .as_ref()
        .unwrap()
        .iter()
        .filter(|n| n.kind == NodeKind::Package)
        .map(|n| n.id.to_string())
        .collect();
    assert_eq!(
        packages,
        [
            "pkg:cargo/demo-core",
            "pkg:cargo/demo-store",
            "pkg:npm/@demo/web",
            "pkg:cargo/serde",
            "pkg:npm/react"
        ]
    );
    let store = update(&batch, "file:crates/store/src/lib.rs");
    let edges: Vec<String> = store
        .observations
        .as_ref()
        .unwrap()
        .observations
        .iter()
        .map(|o| format!("{} {}", o.kind, o.dst))
        .collect();
    assert!(
        edges.contains(&"imports file:crates/core/src/node.rs".to_owned()),
        "{edges:?}"
    );
    assert!(
        edges.contains(&"calls file:crates/core/src/node.rs".to_owned()),
        "{edges:?}"
    );
}

/// Criterion of task 3.4: a second indexing without change parses no file.
#[test]
fn a_second_indexing_with_the_cache_parses_nothing() {
    let dir = repository();
    let mut whole = indexer(dir.path());
    let progress = whole.progress();
    let first = whole.index_all(&HashMap::new(), &[], None).unwrap();
    // T-115: every file read is counted, of all those found.
    use std::sync::atomic::Ordering;
    assert_eq!(
        (
            progress.done.load(Ordering::Relaxed),
            progress.total.load(Ordering::Relaxed)
        ),
        (5, 5)
    );
    let cache = cache_of(&first);
    let second = indexer(dir.path()).index_all(&cache, &[], None).unwrap();
    assert_eq!((second.stats.parsed, second.stats.cached), (0, 5));
    assert!(second.cache_put.is_empty());
    assert_eq!(first.updates, second.updates);
    // A changed file is parsed again, and only it.
    write(
        dir.path(),
        "web/src/App.tsx",
        "export function App() { return 1; }\n",
    );
    let third = indexer(dir.path()).index_all(&cache, &[], None).unwrap();
    assert_eq!((third.stats.parsed, third.stats.cached), (1, 4));
}

#[test]
fn files_deleted_while_away_are_removed_from_the_store_and_the_cache() {
    let dir = repository();
    let first = indexer(dir.path())
        .index_all(&HashMap::new(), &[], None)
        .unwrap();
    let stored: Vec<ScopeKey> = first.updates.iter().map(|u| u.scope.clone()).collect();
    std::fs::remove_file(dir.path().join("web/src/App.tsx")).unwrap();
    let second = indexer(dir.path())
        .index_all(&cache_of(&first), &stored, None)
        .unwrap();
    assert!(scopes(&second).contains(&"-file:web/src/App.tsx".to_owned()));
    assert_eq!(second.cache_remove, ["web/src/App.tsx"]);
    assert_eq!(second.stats.removed, 1);
}

#[test]
fn a_saved_file_is_reparsed_incrementally_with_the_same_result() {
    let dir = repository();
    let mut indexer = indexer(dir.path());
    indexer.index_all(&HashMap::new(), &[], None).unwrap();
    let path = "crates/core/src/node.rs";
    let mut text = std::fs::read_to_string(dir.path().join(path)).unwrap();
    for (i, addition) in [
        "pub fn a() {}\n",
        "pub fn b(x: u32) -> u32 { x }\n",
        "// note\n",
    ]
    .iter()
    .enumerate()
    {
        text.push_str(addition);
        write(dir.path(), path, &text);
        let batch = indexer.refresh(&[PathBuf::from(path)]).unwrap();
        assert_eq!(batch.stats.parsed, 1);
        // The first save after a full indexing has no tree yet.
        assert_eq!(batch.stats.incremental, usize::from(i > 0));
        let fresh = Packs::builtin()
            .unwrap()
            .extract(path, text.as_bytes())
            .unwrap();
        assert_eq!(indexer.project().file(path).unwrap().facts, fresh);
    }
    // Saving the same content again parses nothing.
    let batch = indexer.refresh(&[dir.path().join(path)]).unwrap();
    assert_eq!((batch.stats.parsed, batch.stats.unchanged), (0, 1));
    assert!(batch.updates.is_empty());
}

#[test]
fn renaming_a_definition_resolves_its_citers_again() {
    let dir = repository();
    let mut indexer = indexer(dir.path());
    indexer.index_all(&HashMap::new(), &[], None).unwrap();
    write(
        dir.path(),
        "crates/core/src/node.rs",
        "pub struct Vertex;\nimpl Vertex {\n    pub fn new() -> Self { Vertex }\n}\n",
    );
    let batch = indexer
        .refresh(&[PathBuf::from("crates/core/src/node.rs")])
        .unwrap();
    assert_eq!(batch.stats.parsed, 1);
    assert_eq!(
        scopes(&batch),
        [
            "file:crates/core/src/node.rs",
            "file:crates/core/src/lib.rs",
            "file:crates/store/src/lib.rs"
        ]
    );
    let store = update(&batch, "file:crates/store/src/lib.rs");
    assert!(
        store.nodes.is_none() && store.content_hash.is_none(),
        "edges only"
    );
}

#[test]
fn deleted_new_and_ignored_files() {
    let dir = repository();
    let mut indexer = indexer(dir.path());
    indexer.index_all(&HashMap::new(), &[], None).unwrap();
    std::fs::remove_file(dir.path().join("web/src/App.tsx")).unwrap();
    let batch = indexer
        .refresh(&[PathBuf::from("web/src/App.tsx")])
        .unwrap();
    assert_eq!(
        scopes(&batch),
        [
            "structure",
            "file:web/src/main.tsx",
            "-file:web/src/App.tsx"
        ]
    );
    assert_eq!(batch.cache_remove, ["web/src/App.tsx"]);
    // A new directory with a file.
    write(
        dir.path(),
        "web/src/views/Map.tsx",
        "export function Map() {}\n",
    );
    let batch = indexer
        .refresh(&[dir.path().join("web/src/views")])
        .unwrap();
    assert_eq!(scopes(&batch), ["structure", "file:web/src/views/Map.tsx"]);
    // Ignored and unread files change nothing.
    write(dir.path(), "target/debug/new.rs", "fn n() {}\n");
    write(dir.path(), "notes.txt", "x\n");
    let batch = indexer
        .refresh(&[
            PathBuf::from("target/debug/new.rs"),
            PathBuf::from("notes.txt"),
        ])
        .unwrap();
    assert!(batch.updates.is_empty(), "{:?}", scopes(&batch));
    // A deleted directory removes its files.
    std::fs::remove_dir_all(dir.path().join("web/src/views")).unwrap();
    let batch = indexer.refresh(&[PathBuf::from("web/src/views")]).unwrap();
    assert!(scopes(&batch).contains(&"-file:web/src/views/Map.tsx".to_owned()));
}

#[test]
fn a_manifest_change_places_every_file_again() {
    let dir = repository();
    let mut indexer = indexer(dir.path());
    indexer.index_all(&HashMap::new(), &[], None).unwrap();
    write(
        dir.path(),
        "crates/core/Cargo.toml",
        "[package]\nname = \"demo-core\"\n[lib]\nname = \"core2\"\n[dependencies]\nserde = \"1\"\n",
    );
    let batch = indexer
        .refresh(&[PathBuf::from("crates/core/Cargo.toml")])
        .unwrap();
    assert_eq!(batch.stats.parsed, 0, "contents did not change");
    let node = update(&batch, "file:crates/core/src/node.rs");
    let ids: Vec<String> = node
        .nodes
        .as_ref()
        .unwrap()
        .iter()
        .map(|n| n.id.to_string())
        .collect();
    assert_eq!(
        ids,
        ["rust:core2::node::Node", "rust:core2::node::Node::new"]
    );
}

/// ADR 0016: a file the clustering left alone stays alone; a file that arrived since joins the
/// component most of its directory neighbours are in.
#[test]
fn singletons_stay_alone_and_newcomers_join_their_neighbours() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "Cargo.toml", "[package]\nname = \"k\"\n");
    let mut lib = String::new();
    for group in 0..2 {
        for i in 0..6 {
            let name = format!("g{group}m{i}");
            lib.push_str(&format!("pub mod {name};\n"));
            let next = format!("g{group}m{}", (i + 1) % 6);
            write(
                root,
                &format!("src/{name}.rs"),
                &format!("pub fn f() {{ crate::{next}::f(); }}\n"),
            );
        }
    }
    lib.push_str("pub mod lone;\n");
    write(root, "src/lone.rs", "pub fn alone() {}\n");
    write(root, "src/lib.rs", &lib);
    let mut indexer = indexer(root);
    let batch = indexer.index_all(&HashMap::new(), &[], None).unwrap();
    let parent = |batch: &Batch, file: &str| -> String {
        update(batch, "structure")
            .nodes
            .as_ref()
            .unwrap()
            .iter()
            .find(|n| n.id.as_str() == file)
            .and_then(|n| n.parent.clone())
            .map(|p| p.to_string())
            .unwrap_or_default()
    };
    assert!(parent(&batch, "file:src/g0m0.rs").starts_with("component:cluster/"));
    assert_eq!(parent(&batch, "file:src/lone.rs"), "pkg:cargo/k");
    write(root, "src/fresh.rs", "pub fn fresh() {}\n");
    let batch = indexer.refresh(&[PathBuf::from("src/fresh.rs")]).unwrap();
    assert!(
        parent(&batch, "file:src/fresh.rs").starts_with("component:cluster/"),
        "{:?}",
        parent(&batch, "file:src/fresh.rs")
    );
    assert_eq!(parent(&batch, "file:src/lone.rs"), "pkg:cargo/k");
}

/// A root reached through a symbolic link (macOS's `/var` → `/private/var`): paths named
/// through the link, or through another link to the same place, are the same files.
#[cfg(unix)]
#[test]
fn paths_through_a_symbolic_link_name_the_same_files() {
    let dir = repository();
    let link_dir = tempfile::tempdir().unwrap();
    let link = link_dir.path().join("repo");
    std::os::unix::fs::symlink(dir.path(), &link).unwrap();
    let mut indexer = indexer(&link);
    indexer.index_all(&HashMap::new(), &[], None).unwrap();
    std::fs::remove_file(link.join("web/src/App.tsx")).unwrap();
    let batch = indexer.refresh(&[link.join("web/src/App.tsx")]).unwrap();
    assert_eq!(batch.stats.removed, 1, "{:?}", scopes(&batch));
    let other = link_dir.path().join("other");
    std::os::unix::fs::symlink(dir.path(), &other).unwrap();
    std::fs::remove_file(dir.path().join("web/src/main.tsx")).unwrap();
    let batch = indexer.refresh(&[other.join("web/src/main.tsx")]).unwrap();
    assert_eq!(batch.stats.removed, 1, "{:?}", scopes(&batch));
}

/// T-119: a whole indexing whose owner stops gives up its files, at once.
#[test]
fn a_whole_indexing_gives_up_when_cancelled() {
    let dir = repository();
    let mut indexer = indexer(dir.path());
    indexer
        .progress()
        .cancelled
        .store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(matches!(
        indexer.index_all(&HashMap::new(), &[], None),
        Err(timonier_index::Error::Cancelled)
    ));
}
