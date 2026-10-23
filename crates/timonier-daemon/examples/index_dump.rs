//! What the indexer finds in a repository, to judge the graph by eye.
//!
//! `cargo run --release -p timonier-daemon --example index_dump [repository]`
//!
//! `TIMONIER_DUMP_EDGES=<file>` also writes every edge between two files of the repository, a
//! symbol's edges lifted to its file, as `kind<TAB>provenance<TAB>from<TAB>to` lines: what
//! PLAN.md 11.1 compares with a SCIP index.
//!
//! `TIMONIER_SCIP_COMPARE=<language>=<index.scip>` compares tree-sitter's file links with that
//! index's, on the files both know: precision and recall, strict and lenient (PLAN.md 11.1).
//!
//! `TIMONIER_SCIP_INDEX=<language>=<index.scip>` reads that index into the graph as the daemon
//! does after a commit, and prints the share of confirmed links before and after (PLAN.md 11.8):
//! between files, and between the innermost boxes of the map with every box open.

use std::collections::{BTreeMap, HashSet};

use timonier_core::{NodeId, NodeKind};
use timonier_daemon::Pipeline;
use timonier_store::Store;

/// Confirmed links among all, and the lines drawn sure, partial and approximate (ADR 0074).
fn certainty(store: &Store, boxes: bool, label: &str) -> String {
    let mut reader = store.reader().expect("reader");
    let head = reader.head().expect("head").seq;
    let nodes = reader.nodes_at(head).expect("nodes");
    let edges = reader.edges_at(head).expect("edges");
    let parents = nodes
        .iter()
        .map(|n| (n.id.clone(), n.parent.clone()))
        .collect();
    let visible: HashSet<NodeId> = nodes
        .iter()
        .filter(|n| {
            if boxes {
                matches!(n.kind, NodeKind::Package | NodeKind::Component)
            } else {
                n.kind == NodeKind::File
            }
        })
        .map(|n| n.id.clone())
        .collect();
    // What holds code of the repository: a link to anything else goes to a dependency, which
    // no index of the repository defines.
    let mut holding: HashSet<&NodeId> = HashSet::new();
    let up: std::collections::HashMap<&NodeId, Option<&NodeId>> =
        nodes.iter().map(|n| (&n.id, n.parent.as_ref())).collect();
    for file in nodes.iter().filter(|n| n.kind == NodeKind::File) {
        let mut at = Some(&file.id);
        while let Some(id) = at {
            if !holding.insert(id) {
                break;
            }
            at = up.get(id).copied().flatten();
        }
    }
    let (mut links, mut confirmed, mut sure, mut partial, mut approximate) = (0, 0, 0, 0, 0);
    let (mut outside, mut outside_confirmed) = (0, 0);
    for edge in timonier_core::aggregate::aggregate(&parents, &edges, &visible) {
        // A package's manifest dependency is no link of the code.
        if edge.src == edge.dst || edge.kind == timonier_core::EdgeKind::DependsOn {
            continue;
        }
        if !holding.contains(&edge.dst) {
            outside += edge.edges;
            outside_confirmed += edge.confirmed;
            continue;
        }
        links += edge.edges;
        confirmed += edge.confirmed;
        if std::env::var_os("TIMONIER_DUMP_KINDS").is_some() && edge.confirmed < edge.edges {
            eprintln!(
                "  unsure {label} {} {} -> {} ({}/{})",
                edge.kind, edge.src, edge.dst, edge.confirmed, edge.edges
            );
        }
        if edge.confirmed == edge.edges {
            sure += 1;
        } else if edge.confirmed > 0 {
            partial += 1;
        } else {
            approximate += 1;
        }
    }
    let share = |part: u32, all: u32| {
        if all == 0 {
            0.0
        } else {
            100.0 * f64::from(part) / f64::from(all)
        }
    };
    let lines = sure + partial + approximate;
    format!(
        "{confirmed}/{links} links confirmed ({:.0} %); {lines} lines: {sure} sure ({:.0} %), {partial} partial, {approximate} approximate; to dependencies: {outside_confirmed}/{outside}",
        share(confirmed, links),
        share(sure, lines)
    )
}

