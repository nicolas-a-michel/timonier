//! The indexing pipeline and the watcher, against a real store (ADR 0011, 0013).

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use timonier_core::{EdgeKind, NodeId, NodeKind};
use timonier_daemon::{Event, Pipeline, Report, Watch};
use timonier_store::Store;

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().expect("a file has a parent")).expect("create dirs");
    std::fs::write(path, text).expect("write file");
}

/// A small workspace; the database lives in another directory, outside what is watched.
fn repository() -> (tempfile::TempDir, tempfile::TempDir) {
    let repo = tempfile::tempdir().expect("temp dir");
    for (path, text) in [
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
        (
            "crates/core/Cargo.toml",
            "[package]\nname = \"demo-core\"\n",
        ),
        ("crates/core/src/lib.rs", "pub mod node;\n"),
        (
            "crates/core/src/node.rs",
            "pub struct Node;\npub fn make() -> Node { Node }\n",
        ),
        (
            "crates/app/Cargo.toml",
            "[package]\nname = \"demo-app\"\n[dependencies]\ndemo-core = { path = \"../core\" }\n",
        ),
        (
            "crates/app/src/main.rs",
            "use demo_core::node::make;\nfn main() { make(); }\n",
        ),
    ] {
        write(repo.path(), path, text);
    }
    (repo, tempfile::tempdir().expect("temp dir"))
}

fn open(db: &Path) -> Store {
    Store::open(db.join("graph.sqlite")).expect("store")
}

fn head_nodes(store: &Store) -> Vec<timonier_core::Node> {
    let mut reader = store.reader().expect("reader");
    let head = reader.head().expect("head").seq;
    reader.nodes_at(head).expect("nodes")
}

#[test]
fn indexing_fills_the_store_and_a_restart_writes_nothing() {
    let (repo, db) = repository();
    let store = open(db.path());
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    let report = pipeline.index_all().unwrap();
    assert_eq!(report.index.parsed, 3);
    let nodes = head_nodes(&store);
    let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    for expected in [
        "pkg:cargo/demo-core",
        "file:crates/app/src/main.rs",
        "rust:demo_core::node::Node",
        "rust:demo_app::main",
    ] {
        assert!(ids.contains(&expected), "{expected} in {ids:?}");
    }
    let mut reader = store.reader().unwrap();
    let edges = reader.edges_at(reader.head().unwrap().seq).unwrap();
    assert!(edges.iter().any(|e| e.kind == EdgeKind::Calls
        && e.src.as_str() == "file:crates/app/src/main.rs"
        && e.dst.as_str() == "file:crates/core/src/node.rs"));
    assert!(edges.iter().any(|e| e.kind == EdgeKind::DependsOn
        && e.src.as_str() == "pkg:cargo/demo-app"
        && e.dst.as_str() == "pkg:cargo/demo-core"));
    drop(pipeline);
    drop(store);

    // A restart on the same database: the cache spares every parse, and nothing is written.
    let store = open(db.path());
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    let report = pipeline.index_all().unwrap();
    assert_eq!((report.index.parsed, report.index.cached), (0, 3));
    assert_eq!(report.apply, timonier_store::ApplyStats::default());
    assert_eq!(store.total_changes().unwrap(), 0);
}

#[test]
fn a_refresh_updates_the_graph_and_a_deletion_removes_the_file() {
    let (repo, db) = repository();
    let store = open(db.path());
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.index_all().unwrap();
    write(
        repo.path(),
        "crates/core/src/node.rs",
        "pub struct Node;\npub fn make(n: u32) -> Node { Node }\n",
    );
    let report = pipeline
        .refresh(&[PathBuf::from("crates/core/src/node.rs")])
        .unwrap();
    assert_eq!(report.index.parsed, 1);
    assert_eq!(report.apply.nodes_changed, 1, "{report:?}");
    let make = head_nodes(&store)
        .into_iter()
        .find(|n| n.id.as_str() == "rust:demo_core::node::make")
        .unwrap();
    assert_eq!(
        make.signature.as_deref(),
        Some("pub fn make(n: u32) -> Node")
    );

    std::fs::remove_file(repo.path().join("crates/app/src/main.rs")).unwrap();
    let report = pipeline
        .refresh(&[repo.path().join("crates/app/src/main.rs")])
        .unwrap();
    assert_eq!(report.index.removed, 1);
    let nodes = head_nodes(&store);
    assert!(
        !nodes
            .iter()
            .any(|n| n.id.as_str().starts_with("rust:demo_app"))
    );
    // The package has no file left: it leaves the map (ADR 0013).
    assert!(
        !nodes
            .iter()
            .any(|n| n.id == NodeId::package("cargo", "demo-app").unwrap())
    );
    assert!(nodes.iter().any(|n| n.kind == NodeKind::Repo));
}

