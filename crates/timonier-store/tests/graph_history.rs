//! The store end to end: insertion, update by difference, snapshots and diffs, moves, fusion,
//! scope ownership, tree enforcement, concurrent readers (PLAN.md Phase 2).

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

use timonier_core::{
    ContentHash, Edge, EdgeChange, EdgeKey, EdgeKind, Freeze, Language, MoveEvidence, Node,
    NodeChange, NodeId, NodeKind, Observation, Provenance, ScopeKey, ScopeUpdate, SnapshotKind,
    SourceObservations, SymbolKind, TreeViolation,
};
use timonier_store::{Error, Store};

// ---------------------------------------------------------------------------------------------
// A tiny project: repo `demo`, package `demo`, files under `src/`, functions in modules.
// ---------------------------------------------------------------------------------------------

fn id(s: &str) -> NodeId {
    NodeId::parse(s).expect("valid id")
}

fn scope(file: &str) -> ScopeKey {
    ScopeKey::of_file(&id(&format!("file:{file}"))).expect("file scope")
}

fn rust() -> Language {
    Language::new("rust").expect("valid language")
}

fn hash_of(value: impl Hash) -> ContentHash {
    let mut bytes = [0_u8; 32];
    for (i, chunk) in bytes.chunks_mut(8).enumerate() {
        let mut hasher = DefaultHasher::new();
        (i, &value).hash(&mut hasher);
        chunk.copy_from_slice(&hasher.finish().to_le_bytes());
    }
    ContentHash::from_bytes(bytes)
}

/// The structure scope: repo, package, and the files placed under the package.
fn structure(files: &[&str]) -> ScopeUpdate {
    let repo = Node::new(id("repo:demo"), NodeKind::Repo, "demo");
    let package =
        Node::new(id("pkg:cargo/demo"), NodeKind::Package, "demo").with_parent(repo.id.clone());
    let mut nodes = vec![repo, package];
    for path in files {
        nodes.push(
            Node::new(id(&format!("file:{path}")), NodeKind::File, *path)
                .with_parent(id("pkg:cargo/demo"))
                .with_language(rust()),
        );
    }
    ScopeUpdate::new(ScopeKey::structure()).with_nodes(nodes)
}

/// What tree-sitter reports for one file of the demo.
struct FileContent<'a> {
    path: &'a str,
    /// Qualified name of the module the file defines.
    module: &'a str,
    /// Functions of the module: (name, body).
    functions: &'a [(&'a str, u32)],
    /// Symbol references: (function of this module, qualified target).
    references: &'a [(&'a str, &'a str)],
    /// File-level calls: target file paths (tree-sitter reports calls between files).
    calls: &'a [&'a str],
}

impl FileContent<'_> {
    fn update(&self) -> ScopeUpdate {
        let file = id(&format!("file:{}", self.path));
        let module = id(&format!("rust:{}", self.module));
        let module_name = self.module.rsplit("::").next().unwrap_or(self.module);
        let mut nodes = vec![
            Node::new(
                module.clone(),
                NodeKind::Symbol(SymbolKind::Module),
                module_name,
            )
            .with_parent(file.clone())
            .with_language(rust())
            .with_body_hash(hash_of(self.functions)),
        ];
        for (name, body) in self.functions {
            nodes.push(function(
                &format!("{}::{name}", self.module),
                &module,
                *body,
            ));
        }
        let mut observations: Vec<Observation> = self
            .references
            .iter()
            .map(|(from, to)| {
                Observation::new(
                    id(&format!("rust:{}::{from}", self.module)),
                    id(to),
                    EdgeKind::References,
                )
            })
            .collect();
        observations.extend(
            self.calls.iter().map(|to| {
                Observation::new(file.clone(), id(&format!("file:{to}")), EdgeKind::Calls)
            }),
        );
        ScopeUpdate::new(ScopeKey::of_file(&file).expect("file scope"))
            .with_content_hash(hash_of((
                self.path,
                self.module,
                self.functions,
                self.references,
                self.calls,
            )))
            .with_nodes(nodes)
            .with_observations(SourceObservations::new(
                Provenance::TreeSitter,
                observations,
            ))
    }
}

fn function(qualified: &str, parent: &NodeId, body: u32) -> Node {
    let name = qualified.rsplit("::").next().unwrap_or(qualified);
    Node::new(
        id(&format!("rust:{qualified}")),
        NodeKind::Symbol(SymbolKind::Function),
        name,
    )
    .with_parent(parent.clone())
    .with_language(rust())
    .with_signature(format!("fn {name}()"))
    .with_body_hash(hash_of((name, body)))
}

/// A file `src/<name>.rs` defining `demo::<name>`.
fn file_scope(path: &str, functions: &[(&str, u32)], references: &[(&str, &str)]) -> ScopeUpdate {
    let name = path.trim_start_matches("src/").trim_end_matches(".rs");
    let module = format!("demo::{name}");
    FileContent {
        path,
        module: &module,
        functions,
        references,
        calls: &[],
    }
    .update()
}

fn open() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().expect("temporary directory");
    let store = Store::open(dir.path().join("graph.sqlite")).expect("store opens");
    (dir, store)
}

fn demo(store: &Store) {
    store
        .apply_all(vec![
            structure(&["src/a.rs", "src/b.rs"]),
            file_scope(
                "src/a.rs",
                &[("f", 1), ("g", 2)],
                &[("f", "rust:demo::b::h")],
            ),
            file_scope("src/b.rs", &[("h", 3)], &[]),
        ])
        .expect("demo applies");
}

fn nodes_at(store: &Store, seq: u64) -> Vec<Node> {
    store
        .reader()
        .expect("reader")
        .nodes_at(seq)
        .expect("nodes")
}

fn ids_at(store: &Store, seq: u64) -> BTreeSet<String> {
    nodes_at(store, seq)
        .into_iter()
        .map(|n| n.id.to_string())
        .collect()
}

fn edges_at(store: &Store, seq: u64) -> Vec<Edge> {
    store
        .reader()
        .expect("reader")
        .edges_at(seq)
        .expect("edges")
}

fn head(store: &Store) -> u64 {
    store.reader().expect("reader").head().expect("head").seq
}

fn node(store: &Store, node: &str) -> Option<Node> {
    store
        .reader()
        .expect("reader")
        .node_at(&id(node), head(store))
        .expect("node")
}

fn versions_of(store: &Store, node: &str) -> i64 {
    let connection = rusqlite::Connection::open(store.path()).expect("inspection connection");
    connection
        .query_row("SELECT count(*) FROM node WHERE id = ?1", [node], |r| {
            r.get(0)
        })
        .expect("count")
}

