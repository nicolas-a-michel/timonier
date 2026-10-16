//! Compaction of symbol edges beyond a budget (ADR 0018).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::time::Instant;

use timonier_core::{EdgeKind, NodeId, Position, Provenance, ScopeKey};
use timonier_daemon::{Pipeline, Refinement};
use timonier_index::lsp::{CallTarget, LanguageServer, LspError, Priority};
use timonier_store::Store;

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().expect("a file has a parent")).expect("create dirs");
    std::fs::write(path, text).expect("write file");
}

/// Answers from a table: (file, line of a function name) → callees.
struct Table(BTreeMap<(String, u32), Vec<CallTarget>>);

impl LanguageServer for Table {
    fn outgoing_calls(&mut self, path: &str, at: Position) -> Result<Vec<CallTarget>, LspError> {
        Ok(self
            .0
            .get(&(path.to_owned(), at.line))
            .cloned()
            .unwrap_or_default())
    }
}

fn target(path: &str, line: u32) -> CallTarget {
    CallTarget {
        path: Some(path.into()),
        at: Position::new(line, 7),
        sites: 1,
    }
}

fn verified_calls(store: &Store, file: &str) -> Vec<(String, String)> {
    let mut reader = store.reader().expect("test helper");
    let head = reader.head().expect("test helper").seq;
    let mut calls: Vec<_> = reader
        .edges_at(head)
        .expect("test helper")
        .into_iter()
        .filter(|e| e.kind == EdgeKind::Calls && e.provenance == Provenance::Lsp)
        .filter(|e| e.src.as_str().contains(file))
        .map(|e| (e.src.to_string(), e.dst.to_string()))
        .collect();
    calls.sort();
    calls
}

#[test]
fn the_oldest_verified_files_keep_their_calls_between_files() {
    let repo = tempfile::tempdir().unwrap();
    let db = tempfile::tempdir().unwrap();
    write(repo.path(), "Cargo.toml", "[package]\nname = \"demo\"\n");
    write(
        repo.path(),
        "src/lib.rs",
        "pub mod a;\npub mod b;\npub mod util;\n",
    );
    write(
        repo.path(),
        "src/util.rs",
        "pub fn helper() {}\npub fn helper2() {}\n",
    );
    let mut table = BTreeMap::new();
    for name in ["a", "b"] {
        write(
            repo.path(),
            &format!("src/{name}.rs"),
            &format!(
                "use crate::util::{{helper, helper2}};\npub fn f{name}() {{ helper(); helper2(); }}\n"
            ),
        );
        table.insert(
            (format!("src/{name}.rs"), 1),
            vec![target("src/util.rs", 0), target("src/util.rs", 1)],
        );
    }
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.index_all().unwrap();
    pipeline.set_symbol_edge_budget(3);
    let mut server = Table(table);

    pipeline.request_refinement("src/a.rs", Priority::Opened);
    pipeline.refine_next(&mut server, || false).unwrap();
    assert_eq!(pipeline.symbol_edges(), (2, 0));
    // b makes 4 > 3: a, the oldest, is compacted, down to 90 % of the budget.
    pipeline.request_refinement("src/b.rs", Priority::Opened);
    let refined = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(
        matches!(refined, Refinement::Verified { compacted: 1, .. }),
        "{refined:?}"
    );
    assert_eq!(pipeline.symbol_edges(), (2, 1));
    assert_eq!(
        verified_calls(&store, "src/a.rs"),
        [("file:src/a.rs".into(), "file:src/util.rs".into())]
    );
    assert_eq!(verified_calls(&store, "demo::b").len(), 2);
    // a stays verified: tree-sitter's calls do not come back.
    let reader = store.reader().unwrap();
    let a = ScopeKey::of_file(&NodeId::file("src/a.rs").unwrap()).unwrap();
    assert_eq!(
        reader.verified(&a, Provenance::Lsp).unwrap(),
        [EdgeKind::Calls]
    );

    // Opened again, a gets its detail back, and b is compacted in turn.
    pipeline.request_refinement("src/a.rs", Priority::Opened);
    pipeline.refine_next(&mut server, || false).unwrap();
    assert_eq!(verified_calls(&store, "demo::a").len(), 2);
    assert_eq!(
        verified_calls(&store, "src/b.rs"),
        [("file:src/b.rs".into(), "file:src/util.rs".into())]
    );
}

/// The load test of PLAN.md 3.10, at the real budget: 260 files of 20 functions, each calling
/// 10 functions of other files, so 52 000 symbol → symbol calls. Run it in release:
/// `cargo test --release -p timonier-daemon --test compaction -- --ignored --nocapture`.
#[test]
#[ignore = "load test, run in release"]
fn a_synthetic_load_stays_within_the_budget() {
    const FILES: usize = 260;
    const FUNCTIONS: usize = 20;
    const CALLS: usize = 10;
    let repo = tempfile::tempdir().unwrap();
    let db = tempfile::tempdir().unwrap();
    write(repo.path(), "Cargo.toml", "[package]\nname = \"load\"\n");
    let mut lib = String::new();
    let mut table = BTreeMap::new();
    for f in 0..FILES {
        let _ = writeln!(lib, "pub mod m{f};");
        let mut text = String::new();
        for g in 0..FUNCTIONS {
            let _ = writeln!(text, "pub fn f{g}() {{}}");
            let targets = (1..=CALLS)
                .map(|k| {
                    target(
                        &format!("src/m{}.rs", (f + k) % FILES),
                        ((g + k) % FUNCTIONS) as u32,
                    )
                })
                .collect();
            table.insert((format!("src/m{f}.rs"), g as u32), targets);
        }
        write(repo.path(), &format!("src/m{f}.rs"), &text);
    }
    write(repo.path(), "src/lib.rs", &lib);
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.index_all().unwrap();
    let mut server = Table(table);
    let start = Instant::now();
    let mut compactions = 0;
    for f in 0..FILES {
        pipeline.request_refinement(&format!("src/m{f}.rs"), Priority::Displayed);
        if let Some(Refinement::Verified { compacted, .. }) =
            pipeline.refine_next(&mut server, || false).unwrap()
        {
            compactions += compacted;
        }
    }
    let elapsed = start.elapsed();
    let (detailed, compacted) = pipeline.symbol_edges();
    let mut reader = store.reader().unwrap();
    let head = reader.head().unwrap().seq;
    let symbol_calls = reader
        .edges_at(head)
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == EdgeKind::Calls && !e.src.as_str().starts_with("file:"))
        .count();
    println!(
        "{} calls verified in {:.2} s; {symbol_calls} symbol calls in the head ({detailed} held), {compacted} files compacted in {compactions} compactions",
        FILES * FUNCTIONS * CALLS,
        elapsed.as_secs_f64()
    );
    assert!(
        symbol_calls <= timonier_daemon::indexing::SYMBOL_EDGE_BUDGET,
        "{symbol_calls}"
    );
    assert_eq!(symbol_calls, detailed);
    assert!(compacted > 0);
}
