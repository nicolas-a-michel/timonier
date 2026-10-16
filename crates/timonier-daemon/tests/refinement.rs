//! Speed 2 (ADR 0014) with a language-server double that answers like the protocol: the call
//! hierarchy of the function whose name is at the given position, each callee pointed at by
//! the start of its name (`selectionRange.start`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use timonier_core::{EdgeKind, NodeId, Position, Provenance, ScopeKey};
use timonier_daemon::{Event, Pipeline, Refinement, Watch};
use timonier_index::lsp::{CallTarget, LanguageServer, LspError, Priority};
use timonier_store::Store;

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().expect("a file has a parent")).expect("create dirs");
    std::fs::write(path, text).expect("write file");
}

const NODE: &str = "use crate::util::helper;\n\npub fn make() -> u32 {\n    helper() + std::mem::size_of::<u8>() as u32\n}\n\npub fn idle() {}\n";
const UTIL: &str = "pub fn helper() -> u32 {\n    1\n}\n";

/// Answers per (file, line of a function name). A position on no known function gets no item,
/// hence no call, as a server does. TypeScript files have no server.
struct Double {
    calls: HashMap<(String, u32), Vec<CallTarget>>,
    asked: Vec<(String, Position)>,
}

impl Double {
    fn new() -> Self {
        let mut calls = HashMap::new();
        calls.insert(
            ("crates/core/src/node.rs".to_owned(), 2),
            vec![
                CallTarget {
                    path: Some("crates/core/src/util.rs".into()),
                    at: Position::new(0, 7),
                    sites: 1,
                },
                // `size_of` lives in the standard library, outside the repository.
                CallTarget {
                    path: None,
                    at: Position::new(400, 12),
                    sites: 1,
                },
            ],
        );
        Self {
            calls,
            asked: Vec::new(),
        }
    }
}

impl LanguageServer for Double {
    fn outgoing_calls(&mut self, path: &str, at: Position) -> Result<Vec<CallTarget>, LspError> {
        if path.ends_with(".ts") {
            return Err(LspError::Unavailable(path.into()));
        }
        self.asked.push((path.to_owned(), at));
        Ok(self
            .calls
            .get(&(path.to_owned(), at.line))
            .cloned()
            .unwrap_or_default())
    }
}