fn summary(changes: &[NodeChange]) -> Vec<(String, &'static str)> {
    changes
        .iter()
        .map(|change| {
            let what = match change {
                NodeChange::Added { .. } => "added",
                NodeChange::Removed { .. } => "removed",
                NodeChange::Modified { .. } => "modified",
                NodeChange::Moved { .. } => "moved",
            };
            (change.id().to_string(), what)
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Insertion and update by difference
// ---------------------------------------------------------------------------------------------

#[test]
fn insertion_makes_the_graph_visible_at_the_head() {
    let (_dir, store) = open();
    demo(&store);
    let expected: BTreeSet<String> = [
        "file:src/a.rs",
        "file:src/b.rs",
        "pkg:cargo/demo",
        "repo:demo",
        "rust:demo::a",
        "rust:demo::a::f",
        "rust:demo::a::g",
        "rust:demo::b",
        "rust:demo::b::h",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    assert_eq!(ids_at(&store, head(&store)), expected);

    let edges = edges_at(&store, head(&store));
    assert_eq!(edges.len(), 1);
    assert_eq!(
        (edges[0].src.as_str(), edges[0].dst.as_str()),
        ("rust:demo::a::f", "rust:demo::b::h")
    );
    assert_eq!(edges[0].provenance, Provenance::TreeSitter);
}

#[test]
fn reapplying_an_identical_update_writes_nothing() {
    let (_dir, store) = open();
    demo(&store);
    let before = store.total_changes().unwrap();
    let again = store
        .apply_all(vec![
            structure(&["src/a.rs", "src/b.rs"]),
            file_scope(
                "src/a.rs",
                &[("f", 1), ("g", 2)],
                &[("f", "rust:demo::b::h")],
            ),
        ])
        .unwrap();
    assert!(again.is_empty(), "{again:?}");
    assert_eq!(
        store.total_changes().unwrap(),
        before,
        "no row may be written"
    );
}

#[test]
fn a_change_rewrites_in_place_until_the_head_is_frozen() {
    let (_dir, store) = open();
    demo(&store);
    // In the head, nobody saw the first version: it is rewritten in place.
    let stats = store
        .apply(file_scope(
            "src/a.rs",
            &[("f", 10), ("g", 2)],
            &[("f", "rust:demo::b::h")],
        ))
        .unwrap();
    assert_eq!(
        (stats.nodes_changed, stats.nodes_added, stats.nodes_removed),
        (2, 0, 0)
    ); // f and the module
    assert_eq!(versions_of(&store, "rust:demo::a::f"), 1);

    // Once frozen, a change closes the old version and opens a new one.
    store.freeze(Freeze::ReviewMark).unwrap();
    store
        .apply(file_scope(
            "src/a.rs",
            &[("f", 11), ("g", 2)],
            &[("f", "rust:demo::b::h")],
        ))
        .unwrap();
    assert_eq!(versions_of(&store, "rust:demo::a::f"), 2);
}

#[test]
fn a_moved_span_is_updated_without_a_new_version() {
    let (_dir, store) = open();
    demo(&store);
    store.freeze(Freeze::ReviewMark).unwrap();
    let mut update = file_scope(
        "src/a.rs",
        &[("f", 1), ("g", 2)],
        &[("f", "rust:demo::b::h")],
    );
    let span = timonier_core::Span::new(
        timonier_core::Position::new(4, 0),
        timonier_core::Position::new(6, 1),
    )
    .unwrap();
    if let Some(nodes) = update.nodes.as_mut() {
        nodes[1] = nodes[1].clone().with_span(span);
    }
    let stats = store.apply(update).unwrap();
    assert_eq!((stats.spans_moved, stats.nodes_changed), (1, 0));
    assert_eq!(versions_of(&store, "rust:demo::a::f"), 1);
    assert_eq!(node(&store, "rust:demo::a::f").unwrap().span, Some(span));
}

#[test]
fn a_node_removed_then_restored_after_a_freeze_shows_no_change() {
    let (_dir, store) = open();
    demo(&store);
    let review = store.freeze(Freeze::ReviewMark).unwrap();
    store
        .apply(file_scope(
            "src/a.rs",
            &[("f", 1)],
            &[("f", "rust:demo::b::h")],
        ))
        .unwrap();
    store
        .apply(file_scope(
            "src/a.rs",
            &[("f", 1), ("g", 2)],
            &[("f", "rust:demo::b::h")],
        ))
        .unwrap();
    let diff = store
        .reader()
        .unwrap()
        .diff(review.seq, head(&store))
        .unwrap();
    assert!(diff.is_empty(), "{diff:#?}");
    assert_eq!(ids_at(&store, review.seq), ids_at(&store, head(&store)));
}

#[test]
fn a_batch_may_list_children_before_their_parents() {
    let (_dir, store) = open();
    store
        .apply_all(vec![
            file_scope("src/b.rs", &[("h", 3)], &[]),
            file_scope("src/a.rs", &[("f", 1)], &[]),
            structure(&["src/a.rs", "src/b.rs"]),
        ])
        .unwrap();
    assert_eq!(ids_at(&store, head(&store)).len(), 8);
}

// ---------------------------------------------------------------------------------------------
// Snapshots and diffs
// ---------------------------------------------------------------------------------------------

#[test]
fn snapshots_are_listed_and_the_last_review_mark_is_found() {
    let (_dir, store) = open();
    let commit = store
        .freeze(Freeze::Commit {
            sha: "6d3e0f8".into(),
        })
        .unwrap();
    let review = store.freeze(Freeze::ReviewMark).unwrap();
    let reader = store.reader().unwrap();
    let kinds: Vec<_> = reader
        .snapshots()
        .unwrap()
        .iter()
        .map(|s| (s.seq, s.kind, s.frozen))
        .collect();
    assert_eq!(
        kinds,
        [
            (commit.seq, SnapshotKind::Commit, true),
            (review.seq, SnapshotKind::ReviewMark, true),
            (review.seq + 1, SnapshotKind::WorkingTree, false),
        ]
    );
    assert_eq!(
        reader.snapshot(commit.seq).unwrap().commit_sha.as_deref(),
        Some("6d3e0f8")
    );
    assert_eq!(
        reader
            .latest(SnapshotKind::ReviewMark)
            .unwrap()
            .unwrap()
            .seq,
        review.seq
    );
    assert!(matches!(
        reader.snapshot(99),
        Err(Error::UnknownSnapshot(99))
    ));
}

/// ADR 0024: a review mark keeps the content of each file as reviewed, for the last review only.
#[test]
fn a_review_mark_keeps_the_content_of_each_file() {
    let (_dir, store) = open();
    demo(&store);
    let contents = || store.reader().unwrap().file_contents().unwrap();
    let reviewed = |seq| store.reader().unwrap().reviewed_contents(seq).unwrap();
    let first_contents = contents();
    assert_eq!(first_contents.len(), 2, "{first_contents:?}");
    let commit = store
        .freeze(Freeze::Commit {
            sha: "6d3e0f8".into(),
        })
        .unwrap();
    assert!(reviewed(commit.seq).is_empty(), "a commit keeps none");
    let first = store.freeze(Freeze::ReviewMark).unwrap();
    assert_eq!(reviewed(first.seq), first_contents);
    store
        .apply_all(vec![file_scope("src/b.rs", &[("h", 4)], &[])])
        .unwrap();
    let second_contents = contents();
    assert_ne!(second_contents, first_contents);
    assert_eq!(reviewed(first.seq), first_contents, "as reviewed");
    let second = store.freeze(Freeze::ReviewMark).unwrap();
    assert!(reviewed(first.seq).is_empty(), "only the last review");
    assert_eq!(reviewed(second.seq), second_contents);
}

/// Review n° 1 of Phase 5, m11: reads in one transaction see one state, whatever the writer
/// commits meanwhile.
#[test]
fn reads_in_one_transaction_see_one_state() {
    let (_dir, store) = open();
    demo(&store);
    let mut reader = store.reader().unwrap();
    let (first, again) = reader
        .consistent(|reader| -> timonier_store::Result<_> {
            let first = reader.file_contents()?;
            store
                .apply_all(vec![file_scope("src/b.rs", &[("h", 9)], &[])])
                .expect("the writer goes on");
            Ok((first, reader.file_contents()?))
        })
        .unwrap();
    assert_eq!(first, again, "one state inside");
    assert_ne!(reader.file_contents().unwrap(), first, "the new one after");
}

#[test]
fn the_diff_since_a_review_lists_additions_removals_and_changes() {
    let (_dir, store) = open();
    demo(&store);
    let review = store.freeze(Freeze::ReviewMark).unwrap();
    store
        .apply_all(vec![
            // g disappears, f refers to k instead of h.
            file_scope("src/a.rs", &[("f", 1)], &[("f", "rust:demo::b::k")]),
            // h's body changes, k appears.
            file_scope("src/b.rs", &[("h", 30), ("k", 4)], &[]),
        ])
        .unwrap();
    let diff = store
        .reader()
        .unwrap()
        .diff(review.seq, head(&store))
        .unwrap();
    assert_eq!(
        summary(&diff.nodes),
        [
            ("rust:demo::a".to_owned(), "modified"),
            ("rust:demo::a::g".to_owned(), "removed"),
            ("rust:demo::b".to_owned(), "modified"),
            ("rust:demo::b::h".to_owned(), "modified"),
            ("rust:demo::b::k".to_owned(), "added"),
        ]
    );
    let edges: Vec<(&str, &str)> = diff
        .edges
        .iter()
        .map(|change| match change {
            EdgeChange::Added { edge } => ("added", edge.dst.as_str()),
            EdgeChange::Removed { edge } => ("removed", edge.dst.as_str()),
            EdgeChange::Modified { after, .. } => ("modified", after.dst.as_str()),
        })
        .collect();
    assert_eq!(
        edges,
        [("removed", "rust:demo::b::h"), ("added", "rust:demo::b::k")]
    );

    assert!(
        store
            .reader()
            .unwrap()
            .diff(head(&store), head(&store))
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        store.reader().unwrap().diff(2, 1),
        Err(Error::BackwardDiff { .. })
    ));
}

#[test]
fn moving_a_file_whose_symbols_get_new_ids_is_a_move() {
    let (_dir, store) = open();
    demo(&store);
    let review = store.freeze(Freeze::ReviewMark).unwrap();

    // src/a.rs becomes src/c.rs, unchanged: its module and functions get new qualified ids.
    store
        .apply_all(vec![
            structure(&["src/c.rs", "src/b.rs"]),
            ScopeUpdate::removal(scope("src/a.rs")),
            file_scope(
                "src/c.rs",
                &[("f", 1), ("g", 2)],
                &[("f", "rust:demo::b::h")],
            ),
        ])
        .unwrap();
    let diff = store
        .reader()
        .unwrap()
        .diff(review.seq, head(&store))
        .unwrap();

    match diff.node(&id("file:src/c.rs")) {
        Some(NodeChange::Moved {
            before, evidence, ..
        }) => {
            assert_eq!(before.id, id("file:src/a.rs"));
            assert_eq!(
                *evidence,
                MoveEvidence::FollowedItsSymbols { moved: 3, total: 3 }
            );
        }
        other => panic!("expected a file move, got {other:?}"),
    }
    for (old, new) in [
        ("rust:demo::a::f", "rust:demo::c::f"),
        ("rust:demo::a::g", "rust:demo::c::g"),
    ] {
        assert!(
            matches!(diff.node(&id(new)), Some(NodeChange::Moved { before, evidence: MoveEvidence::SameBody, .. }) if before.id == id(old)),
            "{new}"
        );
    }
    assert_eq!(diff.nodes.len(), 4, "only moves: {:#?}", diff.nodes);
    assert!(
        diff.edges.is_empty(),
        "the reference follows the move: {:?}",
        diff.edges
    );
}

#[test]
fn renaming_a_file_whose_symbols_keep_their_ids_is_a_move_whatever_the_batch_order() {
    // Rust: `src/foo/mod.rs` → `src/foo.rs`; the module stays `demo::foo`.
    let old = FileContent {
        path: "src/foo/mod.rs",
        module: "demo::foo",
        functions: &[("f", 1), ("g", 2)],
        references: &[("f", "rust:demo::foo::g")],
        calls: &["src/b.rs"],
    };
    let new = FileContent {
        path: "src/foo.rs",
        ..old
    };
    for structure_first in [true, false] {
        let (_dir, store) = open();
        store
            .apply_all(vec![
                structure(&["src/foo/mod.rs", "src/b.rs"]),
                old.update(),
                file_scope("src/b.rs", &[], &[]),
            ])
            .unwrap();
        let review = store.freeze(Freeze::ReviewMark).unwrap();
        let mut batch = vec![
            structure(&["src/foo.rs", "src/b.rs"]),
            ScopeUpdate::removal(scope("src/foo/mod.rs")),
            new.update(),
        ];
        if !structure_first {
            batch.reverse();
        }
        store.apply_all(batch).unwrap();
        let diff = store
            .reader()
            .unwrap()
            .diff(review.seq, head(&store))
            .unwrap();
        assert!(
            matches!(
                diff.node(&id("rust:demo::foo")),
                Some(NodeChange::Moved {
                    evidence: MoveEvidence::SameId,
                    ..
                })
            ),
            "{:#?}",
            diff.nodes
        );
        assert!(
            matches!(diff.node(&id("file:src/foo.rs")), Some(NodeChange::Moved { before, .. }) if before.id == id("file:src/foo/mod.rs")),
            "{:#?}",
            diff.nodes
        );
        assert_eq!(
            diff.nodes.len(),
            2,
            "the functions did not change: {:#?}",
            diff.nodes
        );
        assert!(
            diff.edges.is_empty(),
            "the file-level call follows the file: {:?}",
            diff.edges
        );
    }
}

#[test]
fn a_symbol_keeping_its_id_in_another_file_is_a_move() {
    // A method keeps its id `demo::T::m` when it moves from one file's impl block to another's.
    let (_dir, store) = open();
    let file_with = |file: &str, with_method: bool| {
        let file = id(&format!("file:{file}"));
        let mut nodes = Vec::new();
        if with_method {
            nodes.push(
                Node::new(
                    id("rust:demo::T::m"),
                    NodeKind::Symbol(SymbolKind::Method),
                    "m",
                )
                .with_parent(file.clone())
                .with_signature("fn m(&self)")
                .with_body_hash(hash_of("m")),
            );
        }
        ScopeUpdate::new(ScopeKey::of_file(&file).unwrap())
            .with_content_hash(hash_of((file.as_str(), with_method)))
            .with_nodes(nodes)
    };
    store
        .apply_all(vec![
            structure(&["src/a.rs", "src/b.rs"]),
            file_with("src/a.rs", true),
            file_with("src/b.rs", false),
        ])
        .unwrap();
    let review = store.freeze(Freeze::ReviewMark).unwrap();
    store
        .apply_all(vec![
            file_with("src/a.rs", false),
            file_with("src/b.rs", true),
        ])
        .unwrap();
    let diff = store
        .reader()
        .unwrap()
        .diff(review.seq, head(&store))
        .unwrap();
    assert_eq!(
        summary(&diff.nodes),
        [("rust:demo::T::m".to_owned(), "moved")]
    );
}

#[test]
fn diffs_between_any_two_snapshots_are_consistent() {
    let (_dir, store) = open();
    demo(&store);
    let first = store.freeze(Freeze::ReviewMark).unwrap();
    store
        .apply(file_scope("src/b.rs", &[("h", 3), ("k", 4)], &[]))
        .unwrap();
    store.freeze(Freeze::WorkingTree).unwrap();
    store
        .apply(file_scope("src/b.rs", &[("k", 4)], &[]))
        .unwrap();
    // From the first snapshot to the head, k was added and h removed, whatever happened between.
    let diff = store
        .reader()
        .unwrap()
        .diff(first.seq, head(&store))
        .unwrap();
    assert_eq!(
        summary(&diff.nodes),
        [
            ("rust:demo::b".to_owned(), "modified"),
            ("rust:demo::b::h".to_owned(), "removed"),
            ("rust:demo::b::k".to_owned(), "added"),
        ]
    );
}

// ---------------------------------------------------------------------------------------------
// Fusion of sources and content anchoring
// ---------------------------------------------------------------------------------------------

fn lsp(observations: Vec<Observation>, kinds: &[EdgeKind]) -> SourceObservations {
    SourceObservations::new(Provenance::Lsp, observations).verifying(kinds.iter().copied())
}

fn reference(src: &str, dst: &str) -> Observation {
    Observation::new(id(src), id(dst), EdgeKind::References)
}

#[test]
fn the_lsp_confirms_and_contradicts_tree_sitter_until_the_file_changes() {
    let (_dir, store) = open();
    demo(&store);
    let refs = [("f", "rust:demo::b::h"), ("g", "rust:demo::b::h")];
    let content = file_scope("src/a.rs", &[("f", 1), ("g", 2)], &refs);
    let hash = content.content_hash.unwrap();
    store.apply(content).unwrap();

    // The LSP, on the same content, sees only f → h and has checked every reference of the file.
    let answer = lsp(
        vec![reference("rust:demo::a::f", "rust:demo::b::h")],
        &[EdgeKind::References],
    )
    .computed_on(hash);
    store
        .apply(ScopeUpdate::new(scope("src/a.rs")).with_observations(answer))
        .unwrap();
    let mut reader = store.reader().unwrap();
    let seen: Vec<_> = reader
        .edges_at(head(&store))
        .unwrap()
        .iter()
        .map(|e| (e.src.to_string(), e.provenance))
        .collect();
    assert_eq!(seen, [("rust:demo::a::f".to_owned(), Provenance::Lsp)]);
    assert_eq!(
        reader
            .verified(&scope("src/a.rs"), Provenance::Lsp)
            .unwrap(),
        [EdgeKind::References]
    );
    assert_eq!(reader.content_hash(&scope("src/a.rs")).unwrap(), Some(hash));

    // The file changes: the LSP's observations are stale, tree-sitter's view is back.
    store
        .apply(file_scope("src/a.rs", &[("f", 5), ("g", 2)], &refs))
        .unwrap();
    let edges = reader.edges_at(head(&store)).unwrap();
    assert_eq!(edges.len(), 2);
    assert!(edges.iter().all(|e| e.provenance == Provenance::TreeSitter));
    assert!(
        reader
            .verified(&scope("src/a.rs"), Provenance::Lsp)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_late_lsp_answer_about_an_old_content_is_ignored() {
    let (_dir, store) = open();
    demo(&store);
    let old = file_scope(
        "src/a.rs",
        &[("f", 1), ("g", 2)],
        &[("f", "rust:demo::b::h")],
    );
    let old_hash = old.content_hash.unwrap();
    // The file is saved again before the LSP answers about the old content.
    let new = file_scope(
        "src/a.rs",
        &[("f", 7), ("g", 2)],
        &[("g", "rust:demo::b::h")],
    );
    let new_hash = new.content_hash.unwrap();
    store.apply(new).unwrap();
    let late = lsp(
        vec![reference("rust:demo::a::f", "rust:demo::b::h")],
        &[EdgeKind::References],
    )
    .computed_on(old_hash);
    let stats = store
        .apply(ScopeUpdate::new(scope("src/a.rs")).with_observations(late))
        .unwrap();
    assert_eq!(stats.stale_observations, 1);

    let mut reader = store.reader().unwrap();
    assert_eq!(
        reader.content_hash(&scope("src/a.rs")).unwrap(),
        Some(new_hash),
        "the scope stays on the new content"
    );
    assert!(
        reader
            .verified(&scope("src/a.rs"), Provenance::Lsp)
            .unwrap()
            .is_empty()
    );
    let edges = reader.edges_at(head(&store)).unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(
        (edges[0].src.as_str(), edges[0].provenance),
        ("rust:demo::a::g", Provenance::TreeSitter)
    );
}

#[test]
fn a_precise_source_cannot_set_the_content_nor_omit_it() {
    let (_dir, store) = open();
    demo(&store);
    let old = file_scope(
        "src/a.rs",
        &[("f", 1), ("g", 2)],
        &[("f", "rust:demo::b::h")],
    );
    let old_hash = old.content_hash.unwrap();
    let answer = lsp(
        vec![reference("rust:demo::a::f", "rust:demo::b::h")],
        &[EdgeKind::References],
    );

    // A late answer that would also bring its old content back is refused.
    let with_content = ScopeUpdate::new(scope("src/a.rs"))
        .with_content_hash(old_hash)
        .with_observations(answer.clone().computed_on(old_hash));
    let result = store.apply(with_content);
    assert!(
        matches!(result, Err(Error::PreciseSourceSetsContent { .. })),
        "{result:?}"
    );

    // An answer that does not say which content it describes is refused.
    let result = store.apply(ScopeUpdate::new(scope("src/a.rs")).with_observations(answer));
    assert!(
        matches!(result, Err(Error::UnanchoredObservations { .. })),
        "{result:?}"
    );
}

#[test]
fn an_lsp_answer_after_the_file_was_removed_is_stale() {
    let (_dir, store) = open();
    demo(&store);
    let hash = file_scope(
        "src/a.rs",
        &[("f", 1), ("g", 2)],
        &[("f", "rust:demo::b::h")],
    )
    .content_hash
    .unwrap();
    store
        .apply_all(vec![
            ScopeUpdate::removal(scope("src/a.rs")),
            structure(&["src/b.rs"]),
        ])
        .unwrap();
    let late = lsp(
        vec![reference("rust:demo::a::f", "rust:demo::b::h")],
        &[EdgeKind::References],
    )
    .computed_on(hash);
    let stats = store
        .apply(ScopeUpdate::new(scope("src/a.rs")).with_observations(late))
        .unwrap();
    assert_eq!(stats.stale_observations, 1);
    assert!(edges_at(&store, head(&store)).is_empty());
}

#[test]
fn only_the_lsp_and_scip_may_store_calls_between_symbols() {
    let (_dir, store) = open();
    demo(&store);
    let calls = SourceObservations::new(
        Provenance::TreeSitter,
        vec![Observation::new(
            id("rust:demo::a::f"),
            id("rust:demo::b::h"),
            EdgeKind::Calls,
        )],
    );
    let result = store.apply(ScopeUpdate::new(scope("src/a.rs")).with_observations(calls));
    assert!(
        matches!(
            result,
            Err(Error::Model(
                timonier_core::Error::ApproximateSymbolCall { .. }
            ))
        ),
        "{result:?}"
    );

    // Calls between files from tree-sitter, and between functions from the LSP, are fine.
    let file_calls = FileContent {
        path: "src/a.rs",
        module: "demo::a",
        functions: &[("f", 1)],
        references: &[],
        calls: &["src/b.rs"],
    };
    let hash = file_calls.update().content_hash.unwrap();
    store.apply(file_calls.update()).unwrap();
    let precise = lsp(
        vec![Observation::new(
            id("rust:demo::a::f"),
            id("rust:demo::b::h"),
            EdgeKind::Calls,
        )],
        &[],
    )
    .computed_on(hash);
    store
        .apply(ScopeUpdate::new(scope("src/a.rs")).with_observations(precise))
        .unwrap();
    let kinds: BTreeSet<(String, Provenance)> = edges_at(&store, head(&store))
        .iter()
        .map(|e| (e.src.to_string(), e.provenance))
        .collect();
    assert_eq!(
        kinds,
        [
            ("file:src/a.rs".to_owned(), Provenance::TreeSitter),
            ("rust:demo::a::f".to_owned(), Provenance::Lsp)
        ]
        .into()
    );
}

#[test]
fn intent_and_component_edges_are_refused_by_the_actual_graph() {
    let (_dir, store) = open();
    demo(&store);
    let intended = SourceObservations::new(
        Provenance::Intended,
        vec![Observation::new(
            id("rust:demo::a::f"),
            id("rust:demo::b::h"),
            EdgeKind::DependsOn,
        )],
    );
    let result = store.apply(ScopeUpdate::new(scope("src/a.rs")).with_observations(intended));
    assert!(matches!(
        result,
        Err(Error::Model(timonier_core::Error::IntendedInActualGraph))
    ));

    let component = SourceObservations::new(
        Provenance::Manifest,
        vec![Observation::new(
            id("component:servers"),
            id("pkg:cargo/demo"),
            EdgeKind::DependsOn,
        )],
    );
    let result = store.apply(ScopeUpdate::new(ScopeKey::structure()).with_observations(component));
    assert!(matches!(
        result,
        Err(Error::Model(timonier_core::Error::ComputedEndpoint { .. }))
    ));
}

#[test]
fn dangling_edges_are_kept_but_not_aggregated() {
    let (_dir, store) = open();
    store
        .apply_all(vec![
            structure(&["src/a.rs", "src/b.rs"]),
            file_scope(
                "src/a.rs",
                &[("f", 1)],
                &[("f", "rust:demo::b::h"), ("f", "rust:not::indexed")],
            ),
            file_scope("src/b.rs", &[("h", 3)], &[]),
        ])
        .unwrap();
    let mut reader = store.reader().unwrap();
    let seq = head(&store);
    assert_eq!(reader.edges_at(seq).unwrap().len(), 2);
    let files = [id("file:src/a.rs"), id("file:src/b.rs")];
    let aggregated = reader.aggregate(seq, &files).unwrap();
    assert_eq!(aggregated.len(), 1);
    assert_eq!(
        (&aggregated[0].src, &aggregated[0].dst),
        (&files[0], &files[1])
    );
    assert!(aggregated[0].approximate_only);
    assert!(
        reader
            .aggregate(seq, &[id("pkg:cargo/demo")])
            .unwrap()
            .is_empty()
    );
}

// ---------------------------------------------------------------------------------------------
// Scopes: what they may hold, who owns what several of them provide
// ---------------------------------------------------------------------------------------------

#[test]
fn scopes_hold_only_what_belongs_to_them() {
    let (_dir, store) = open();
    demo(&store);
    let out_of_scope = |result: Result<_, Error>| matches!(result, Err(Error::OutOfScope { .. }));

    // A component in a file scope.
    let component = ScopeUpdate::new(scope("src/a.rs")).with_nodes(vec![
        Node::new(id("component:c"), NodeKind::Component, "c").with_parent(id("repo:demo")),
    ]);
    assert!(out_of_scope(store.apply(component)));
    // A symbol of another file.
    let mut foreign = file_scope("src/a.rs", &[("f", 1)], &[]);
    if let Some(nodes) = foreign.nodes.as_mut() {
        nodes.push(function("demo::b::x", &id("file:src/b.rs"), 9));
    }
    assert!(out_of_scope(store.apply(foreign)));
    // A symbol in the structure.
    let mut symbol = structure(&["src/a.rs", "src/b.rs"]);
    if let Some(nodes) = symbol.nodes.as_mut() {
        nodes.push(function("demo::loose", &id("file:src/a.rs"), 9));
    }
    assert!(out_of_scope(store.apply(symbol)));
    // An observed edge leaving a node the scope does not provide.
    let foreign_edge = SourceObservations::new(
        Provenance::TreeSitter,
        vec![reference("rust:demo::b::h", "rust:demo::a::f")],
    );
    let result = store.apply(ScopeUpdate::new(scope("src/a.rs")).with_observations(foreign_edge));
    assert!(
        matches!(result, Err(Error::ForeignSource { .. })),
        "{result:?}"
    );
}

/// `src/a.rs` and `src/b.rs` both define `demo::dup`, with different bodies, and each refers to
/// `demo::b::h` from it, with a different weight.
fn dup_in(file: &str, body: u32, weight: u32) -> ScopeUpdate {
    let mut update = file_scope(file, &[("f", 1)], &[]);
    let module = id(&format!(
        "rust:demo::{}",
        file.trim_start_matches("src/").trim_end_matches(".rs")
    ));
    if let Some(nodes) = update.nodes.as_mut() {
        nodes.push(function("demo::dup", &module, body));
    }
    // The content hash must change with dup's body and weight.
    update.content_hash = Some(hash_of((file, body, weight)));
    update.observations = Some(SourceObservations::new(
        Provenance::TreeSitter,
        vec![reference("rust:demo::dup", "rust:demo::b::h").with_weight(weight)],
    ));
    update
}

fn dup_state(store: &Store) -> (Option<Node>, Vec<(String, u32)>) {
    let edges = edges_at(store, head(store))
        .into_iter()
        .filter(|e| e.src.as_str() == "rust:demo::dup")
        .map(|e| (e.dst.to_string(), e.weight))
        .collect();
    (node(store, "rust:demo::dup"), edges)
}

#[test]
fn a_node_two_files_provide_belongs_to_the_smallest_scope_whatever_the_order() {
    let mut heads = Vec::new();
    for a_first in [true, false] {
        let (_dir, store) = open();
        demo(&store);
        let (a, b) = (dup_in("src/a.rs", 1, 2), dup_in("src/b.rs", 2, 5));
        if a_first {
            store.apply(a.clone()).unwrap();
            store.apply(b.clone()).unwrap();
        } else {
            store.apply(b.clone()).unwrap();
            store.apply(a.clone()).unwrap();
        }
        let state = dup_state(&store);
        assert_eq!(
            state.0.as_ref().and_then(|n| n.parent.clone()),
            Some(id("rust:demo::a")),
            "file:src/a.rs owns it"
        );
        assert_eq!(
            state.1,
            [("rust:demo::b::h".to_owned(), 2)],
            "with a.rs's edge"
        );

        // Re-applying either file writes nothing, and a review sees no false move.
        let review = store.freeze(Freeze::ReviewMark).unwrap();
        let before = store.total_changes().unwrap();
        for update in [a.clone(), b.clone(), a.clone(), b.clone()] {
            assert!(store.apply(update).unwrap().is_empty());
        }
        assert_eq!(store.total_changes().unwrap(), before);
        assert!(
            store
                .reader()
                .unwrap()
                .diff(review.seq, head(&store))
                .unwrap()
                .is_empty()
        );

        // a.rs lets go: b.rs's version takes over, node and edge; then b.rs lets go too.
        let stats = store
            .apply(file_scope("src/a.rs", &[("f", 1)], &[]))
            .unwrap();
        assert_eq!(
            (stats.nodes_taken_over, stats.edges_taken_over),
            (1, 1),
            "{stats:?}"
        );
        let state = dup_state(&store);
        assert_eq!(
            state.0.as_ref().and_then(|n| n.parent.clone()),
            Some(id("rust:demo::b"))
        );
        assert_eq!(state.1, [("rust:demo::b::h".to_owned(), 5)]);
        store
            .apply(file_scope("src/b.rs", &[("h", 3)], &[]))
            .unwrap();
        assert_eq!(dup_state(&store), (None, Vec::new()));
        heads.push(nodes_at(&store, head(&store)));
    }
    assert_eq!(heads[0], heads[1]);
}

#[test]
fn a_symbol_moving_between_files_converges_whatever_the_reindexing_order() {
    let move_g = |destination_first: bool| {
        let (_dir, store) = open();
        demo(&store);
        // `g` moves from a.rs to b.rs, keeping its id (for example a method of an impl block).
        let mut destination = file_scope("src/b.rs", &[("h", 3)], &[]);
        if let Some(nodes) = destination.nodes.as_mut() {
            nodes.push(function("demo::a::g", &id("rust:demo::b"), 2));
        }
        let source = file_scope("src/a.rs", &[("f", 1)], &[("f", "rust:demo::b::h")]);
        let (first, second) = if destination_first {
            (destination, source)
        } else {
            (source, destination)
        };
        let stats_first = store.apply(first).unwrap();
        store.apply(second).unwrap();
        (
            node(&store, "rust:demo::a::g").map(|g| g.parent),
            stats_first.nodes_waiting,
        )
    };
    let (parent_when_destination_first, waiting) = move_g(true);
    let (parent_when_source_first, _) = move_g(false);
    assert_eq!(
        parent_when_destination_first,
        Some(Some(id("rust:demo::b")))
    );
    assert_eq!(parent_when_source_first, parent_when_destination_first);
    assert_eq!(
        waiting, 1,
        "b.rs's g waited while a.rs, the smaller scope, still provided it"
    );
}

// ---------------------------------------------------------------------------------------------
// The tree is enforced; a failed batch changes nothing
// ---------------------------------------------------------------------------------------------

/// Components under the repo: `component:outer` holds `pkg:cargo/inner`, which holds
/// `component:sub`; `pkg:cargo/host` is another package.
fn components() -> ScopeUpdate {
    let repo = Node::new(id("repo:demo"), NodeKind::Repo, "demo");
    ScopeUpdate::new(ScopeKey::structure()).with_nodes(vec![
        repo.clone(),
        Node::new(id("component:outer"), NodeKind::Component, "outer").with_parent(repo.id.clone()),
        Node::new(id("pkg:cargo/inner"), NodeKind::Package, "inner")
            .with_parent(id("component:outer")),
        Node::new(id("component:sub"), NodeKind::Component, "sub")
            .with_parent(id("pkg:cargo/inner")),
        Node::new(id("pkg:cargo/host"), NodeKind::Package, "host").with_parent(repo.id),
    ])
}

fn with_parent(mut update: ScopeUpdate, node: &str, parent: &str) -> ScopeUpdate {
    if let Some(nodes) = update.nodes.as_mut() {
        let target = nodes
            .iter_mut()
            .find(|n| n.id.as_str() == node)
            .expect("node in update");
        target.parent = Some(id(parent));
    }
    update
}

#[test]
fn a_batch_breaking_the_tree_is_refused_whole() {
    let (_dir, store) = open();
    demo(&store);
    let before = ids_at(&store, head(&store));

    // A file placed under another file, in a batch whose first update is fine.
    let mut nested = structure(&["src/a.rs", "src/b.rs"]);
    if let Some(nodes) = nested.nodes.as_mut() {
        nodes.push(
            Node::new(id("file:src/c.rs"), NodeKind::File, "c.rs").with_parent(id("file:src/a.rs")),
        );
    }
    let result = store.apply_all(vec![
        file_scope("src/b.rs", &[("h", 3), ("k", 4)], &[]),
        nested,
    ]);
    assert!(
        matches!(
            result,
            Err(Error::Tree(TreeViolation::ForbiddenParent { .. }))
        ),
        "{result:?}"
    );
    assert_eq!(
        ids_at(&store, head(&store)),
        before,
        "the first update of the batch was rolled back too"
    );

    // Removing a file whose symbols are still there.
    let result = store.apply(structure(&["src/b.rs"]));
    assert!(
        matches!(
            result,
            Err(Error::Tree(TreeViolation::OrphanedChild { .. }))
        ),
        "{result:?}"
    );

    // A second repo.
    let mut second = structure(&["src/a.rs", "src/b.rs"]);
    if let Some(nodes) = second.nodes.as_mut() {
        nodes.push(Node::new(id("repo:other"), NodeKind::Repo, "other"));
    }
    let result = store.apply(second);
    assert!(
        matches!(result, Err(Error::Tree(TreeViolation::SecondRepo { .. }))),
        "{result:?}"
    );
    assert_eq!(ids_at(&store, head(&store)), before);
}

#[test]
fn reparenting_into_a_cycle_is_refused() {
    let (_dir, store) = open();
    store.apply(components()).unwrap();
    // outer → inner → sub, then outer under sub: a cycle.
    let result = store.apply(with_parent(
        components(),
        "component:outer",
        "component:sub",
    ));
    assert!(
        matches!(result, Err(Error::Tree(TreeViolation::Cycle { .. }))),
        "{result:?}"
    );
}

#[test]
fn moving_a_component_holding_a_package_into_a_package_is_refused() {
    let (_dir, store) = open();
    store.apply(components()).unwrap();
    // The component itself may sit in a package; the package it holds may not.
    let result = store.apply(with_parent(
        components(),
        "component:outer",
        "pkg:cargo/host",
    ));
    assert!(
        matches!(&result, Err(Error::Tree(TreeViolation::NestedPackage { node, .. })) if node.as_str() == "pkg:cargo/inner"),
        "{result:?}"
    );
}

#[test]
fn ids_must_match_kinds_and_appear_once() {
    let (_dir, store) = open();
    demo(&store);
    let mut update = structure(&["src/a.rs", "src/b.rs"]);
    if let Some(nodes) = update.nodes.as_mut() {
        nodes.push(
            Node::new(id("file:src/c.rs"), NodeKind::Package, "c").with_parent(id("repo:demo")),
        );
    }
    assert!(matches!(
        store.apply(update),
        Err(Error::Model(timonier_core::Error::KindMismatch { .. }))
    ));

    let mut update = file_scope("src/b.rs", &[("h", 3)], &[]);
    if let Some(nodes) = update.nodes.as_mut() {
        let copy = nodes[1].clone();
        nodes.push(copy);
    }
    assert!(matches!(
        store.apply(update),
        Err(Error::DuplicateNode { .. })
    ));
}

#[test]
fn the_incremental_tree_checks_agree_with_a_full_check() {
    use proptest::prelude::*;
    use proptest::test_runner::{Config, TestRunner};

    let mut runner = TestRunner::new(Config::with_cases(48));
    let ids = [
        "component:c0",
        "component:c1",
        "component:c2",
        "pkg:cargo/p0",
        "pkg:cargo/p1",
    ];
    // (operation: 0 reparent, 1 remove, 2 restore; node; new parent, `ids.len()` meaning the repo)
    let strategy = prop::collection::vec((0_u8..3, 0..ids.len(), 0..ids.len() + 1), 1..14);
    runner
        .run(&strategy, |operations| {
            let (_dir, store) = open();
            let repo = Node::new(id("repo:demo"), NodeKind::Repo, "demo");
            let mut nodes: Vec<Node> = ids
                .iter()
                .map(|i| {
                    let kind = if i.starts_with("pkg:") {
                        NodeKind::Package
                    } else {
                        NodeKind::Component
                    };
                    Node::new(id(i), kind, *i).with_parent(repo.id.clone())
                })
                .collect();
            let mut present = [true; 5];
            let full = |nodes: &[Node], present: &[bool; 5]| {
                let mut all = vec![repo.clone()];
                all.extend(
                    nodes
                        .iter()
                        .zip(present)
                        .filter(|(_, p)| **p)
                        .map(|(n, _)| n.clone()),
                );
                all
            };
            store
                .apply(ScopeUpdate::new(ScopeKey::structure()).with_nodes(full(&nodes, &present)))
                .unwrap();
            for (operation, index, parent) in operations {
                let (mut next_nodes, mut next_present) = (nodes.clone(), present);
                match operation {
                    0 => {
                        next_nodes[index].parent = Some(if parent == ids.len() {
                            repo.id.clone()
                        } else {
                            id(ids[parent])
                        })
                    }
                    1 => next_present[index] = false,
                    _ => next_present[index] = true,
                }
                let candidate = full(&next_nodes, &next_present);
                let expected = timonier_core::tree::validate_forest(&candidate).is_ok();
                let accepted = store
                    .apply(ScopeUpdate::new(ScopeKey::structure()).with_nodes(candidate))
                    .is_ok();
                prop_assert_eq!(
                    accepted,
                    expected,
                    "operation {} on {}",
                    operation,
                    ids[index]
                );
                if accepted {
                    (nodes, present) = (next_nodes, next_present);
                }
                let mut stored = nodes_at(&store, head(&store));
                stored.sort_by(|a, b| a.id.cmp(&b.id));
                let mut kept = full(&nodes, &present);
                kept.sort_by(|a, b| a.id.cmp(&b.id));
                prop_assert_eq!(stored, kept, "a refused batch must leave no trace");
            }
            Ok(())
        })
        .unwrap();
}

// ---------------------------------------------------------------------------------------------
// One writer, many readers
// ---------------------------------------------------------------------------------------------

#[test]
fn readers_see_whole_states_while_the_writer_works() {
    let (_dir, store) = open();
    demo(&store);
    let small: Vec<(String, u32)> = (0..5).map(|i| (format!("s{i}"), i)).collect();
    let large: Vec<(String, u32)> = (0..15).map(|i| (format!("s{i}"), i)).collect();

    let writer = {
        let store = store.clone();
        std::thread::spawn(move || {
            for round in 0..40 {
                let list = if round % 2 == 0 { &large } else { &small };
                let functions: Vec<(&str, u32)> =
                    list.iter().map(|(n, b)| (n.as_str(), *b)).collect();
                store
                    .apply(file_scope("src/b.rs", &functions, &[]))
                    .expect("write");
            }
        })
    };
    let readers: Vec<_> = (0..3)
        .map(|_| {
            let store = store.clone();
            std::thread::spawn(move || {
                let mut reader = store.reader().expect("reader");
                for _ in 0..40 {
                    let seq = reader.head().expect("head").seq;
                    let in_b = reader
                        .nodes_at(seq)
                        .expect("nodes")
                        .iter()
                        .filter(|n| {
                            n.parent
                                .as_ref()
                                .is_some_and(|p| p.as_str() == "rust:demo::b")
                        })
                        .count();
                    assert!(
                        [1, 5, 15].contains(&in_b),
                        "a partial state was visible: {in_b} functions"
                    );
                }
            })
        })
        .collect();
    writer.join().expect("writer thread");
    for reader in readers {
        reader.join().expect("reader thread");
    }
}

// ---------------------------------------------------------------------------------------------
// Model-based check of the history: every frozen snapshot keeps exactly its state
// ---------------------------------------------------------------------------------------------

mod history {
    use proptest::prelude::*;

    use super::*;

    #[derive(Clone, Debug)]
    enum Op {
        /// Rewrite file `file`: the functions whose bit is set in `mask`, bodies of `version`,
        /// and file-level calls to the files whose bit is set in `calls`.
        Write {
            file: usize,
            mask: u8,
            version: u32,
            calls: u8,
        },
        Freeze,
    }

    fn ops() -> impl Strategy<Value = Vec<Op>> {
        prop::collection::vec(
            prop_oneof![
                4 => (0_usize..3, any::<u8>(), 0_u32..3, 0_u8..8)
                    .prop_map(|(file, mask, version, calls)| Op::Write { file, mask, version, calls }),
                1 => Just(Op::Freeze),
            ],
            1..25,
        )
    }

    const FILES: [&str; 3] = ["src/a.rs", "src/b.rs", "src/c.rs"];

    /// Function names are unique across files, bodies unique per (name, version): no two
    /// different functions can be mistaken for a move.
    fn update(file: usize, mask: u8, version: u32, calls: u8) -> ScopeUpdate {
        let functions: Vec<(String, u32)> = (0..8_u32)
            .filter(|bit| mask & (1 << bit) != 0)
            .map(|bit| (format!("f{file}_{bit}"), version * 100 + bit))
            .collect();
        let functions: Vec<(&str, u32)> = functions.iter().map(|(n, b)| (n.as_str(), *b)).collect();
        let targets: Vec<&str> = (0..3)
            .filter(|t| *t != file && calls & (1 << t) != 0)
            .map(|t| FILES[t])
            .collect();
        let module = format!("demo::m{file}");
        FileContent {
            path: FILES[file],
            module: &module,
            functions: &functions,
            references: &[],
            calls: &targets,
        }
        .update()
    }

    type State = (Vec<Node>, Vec<Edge>);

    fn state(store: &Store, seq: u64) -> State {
        (nodes_at(store, seq), edges_at(store, seq))
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(24))]

        #[test]
        fn every_frozen_snapshot_keeps_its_state(ops in ops()) {
            let (_dir, store) = open();
            store.apply(structure(&FILES)).unwrap();
            let mut current: BTreeMap<usize, (u8, u32, u8)> = BTreeMap::new();
            let mut points: Vec<(u64, State)> = Vec::new();
            for op in ops {
                match op {
                    Op::Write { file, mask, version, calls } => {
                        store.apply(update(file, mask, version, calls)).unwrap();
                        current.insert(file, (mask, version, calls));
                    }
                    Op::Freeze => {
                        let seq = head(&store);
                        let frozen = state(&store, seq);
                        store.freeze(Freeze::WorkingTree).unwrap();
                        points.push((seq, frozen));
                    }
                }
            }
            // Each frozen snapshot still shows exactly its nodes (all attributes) and edges.
            for (seq, frozen) in &points {
                prop_assert_eq!(&state(&store, *seq), frozen);
            }
            // Diffs between any two points match their states.
            points.push((head(&store), state(&store, head(&store))));
            for (i, (from, (before_nodes, before_edges))) in points.iter().enumerate() {
                for (to, (after_nodes, after_edges)) in &points[i..] {
                    let diff = store.reader().unwrap().diff(*from, *to).unwrap();
                    let before: BTreeMap<&NodeId, &Node> = before_nodes.iter().map(|n| (&n.id, n)).collect();
                    let after: BTreeMap<&NodeId, &Node> = after_nodes.iter().map(|n| (&n.id, n)).collect();
                    let mut expected = BTreeMap::new();
                    for (id, node) in &before {
                        match after.get(id) {
                            None => { expected.insert(id.to_string(), "removed"); }
                            Some(new) if !new.same_version(node) => { expected.insert(id.to_string(), "modified"); }
                            Some(_) => {}
                        }
                    }
                    for id in after.keys().filter(|id| !before.contains_key(*id)) {
                        expected.insert(id.to_string(), "added");
                    }
                    let actual: BTreeMap<String, &str> = summary(&diff.nodes).into_iter().collect();
                    prop_assert_eq!(&actual, &expected, "nodes from {} to {}", from, to);

                    let keys = |edges: &[Edge]| edges.iter().map(Edge::key).collect::<BTreeSet<EdgeKey>>();
                    let (old_keys, new_keys) = (keys(before_edges), keys(after_edges));
                    let mut expected_edges: BTreeMap<EdgeKey, &str> = BTreeMap::new();
                    for key in old_keys.difference(&new_keys) { expected_edges.insert(key.clone(), "removed"); }
                    for key in new_keys.difference(&old_keys) { expected_edges.insert(key.clone(), "added"); }
                    let actual_edges: BTreeMap<EdgeKey, &str> = diff.edges.iter().map(|change| {
                        let what = match change {
                            EdgeChange::Added { .. } => "added",
                            EdgeChange::Removed { .. } => "removed",
                            EdgeChange::Modified { .. } => "modified",
                        };
                        (change.key(), what)
                    }).collect();
                    prop_assert_eq!(&actual_edges, &expected_edges, "edges from {} to {}", from, to);
                }
            }
            // Re-applying the current state writes nothing.
            let before = store.total_changes().unwrap();
            for (file, (mask, version, calls)) in &current {
                prop_assert!(store.apply(update(*file, *mask, *version, *calls)).unwrap().is_empty());
            }
            prop_assert_eq!(store.total_changes().unwrap(), before);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Waiting contributions (third review)
// ---------------------------------------------------------------------------------------------

fn claims_left(store: &Store) -> (i64, i64) {
    let connection = rusqlite::Connection::open(store.path()).expect("inspection connection");
    let count = |table: &str| -> i64 {
        connection
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .expect("count")
    };
    (count("node_claim"), count("edge_claim"))
}

#[test]
fn ownership_passes_down_three_scopes_and_leaves_nothing_behind() {
    let (_dir, store) = open();
    let files = ["src/a.rs", "src/b.rs", "src/c.rs"];
    store
        .apply_all(vec![
            structure(&files),
            file_scope("src/a.rs", &[], &[]),
            file_scope("src/b.rs", &[("h", 3)], &[]),
        ])
        .unwrap();
    // c.rs first, then b.rs, then a.rs: the order does not matter, a.rs (the smallest) owns.
    store.apply(dup_in("src/c.rs", 3, 7)).unwrap();
    store.apply(dup_in("src/b.rs", 2, 5)).unwrap();
    store.apply(dup_in("src/a.rs", 1, 2)).unwrap();
    assert_eq!(
        dup_state(&store).0.and_then(|n| n.parent),
        Some(id("rust:demo::a"))
    );

    // c.rs changes its version while it waits: the head does not move.
    let stats = store.apply(dup_in("src/c.rs", 30, 7)).unwrap();
    assert_eq!(
        (stats.nodes_waiting, stats.nodes_changed),
        (1, 0),
        "{stats:?}"
    );
    assert_eq!(
        dup_state(&store).0.and_then(|n| n.parent),
        Some(id("rust:demo::a"))
    );

    // The owner's scope is removed: b.rs takes over; then b.rs lets go: c.rs, with its latest version.
    store
        .apply_all(vec![
            ScopeUpdate::removal(scope("src/a.rs")),
            structure(&["src/b.rs", "src/c.rs"]),
        ])
        .unwrap();
    assert_eq!(
        dup_state(&store).0.and_then(|n| n.parent),
        Some(id("rust:demo::b"))
    );
    store
        .apply(file_scope("src/b.rs", &[("h", 3)], &[]))
        .unwrap();
    let (dup, edges) = dup_state(&store);
    let dup = dup.expect("c.rs's dup");
    assert_eq!(dup.parent, Some(id("rust:demo::c")));
    assert_eq!(dup.body_hash, Some(hash_of(("dup", 30_u32))));
    assert_eq!(edges, [("rust:demo::b::h".to_owned(), 7)]);

    store.apply(file_scope("src/c.rs", &[], &[])).unwrap();
    assert_eq!(dup_state(&store), (None, Vec::new()));
    assert_eq!(
        claims_left(&store),
        (0, 0),
        "no waiting contribution may leak"
    );
}

#[test]
fn a_file_cannot_leave_the_structure_while_one_of_its_contributions_waits() {
    let (_dir, store) = open();
    demo(&store);
    store.apply(dup_in("src/a.rs", 1, 2)).unwrap();
    // b.rs only provides `dup`, which a.rs owns: it waits under b.rs's module.
    store.apply(dup_in("src/b.rs", 2, 5)).unwrap();
    let result = store.apply(structure(&["src/a.rs"]));
    assert!(
        matches!(
            result,
            Err(Error::Tree(TreeViolation::OrphanedChild { .. }))
        ),
        "{result:?}"
    );
}

#[test]
fn the_most_reliable_version_of_an_edge_wins_over_the_smallest_scope() {
    let (_dir, store) = open();
    demo(&store);
    store.apply(dup_in("src/a.rs", 1, 2)).unwrap();
    let b = dup_in("src/b.rs", 2, 5);
    let b_hash = b.content_hash.unwrap();
    store.apply(b).unwrap();
    // The LSP confirms b.rs's reference: Lsp beats a.rs's tree-sitter, although a.rs < b.rs.
    let answer = lsp(
        vec![reference("rust:demo::dup", "rust:demo::b::h").with_weight(4)],
        &[EdgeKind::References],
    )
    .computed_on(b_hash);
    store
        .apply(ScopeUpdate::new(scope("src/b.rs")).with_observations(answer))
        .unwrap();
    let edge = edges_at(&store, head(&store))
        .into_iter()
        .find(|e| e.src.as_str() == "rust:demo::dup")
        .unwrap();
    assert_eq!((edge.provenance, edge.weight), (Provenance::Lsp, 4));

    // b.rs changes: its LSP answer is stale, its tree-sitter edge no longer beats a.rs's.
    store.apply(dup_in("src/b.rs", 20, 5)).unwrap();
    let edge = edges_at(&store, head(&store))
        .into_iter()
        .find(|e| e.src.as_str() == "rust:demo::dup")
        .unwrap();
    assert_eq!(
        (edge.provenance, edge.weight),
        (Provenance::TreeSitter, 2),
        "back to a.rs's version"
    );
}

#[test]
fn a_removed_file_takes_no_observation_and_a_removal_brings_nothing_else() {
    let (_dir, store) = open();
    demo(&store);
    store
        .apply_all(vec![
            ScopeUpdate::removal(scope("src/a.rs")),
            structure(&["src/b.rs"]),
        ])
        .unwrap();
    // Approximate observations without a content hash, on a file scope without content.
    let late = SourceObservations::new(
        Provenance::TreeSitter,
        vec![reference("rust:demo::a::f", "rust:demo::b::h")],
    );
    let stats = store
        .apply(ScopeUpdate::new(scope("src/a.rs")).with_observations(late))
        .unwrap();
    assert_eq!(stats.stale_observations, 1);
    assert!(edges_at(&store, head(&store)).is_empty());

    let mut removal = ScopeUpdate::removal(scope("src/b.rs"));
    removal.nodes = Some(Vec::new());
    assert!(matches!(
        store.apply(removal),
        Err(Error::RemovalWithContent { .. })
    ));
}

#[test]
fn a_precise_source_brings_no_nodes() {
    let (_dir, store) = open();
    demo(&store);
    let hash = file_scope(
        "src/a.rs",
        &[("f", 1), ("g", 2)],
        &[("f", "rust:demo::b::h")],
    )
    .content_hash
    .unwrap();
    let update = ScopeUpdate::new(scope("src/a.rs"))
        .with_nodes(vec![function("demo::a::extra", &id("file:src/a.rs"), 9)])
        .with_observations(lsp(vec![], &[EdgeKind::References]).computed_on(hash));
    assert!(matches!(
        store.apply(update),
        Err(Error::PreciseSourceSetsContent { .. })
    ));
}

#[test]
fn the_extraction_cache_saves_replaces_and_forgets() {
    let dir = tempfile::tempdir().unwrap();
    let store = timonier_store::Store::open(dir.path().join("graph.sqlite")).unwrap();
    let entry = |path: &str, hash: u8, facts: &str| timonier_store::CachedExtraction {
        path: path.into(),
        language: "rust".into(),
        pack_version: "1.0.0".into(),
        extractor: 1,
        content_hash: timonier_core::ContentHash::from_bytes([hash; 32]),
        facts: facts.into(),
    };
    store
        .save_extractions(
            vec![entry("src/a.rs", 1, "{}"), entry("src/b.rs", 2, "[]")],
            Vec::new(),
        )
        .unwrap();
    store
        .save_extractions(
            vec![entry("src/a.rs", 3, "{\"x\":1}")],
            vec!["src/b.rs".into()],
        )
        .unwrap();
    let reader = store.reader().unwrap();
    assert_eq!(
        reader.extractions().unwrap(),
        [entry("src/a.rs", 3, "{\"x\":1}")]
    );
}

#[test]
fn roles_are_stored_and_a_changed_role_is_a_modification() {
    use timonier_core::{Freeze, Node, NodeChange, NodeId, NodeKind, Role, ScopeKey, ScopeUpdate};
    let dir = tempfile::tempdir().unwrap();
    let store = timonier_store::Store::open(dir.path().join("graph.sqlite")).unwrap();
    let repo = NodeId::repo("r").unwrap();
    let package = NodeId::package("cargo", "store").unwrap();
    let structure = |role: Role| {
        ScopeUpdate::new(ScopeKey::structure()).with_nodes(vec![
            Node::new(repo.clone(), NodeKind::Repo, "r"),
            Node::new(package.clone(), NodeKind::Package, "store")
                .with_parent(repo.clone())
                .with_role(role),
        ])
    };
    store.apply(structure(Role::Utility)).unwrap();
    let mark = store.freeze(Freeze::ReviewMark).unwrap().seq;
    store.apply(structure(Role::Persistence)).unwrap();
    let mut reader = store.reader().unwrap();
    let head = reader.head().unwrap().seq;
    let node = reader.node_at(&package, head).unwrap().unwrap();
    assert_eq!(node.role, Some(Role::Persistence));
    let diff = reader.diff(mark, head).unwrap();
    assert!(
        matches!(&diff.nodes[..], [NodeChange::Modified { before, after }]
        if before.role == Some(Role::Utility) && after.role == Some(Role::Persistence)),
        "{diff:?}"
    );
}

#[test]
fn readers_find_the_innermost_symbol_the_scopes_and_a_source_s_observations() {
    use timonier_core::{
        EdgeKind, Node, NodeId, NodeKind, Observation, Position, Provenance, ScopeKey, ScopeUpdate,
        SourceObservations, Span, SymbolKind,
    };
    let dir = tempfile::tempdir().unwrap();
    let store = timonier_store::Store::open(dir.path().join("graph.sqlite")).unwrap();
    let repo = NodeId::repo("r").unwrap();
    let file = NodeId::file("src/a.rs").unwrap();
    let ty = NodeId::parse("rust:a::T").unwrap();
    let method = NodeId::parse("rust:a::T::m").unwrap();
    let span = |a: (u32, u32), b: (u32, u32)| {
        Span::new(Position::new(a.0, a.1), Position::new(b.0, b.1)).unwrap()
    };
    let hash = timonier_core::ContentHash::from_bytes([7; 32]);
    let scope = ScopeKey::of_file(&file).unwrap();
    store
        .apply_all(vec![
            ScopeUpdate::new(ScopeKey::structure()).with_nodes(vec![
                Node::new(repo.clone(), NodeKind::Repo, "r"),
                Node::new(file.clone(), NodeKind::File, "a.rs").with_parent(repo.clone()),
            ]),
            ScopeUpdate::new(scope.clone())
                .with_content_hash(hash)
                .with_nodes(vec![
                    Node::new(ty.clone(), NodeKind::Symbol(SymbolKind::Type), "T")
                        .with_parent(file.clone())
                        .with_span(span((0, 0), (10, 1))),
                    Node::new(method.clone(), NodeKind::Symbol(SymbolKind::Method), "m")
                        .with_parent(ty.clone())
                        .with_span(span((2, 4), (4, 5))),
                ]),
        ])
        .unwrap();
    store
        .apply(
            ScopeUpdate::new(scope.clone()).with_observations(
                SourceObservations::new(
                    Provenance::Lsp,
                    vec![Observation::new(
                        method.clone(),
                        ty.clone(),
                        EdgeKind::Calls,
                    )],
                )
                .verifying([EdgeKind::Calls])
                .computed_on(hash),
            ),
        )
        .unwrap();
    let reader = store.reader().unwrap();
    assert_eq!(
        reader.symbol_at(&file, Position::new(3, 0)).unwrap(),
        Some(method.clone())
    );
    assert_eq!(
        reader.symbol_at(&file, Position::new(2, 4)).unwrap(),
        Some(method.clone())
    );
    assert_eq!(
        reader.symbol_at(&file, Position::new(5, 0)).unwrap(),
        Some(ty.clone())
    );
    assert_eq!(reader.symbol_at(&file, Position::new(11, 0)).unwrap(), None);
    assert_eq!(
        reader.scopes().unwrap(),
        [scope.clone(), ScopeKey::structure()]
    );
    let lsp = reader.observations(Provenance::Lsp).unwrap();
    assert_eq!(lsp.len(), 1);
    assert_eq!(lsp[0].0, scope);
    assert_eq!(lsp[0].1.verified, [EdgeKind::Calls]);
    assert_eq!(lsp[0].1.observations.len(), 1);
    assert!(reader.observations(Provenance::Scip).unwrap().is_empty());
}
