//! What the indexer finds in a repository, to judge the graph by eye.
//!
//! `cargo run --release -p timonier-daemon --example index_dump [repository]`

use std::collections::BTreeMap;

use timonier_core::NodeKind;
use timonier_daemon::Pipeline;
use timonier_store::Store;

fn main() {
    let root = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let db = tempfile::tempdir().expect("temp dir");
    let store = Store::open(db.path().join("graph.sqlite")).expect("store");
    let mut pipeline = Pipeline::new(&root, store.clone()).expect("pipeline");
    pipeline.index_all().expect("index");
    // Components belong to the last commit (ADR 0016): cluster the working tree as it is.
    pipeline.recluster().expect("recluster");
    let mut reader = store.reader().expect("reader");
    let head = reader.head().expect("head").seq;
    let nodes = reader.nodes_at(head).expect("nodes");
    let edges = reader.edges_at(head).expect("edges");
    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    for node in &nodes {
        *by_kind.entry(node.kind.to_string()).or_default() += 1;
    }
    println!("nodes: {by_kind:?}");
    // The tree down to files, with roles.
    fn print(nodes: &[timonier_core::Node], parent: &timonier_core::NodeId, depth: usize) {
        for node in nodes.iter().filter(|n| n.parent.as_ref() == Some(parent)) {
            if node.kind.is_symbol() {
                continue;
            }
            let role = node.role.map_or(String::new(), |r| format!(" [{r}]"));
            println!("{}{} {}{role}", "  ".repeat(depth), node.kind, node.id);
            if node.kind != NodeKind::File {
                print(nodes, &node.id, depth + 1);
            }
        }
    }
    if let Some(repo) = nodes.iter().find(|n| n.kind == NodeKind::Repo) {
        println!("{}", repo.id);
        print(&nodes, &repo.id, 1);
    }
    let mut by_edge: BTreeMap<String, usize> = BTreeMap::new();
    for edge in &edges {
        *by_edge
            .entry(format!("{} {}", edge.kind, edge.provenance))
            .or_default() += 1;
    }
    println!("edges: {by_edge:?}");
    let mut heavy: Vec<_> = edges
        .iter()
        .filter(|e| e.src.as_str().starts_with("file:"))
        .collect();
    heavy.sort_by_key(|e| std::cmp::Reverse(e.weight));
    for edge in heavy.iter().take(40) {
        println!(
            "  {:>4} {} {} -> {}",
            edge.weight, edge.kind, edge.src, edge.dst
        );
    }
    for edge in edges.iter().filter(|e| e.kind.to_string() == "depends_on") {
        println!("  depends_on {} -> {}", edge.src, edge.dst);
    }
}