fn repository() -> (tempfile::TempDir, tempfile::TempDir) {
    let repo = tempfile::tempdir().expect("temp dir");
    for (path, text) in [
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
        (
            "crates/core/Cargo.toml",
            "[package]\nname = \"demo-core\"\n",
        ),
        ("crates/core/src/lib.rs", "pub mod node;\npub mod util;\n"),
        ("crates/core/src/node.rs", NODE),
        ("crates/core/src/util.rs", UTIL),
        ("web/package.json", r#"{"name": "web"}"#),
        ("web/src/a.ts", "export function a() {}\n"),
    ] {
        write(repo.path(), path, text);
    }
    (repo, tempfile::tempdir().expect("temp dir"))
}

fn calls_at_head(store: &Store) -> Vec<(String, String, Provenance)> {
    let mut reader = store.reader().expect("test helper");
    let head = reader.head().expect("test helper").seq;
    let mut calls: Vec<_> = reader
        .edges_at(head)
        .expect("test helper")
        .into_iter()
        .filter(|e| e.kind == EdgeKind::Calls)
        .map(|e| (e.src.to_string(), e.dst.to_string(), e.provenance))
        .collect();
    calls.sort();
    calls
}

fn node_scope() -> ScopeKey {
    ScopeKey::of_file(&NodeId::file("crates/core/src/node.rs").expect("test helper"))
        .expect("test helper")
}

#[test]
fn verified_calls_replace_the_tree_sitter_ones_and_go_stale_with_the_content() {
    let (repo, db) = repository();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.index_all().unwrap();
    assert_eq!(
        calls_at_head(&store),
        [(
            "file:crates/core/src/node.rs".into(),
            "file:crates/core/src/util.rs".into(),
            Provenance::TreeSitter
        )]
    );

    let mut server = Double::new();
    pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
    let refined = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(
        matches!(&refined, Refinement::Verified { calls: 1, .. }),
        "{refined:?}"
    );
    // Asked at the start of each function name: `make` (line 2) and `idle` (line 6).
    assert_eq!(
        server.asked,
        [
            ("crates/core/src/node.rs".to_owned(), Position::new(2, 7)),
            ("crates/core/src/node.rs".to_owned(), Position::new(6, 7))
        ]
    );
    // A function → function edge from the LSP; the file → file one from tree-sitter is gone.
    assert_eq!(
        calls_at_head(&store),
        [(
            "rust:demo_core::node::make".into(),
            "rust:demo_core::util::helper".into(),
            Provenance::Lsp
        )]
    );
    let reader = store.reader().unwrap();
    assert_eq!(
        reader.verified(&node_scope(), Provenance::Lsp).unwrap(),
        [EdgeKind::Calls]
    );

    // The file changes: its verification no longer holds, and it waits for refinement again.
    write(
        repo.path(),
        "crates/core/src/node.rs",
        &NODE.replace("pub fn idle() {}", "pub fn idle() { helper(); }"),
    );
    pipeline
        .refresh(&[PathBuf::from("crates/core/src/node.rs")])
        .unwrap();
    assert_eq!(pipeline.refinements_waiting(), 1);
    assert!(
        reader
            .verified(&node_scope(), Provenance::Lsp)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        calls_at_head(&store),
        [(
            "file:crates/core/src/node.rs".into(),
            "file:crates/core/src/util.rs".into(),
            Provenance::TreeSitter
        )]
    );
}

#[test]
fn a_changed_signature_sends_verified_dependents_back() {
    let (repo, db) = repository();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.index_all().unwrap();
    let mut server = Double::new();
    pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
    pipeline.refine_next(&mut server, || false).unwrap();
    assert_eq!(pipeline.refinements_waiting(), 0);

    // `helper` changes signature: node.rs, verified and citing it, comes back as a dependent.
    write(
        repo.path(),
        "crates/core/src/util.rs",
        "pub fn helper(x: u32) -> u32 {\n    x\n}\n",
    );
    pipeline
        .refresh(&[PathBuf::from("crates/core/src/util.rs")])
        .unwrap();
    assert_eq!(pipeline.refinements_waiting(), 1);

    // Yielding leaves the job waiting, untouched.
    let yielded = pipeline.refine_next(&mut server, || true).unwrap().unwrap();
    assert!(matches!(yielded, Refinement::Yielded { .. }));
    assert_eq!(pipeline.refinements_waiting(), 1);

    // A file without server is skipped, not an error.
    pipeline.request_refinement("web/src/a.ts", Priority::Displayed);
    let skipped = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(matches!(skipped, Refinement::Skipped { .. }), "{skipped:?}");
}

#[test]
fn the_watch_refines_when_idle() {
    let (repo, db) = repository();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.index_all().unwrap();
    let (send, receive) = mpsc::channel();
    let watch = Watch::start(
        pipeline,
        Duration::from_millis(100),
        Some(Box::new(Double::new())),
        move |event| {
            if let Event::Refined(refined) = event {
                let _ = send.send(refined);
            }
        },
    )
    .unwrap();
    watch.request_refinement("crates/core/src/node.rs", Priority::Displayed);
    let refined = receive
        .recv_timeout(Duration::from_secs(20))
        .unwrap()
        .unwrap();
    assert!(
        matches!(refined, Refinement::Verified { calls: 1, .. }),
        "{refined:?}"
    );
    assert!(
        calls_at_head(&store)
            .iter()
            .any(|(_, _, p)| *p == Provenance::Lsp)
    );
    watch.stop().unwrap();
}