fn main() {
    let root = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let db = tempfile::tempdir().expect("temp dir");
    let store = Store::open(db.path().join("graph.sqlite")).expect("store");
    let mut pipeline = Pipeline::new(&root, store.clone()).expect("pipeline");
    pipeline.index_all().expect("index");
    // Components belong to the last commit (ADR 0016): cluster the working tree as it is.
    pipeline.recluster().expect("recluster");
    if let Ok(given) = std::env::var("TIMONIER_SCIP_COMPARE") {
        let (language, path) = given
            .split_once('=')
            .expect("TIMONIER_SCIP_COMPARE is <language>=<index.scip>");
        let project = pipeline.indexer().project();
        let files: std::collections::BTreeMap<String, timonier_index::scip::Indexed> = project
            .files()
            .filter(|f| f.language.as_str() == language)
            .map(|f| {
                (
                    f.path.clone(),
                    timonier_index::scip::Indexed {
                        id: f.id.clone(),
                        content_hash: f.content_hash,
                    },
                )
            })
            .collect();
        let bytes = std::fs::read(path).expect("the index");
        let root = std::path::PathBuf::from(&root);
        let read = timonier_index::scip::read(
            &bytes,
            |p| files.get(p).cloned(),
            |p| std::fs::read(root.join(p)).ok(),
            |p, at| project.symbol_at(p, at),
        )
        .expect("an index");
        let paths: std::collections::BTreeSet<String> = files.keys().cloned().collect();
        let c = timonier_index::scip::compare(&read, &project.file_links(&paths));
        println!(
            "files {}; links index {}, tree-sitter {}; strict: precision {:.0} %, recall {:.0} %; lenient: precision {:.0} %, recall {:.0} %",
            c.files,
            c.index_links,
            c.tree_sitter_links,
            c.precision * 100.0,
            c.recall * 100.0,
            c.lenient_precision * 100.0,
            c.lenient_recall * 100.0
        );
    }
    if let Ok(given) = std::env::var("TIMONIER_SCIP_INDEX") {
        let (language, path) = given
            .split_once('=')
            .expect("TIMONIER_SCIP_INDEX is <language>=<index.scip>");
        let before = (
            certainty(&store, false, "files-before"),
            certainty(&store, true, "boxes-before"),
        );
        let job = pipeline
            .scip_job("measure")
            .expect("a language with an indexer");
        let files = job
            .languages
            .into_iter()
            .find(|l| l.language == language)
            .expect("the language's files")
            .files;
        let produced = timonier_daemon::scip::Produced {
            sha: "measure".into(),
            language: language.into(),
            indexer: "given".into(),
            seconds: 0.0,
            bytes: std::fs::read(path).expect("the index"),
            files,
        };
        let applied = pipeline.apply_scip(&produced).expect("apply");
        println!(
            "scip: {:.0} % of tree-sitter's links confirmed, applied: {}",
            applied.confirmed.unwrap_or(0.0) * 100.0,
            applied.apply.is_some()
        );
        println!("files before: {}", before.0);
        println!("files after:  {}", certainty(&store, false, "files-after"));
        println!("boxes before: {}", before.1);
        println!("boxes after:  {}", certainty(&store, true, "boxes-after"));
    }
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
    if let Ok(path) = std::env::var("TIMONIER_DUMP_EDGES") {
        // Each node's file: itself, or the file a symbol hangs from.
        let parents: BTreeMap<&str, Option<&str>> = nodes
            .iter()
            .map(|n| (n.id.as_str(), n.parent.as_ref().map(|p| p.as_str())))
            .collect();
        let file_of = |id: &str| {
            let mut at = Some(id);
            while let Some(current) = at {
                if let Some(path) = current.strip_prefix("file:") {
                    return Some(path.to_owned());
                }
                at = parents.get(current).copied().flatten();
            }
            None
        };
        let mut lines = String::new();
        for edge in &edges {
            if let (Some(from), Some(to)) = (file_of(edge.src.as_str()), file_of(edge.dst.as_str()))
                && from != to
            {
                lines.push_str(&format!(
                    "{}\t{}\t{from}\t{to}\n",
                    edge.kind, edge.provenance
                ));
            }
        }
        std::fs::write(&path, lines).expect("write the edges");
        println!("edges between files written to {path}");
    }
}