/// Speed 1, end to end: a save on disk, the debounce, then the graph in SQLite.
#[test]
fn the_watcher_updates_the_graph_after_a_save() {
    let (repo, db) = repository();
    let store = open(db.path());
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.without_scip();
    pipeline.index_all().unwrap();
    let (send, receive) = mpsc::channel::<Report>();
    let watch = Watch::start(pipeline, Duration::from_millis(100), None, move |event| {
        if let Event::Indexed(Ok(report)) = event
            && report.index.parsed > 0
        {
            let _ = send.send(report);
        }
    })
    .unwrap();
    // Some watchers (FSEvents) need a moment before they report.
    std::thread::sleep(Duration::from_millis(500));
    write(
        repo.path(),
        "crates/core/src/node.rs",
        "pub struct Node;\npub fn make() -> Node { Node }\npub fn added() {}\n",
    );
    let report = receive
        .recv_timeout(Duration::from_secs(20))
        .expect("the watcher reports the save");
    assert_eq!(report.index.parsed, 1);
    assert!(
        head_nodes(&store)
            .iter()
            .any(|n| n.id.as_str() == "rust:demo_core::node::added")
    );
    let pipeline = watch.stop().expect("the pipeline comes back");
    assert_eq!(pipeline.indexer().project().files().count(), 3);
}

/// The extraction feeds move detection (ADR 0010): a function moved to another file, and a
/// renamed file, are moves in the review diff, not deletions and additions.
#[test]
fn moves_between_files_show_as_moves_in_the_diff() {
    use timonier_core::{Freeze, NodeChange};
    let (repo, db) = repository();
    let store = open(db.path());
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    write(
        repo.path(),
        "crates/core/src/node.rs",
        "pub struct Node;\npub fn make() -> Node {\n    let n = Node;\n    n\n}\n",
    );
    write(repo.path(), "crates/core/src/edge.rs", "pub struct Edge;\n");
    write(
        repo.path(),
        "crates/core/src/lib.rs",
        "pub mod node;\npub mod edge;\n",
    );
    pipeline.index_all().unwrap();
    let mark = store.freeze(Freeze::ReviewMark).unwrap().seq;

    // `make` moves to edge.rs, reformatted; node.rs is renamed vertex.rs.
    write(
        repo.path(),
        "crates/core/src/edge.rs",
        "pub struct Edge;\n\npub fn make() -> Node { let n = Node; n }\n",
    );
    std::fs::remove_file(repo.path().join("crates/core/src/node.rs")).unwrap();
    write(
        repo.path(),
        "crates/core/src/vertex.rs",
        "pub struct Node;\n",
    );
    pipeline
        .refresh(&[
            PathBuf::from("crates/core/src/edge.rs"),
            PathBuf::from("crates/core/src/node.rs"),
            PathBuf::from("crates/core/src/vertex.rs"),
        ])
        .unwrap();
    let mut reader = store.reader().unwrap();
    let head = reader.head().unwrap().seq;
    let diff = reader.diff(mark, head).unwrap();
    let moved: Vec<(String, String)> = diff
        .nodes
        .iter()
        .filter_map(|change| match change {
            NodeChange::Moved { before, after, .. } => {
                Some((before.id.to_string(), after.id.to_string()))
            }
            _ => None,
        })
        .collect();
    assert!(
        moved.contains(&(
            "rust:demo_core::node::make".into(),
            "rust:demo_core::edge::make".into()
        )),
        "{diff:#?}"
    );
    assert!(
        moved.contains(&(
            "file:crates/core/src/node.rs".into(),
            "file:crates/core/src/vertex.rs".into()
        )),
        "{diff:#?}"
    );
    assert!(
        !diff.nodes.iter().any(|c| matches!(c, NodeChange::Removed { node } if node.id.as_str() == "rust:demo_core::node::make")),
        "{diff:#?}"
    );
}

/// With components (more than 12 files in a package), a restart still writes nothing: the
/// clustering starts from the stored partition and finds it again (ADR 0016).
#[test]
fn a_restart_with_components_writes_nothing() {
    let repo = tempfile::tempdir().unwrap();
    let db = tempfile::tempdir().unwrap();
    write(repo.path(), "Cargo.toml", "[package]\nname = \"big\"\n");
    let mut lib = String::new();
    for group in 0..2 {
        for i in 0..8 {
            let name = format!("g{group}m{i}");
            lib.push_str(&format!("pub mod {name};\n"));
            let next = format!("g{group}m{}", (i + 1) % 8);
            write(
                repo.path(),
                &format!("src/{name}.rs"),
                &format!("use crate::{next}::f;\npub fn g() {{ f(); }}\npub fn f() {{}}\n"),
            );
        }
    }
    write(repo.path(), "src/lib.rs", &lib);
    let store = open(db.path());
    Pipeline::new(repo.path(), store.clone())
        .unwrap()
        .index_all()
        .unwrap();
    let nodes = head_nodes(&store);
    assert!(
        nodes.iter().any(|n| n.kind == NodeKind::Component),
        "{nodes:?}"
    );
    drop(store);
    let store = open(db.path());
    let report = Pipeline::new(repo.path(), store.clone())
        .unwrap()
        .index_all()
        .unwrap();
    assert_eq!(
        report.apply,
        timonier_store::ApplyStats::default(),
        "{report:?}"
    );
    assert_eq!(store.total_changes().unwrap(), 0);
}
