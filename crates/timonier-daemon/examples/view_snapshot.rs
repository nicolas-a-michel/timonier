//! Views of a repository as the daemon sends them, for the map's demonstration page and its
//! tests (PLAN.md 4.7).
//!
//! `cargo run -p timonier-daemon --example view_snapshot -- [repository] [output directory]`
//!
//! The repository (the current directory by default) is indexed into a temporary database, read
//! only. Two snapshots are written: `top.json`, the opening of the map, and `open.json`, with
//! every package and component expanded.

use std::path::PathBuf;

use timonier_core::NodeKind;
use timonier_core::protocol::ViewSpec;
use timonier_daemon::Pipeline;
use timonier_daemon::view::{self, ViewState};
use timonier_store::Store;

fn main() {
    let mut args = std::env::args().skip(1);
    let root = PathBuf::from(args.next().unwrap_or_else(|| ".".into()));
    let out = PathBuf::from(
        args.next()
            .unwrap_or_else(|| "webview/test/views/timonier".into()),
    );
    let db = tempfile::tempdir().expect("temp dir");
    let store = Store::open(db.path().join("graph.sqlite")).expect("store");
    let mut pipeline = Pipeline::new(&root, store.clone()).expect("pipeline");
    let report = pipeline.index_all().expect("index");
    let mut reader = store.reader().expect("reader");
    let graph = view::load(&mut reader, &root).expect("graph");
    let head = reader.head().expect("head").seq;
    let open: Vec<_> = reader
        .nodes_at(head)
        .expect("nodes")
        .into_iter()
        .filter(|n| matches!(n.kind, NodeKind::Package | NodeKind::Component))
        .map(|n| n.id)
        .collect();
    std::fs::create_dir_all(&out).expect("output directory");
    for (name, spec) in [
        ("top", ViewSpec::default()),
        ("open", ViewSpec { expanded: open }),
    ] {
        let snapshot = ViewState::compute(&graph, &spec, None).snapshot(1, 0);
        let file = out.join(format!("{name}.json"));
        let json = serde_json::to_string_pretty(&serde_json::json!({
            "spec": spec,
            "snapshot": snapshot,
        }))
        .expect("json");
        std::fs::write(&file, json + "\n").expect("write");
        println!(
            "{}: {} nodes, {} edges ({} files indexed)",
            file.display(),
            snapshot.nodes.len(),
            snapshot.edges.len(),
            report.index.files
        );
    }
}
