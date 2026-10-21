//! Speed 2 (ADR 0014, 0022) with a language-server double that answers like the protocol: the
//! call hierarchy of the function whose name is at the given position, each callee pointed at by
//! the start of its name (`selectionRange.start`); the definitions of names; the users of a
//! symbol.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use timonier_core::{EdgeKind, NodeId, Position, Provenance, ScopeKey};
use timonier_daemon::{Event, Pipeline, Refinement, Watch};
use timonier_index::lsp::{CallTarget, Definition, LanguageServer, LspError, Priority, Question};
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
    /// Per (file, position of a name): where it is defined. None: no definitions at all.
    definitions: Option<HashMap<(String, Position), Vec<Definition>>>,
    asked_definitions: Vec<(String, Vec<Position>)>,
    /// The users of any symbol. None: no references at all.
    users: Option<Vec<String>>,
    /// The users of the symbol named at a position, in place of `users`.
    users_at: HashMap<Position, Vec<String>>,
    asked_references: Vec<(String, Position)>,
    /// Its documents are not what is on disk: it answers nothing (ADR 0022).
    unsaved: bool,
    /// The same, from the definitions on: the calls were answered.
    unsaved_definitions: bool,
    /// Another client answers now, once.
    hands_changed: bool,
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
            definitions: None,
            asked_definitions: Vec::new(),
            users: None,
            users_at: HashMap::new(),
            asked_references: Vec::new(),
            unsaved: false,
            unsaved_definitions: false,
            hands_changed: false,
        }
    }
}

impl LanguageServer for Double {
    fn changed_hands(&mut self) -> bool {
        std::mem::take(&mut self.hands_changed)
    }

    fn outgoing_calls(&mut self, path: &str, at: Position) -> Result<Vec<CallTarget>, LspError> {
        if self.unsaved {
            return Err(LspError::Unsaved);
        }
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

    fn definitions(
        &mut self,
        path: &str,
        questions: &[Question],
    ) -> Result<Vec<Vec<Definition>>, LspError> {
        if self.unsaved || self.unsaved_definitions {
            return Err(LspError::Unsaved);
        }
        let Some(table) = &self.definitions else {
            return Err(LspError::Unavailable("no definitions".into()));
        };
        let at: Vec<Position> = questions.iter().map(|q| q.at).collect();
        self.asked_definitions.push((path.to_owned(), at.clone()));
        Ok(at
            .iter()
            .map(|p| {
                table
                    .get(&(path.to_owned(), *p))
                    .cloned()
                    .unwrap_or_default()
            })
            .collect())
    }

    fn references(&mut self, path: &str, at: Position) -> Result<Vec<String>, LspError> {
        if self.unsaved {
            return Err(LspError::Unsaved);
        }
        self.asked_references.push((path.to_owned(), at));
        if let Some(users) = self.users_at.get(&at) {
            return Ok(users.clone());
        }
        self.users
            .clone()
            .ok_or_else(|| LspError::Unavailable("no references".into()))
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
    // node.rs cites nothing of another file but calls: its other usages are verified too,
    // with nothing to ask (ADR 0022).
    let reader = store.reader().unwrap();
    assert_eq!(
        reader.verified(&node_scope(), Provenance::Lsp).unwrap(),
        [
            EdgeKind::Calls,
            EdgeKind::Implements,
            EdgeKind::Inherits,
            EdgeKind::References
        ]
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

    // `helper` changes signature: its users are looked up first (ADR 0022). This server cannot
    // say: node.rs, verified and citing the name, comes back as a dependent.
    write(
        repo.path(),
        "crates/core/src/util.rs",
        "pub fn helper(x: u32) -> u32 {\n    x\n}\n",
    );
    pipeline
        .refresh(&[PathBuf::from("crates/core/src/util.rs")])
        .unwrap();
    assert_eq!(pipeline.refinements_waiting(), 1);
    let looked = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(
        matches!(
            looked,
            Refinement::LookedUp {
                dependents: 1,
                by_name: true,
                ..
            }
        ),
        "{looked:?}"
    );
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

/// What a watch did, in order: for the tests of what it does on its own.
#[derive(Debug)]
enum Step {
    Indexed,
    Refined(Refinement),
}

/// A watch on `pipeline` with the double, whose watcher says nothing for 10 s: longer than the
/// tests. Its steps, once its own first indexing is done.
fn quiet_watch(pipeline: Pipeline) -> (Watch, mpsc::Receiver<Step>) {
    let (send, receive) = mpsc::channel();
    let watch = Watch::start(
        pipeline,
        Duration::from_secs(10),
        Some(Box::new(Double::new())),
        move |event| {
            let step = match event {
                Event::Refined(refined) => Step::Refined(refined.expect("test helper")),
                Event::Indexed(_) => Step::Indexed,
                Event::Committed(_) | Event::Reviewed(_) | Event::Frozen(_) => return,
            };
            let _ = send.send(step);
        },
    )
    .expect("test helper");
    assert!(matches!(
        receive.recv_timeout(Duration::from_secs(20)),
        Ok(Step::Indexed)
    ));
    (watch, receive)
}

/// Review n° 7: the disk differs from the index, and the watcher has not said so yet (here, a
/// long debounce; it may also have missed the change). The watch reads the file again at once,
/// the views learn of it (review n° 8, R4), and the file held is refined: no waiting for the
/// watcher, no question asked again at each release.
#[test]
fn the_watch_reads_a_file_found_stale_at_once() {
    let (repo, db) = repository();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store).unwrap();
    pipeline.index_all().unwrap();
    let (watch, steps) = quiet_watch(pipeline);
    write(
        repo.path(),
        "crates/core/src/util.rs",
        "pub fn helper() -> u32 {\n    1\n}\n\npub fn more() {}\n",
    );
    watch.request_refinement("crates/core/src/node.rs", Priority::Displayed);
    let mut seen = Vec::new();
    loop {
        // Well before the watcher would say anything.
        let step = steps
            .recv_timeout(Duration::from_secs(3))
            .expect("a step within 3 s");
        let verified = matches!(step, Step::Refined(Refinement::Verified { .. }));
        seen.push(step);
        if verified {
            break;
        }
    }
    assert!(
        matches!(
            seen.as_slice(),
            [
                Step::Refined(Refinement::Held { .. }),
                Step::Indexed,
                Step::Refined(Refinement::Verified { calls: 1, .. })
            ]
        ),
        "{seen:?}"
    );
    watch.stop().unwrap();
}

/// Review n° 8, I8-1: the file the answers point into cannot be read. The watch reads it again
/// once, and the file it holds waits: no loop of questions.
#[test]
// File modes: Unix.
#[cfg(unix)]
fn the_watch_does_not_loop_on_a_file_out_of_reach() {
    use std::os::unix::fs::PermissionsExt;
    let (repo, db) = repository();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store).unwrap();
    pipeline.index_all().unwrap();
    let (watch, steps) = quiet_watch(pipeline);
    let util = repo.path().join("crates/core/src/util.rs");
    std::fs::set_permissions(&util, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read(&util).is_ok() {
        // Run as root: nothing is out of reach.
        return;
    }
    watch.request_refinement("crates/core/src/node.rs", Priority::Displayed);
    // At most 50: a loop would never pause.
    let seen: Vec<Step> =
        std::iter::from_fn(|| steps.recv_timeout(Duration::from_millis(1500)).ok())
            .take(50)
            .collect();
    std::fs::set_permissions(&util, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(
        matches!(
            seen.as_slice(),
            [Step::Refined(Refinement::Held { .. }), Step::Indexed]
        ),
        "{seen:?}"
    );
    assert_eq!(watch.refinements_held(), 1);
    watch.stop().unwrap();
}

/// A changed signature, with a server that names the users of `helper` (ADR 0022).
fn changed_helper(users: Vec<String>) -> Refinement {
    let (repo, db) = repository();
    let store = Store::open(db.path().join("graph.sqlite")).expect("test helper");
    let mut pipeline = Pipeline::new(repo.path(), store).expect("test helper");
    pipeline.index_all().expect("test helper");
    let mut server = Double::new();
    server.users = Some(users);
    pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
    pipeline
        .refine_next(&mut server, || false)
        .expect("test helper");
    write(
        repo.path(),
        "crates/core/src/util.rs",
        "pub fn helper(x: u32) -> u32 {\n    x\n}\n",
    );
    pipeline
        .refresh(&[PathBuf::from("crates/core/src/util.rs")])
        .expect("test helper");
    let looked = pipeline
        .refine_next(&mut server, || false)
        .expect("test helper")
        .expect("test helper");
    assert_eq!(pipeline.refinements_waiting(), looked_up(&looked));
    looked
}

fn looked_up(refinement: &Refinement) -> usize {
    match refinement {
        Refinement::LookedUp { dependents, .. } => *dependents,
        other => panic!("not a lookup: {other:?}"),
    }
}

#[test]
fn the_server_names_the_dependents_of_a_changed_signature() {
    // The users the server names come back, and they alone.
    let named = changed_helper(vec!["crates/core/src/node.rs".into()]);
    assert!(matches!(
        named,
        Refinement::LookedUp {
            dependents: 1,
            by_name: false,
            ..
        }
    ));
    // node.rs cites `helper`, but the server names another user: node.rs stays verified.
    let other = changed_helper(vec!["crates/core/src/other.rs".into()]);
    assert!(matches!(
        other,
        Refinement::LookedUp {
            dependents: 0,
            by_name: false,
            ..
        }
    ));
    // No user outside its own file: the server may not know yet. By name, none is missed.
    let none = changed_helper(vec!["crates/core/src/util.rs".into()]);
    assert!(matches!(
        none,
        Refinement::LookedUp {
            dependents: 1,
            by_name: true,
            ..
        }
    ));
}

const SHAPE: &str = "pub trait Shape {}\npub struct Unit;\n";
const OTHER: &str = "pub struct Unit;\n";
const USER: &str = "use crate::shape::{Shape, Unit};\n\npub struct Square;\nimpl Shape for Square {}\npub fn area(a: &Unit, b: &Unit) {}\n";

fn usages_repository() -> (tempfile::TempDir, tempfile::TempDir) {
    let repo = tempfile::tempdir().expect("temp dir");
    for (path, text) in [
        ("Cargo.toml", "[package]\nname = \"demo\"\n"),
        (
            "src/lib.rs",
            "pub mod other;\npub mod shape;\npub mod user;\n",
        ),
        ("src/shape.rs", SHAPE),
        ("src/other.rs", OTHER),
        ("src/user.rs", USER),
    ] {
        write(repo.path(), path, text);
    }
    (repo, tempfile::tempdir().expect("temp dir"))
}

fn edges_at_head(store: &Store, kind: EdgeKind) -> Vec<(String, String, Provenance)> {
    let mut reader = store.reader().expect("test helper");
    let head = reader.head().expect("test helper").seq;
    let mut edges: Vec<_> = reader
        .edges_at(head)
        .expect("test helper")
        .into_iter()
        .filter(|e| e.kind == kind)
        .map(|e| (e.src.to_string(), e.dst.to_string(), e.provenance))
        .collect();
    edges.sort();
    edges
}

/// A server that says `Shape` is in shape.rs, and `Unit` in other.rs.
fn usage_server() -> Double {
    let mut server = Double::new();
    let user = "src/user.rs".to_owned();
    server.definitions = Some(HashMap::from([
        (
            (user.clone(), Position::new(3, 5)),
            vec![Definition {
                path: Some("src/shape.rs".into()),
                at: Position::new(0, 10),
            }],
        ),
        (
            (user, Position::new(4, 16)),
            vec![Definition {
                path: Some("src/other.rs".into()),
                at: Position::new(0, 11),
            }],
        ),
    ]));
    server
}

#[test]
fn usages_are_verified_by_their_definitions() {
    let (repo, db) = usages_repository();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.index_all().unwrap();
    assert_eq!(
        edges_at_head(&store, EdgeKind::References),
        [(
            "file:src/user.rs".into(),
            "file:src/shape.rs".into(),
            Provenance::TreeSitter
        )]
    );
    // The server knows better: this `Unit` is the one of other.rs.
    let mut server = usage_server();
    let user = "src/user.rs".to_owned();
    pipeline.request_refinement("src/user.rs", Priority::Opened);
    let refined = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(
        matches!(refined, Refinement::Verified { usages: 2, .. }),
        "{refined:?}"
    );
    // One batch for the file, one position per path cited, not per site: `Unit` appears twice,
    // `Shape` once.
    assert_eq!(
        server.asked_definitions,
        [(user, vec![Position::new(4, 16), Position::new(3, 5)])]
    );
    // The edge of the server replaces tree-sitter's; it counts the two sites.
    assert_eq!(
        edges_at_head(&store, EdgeKind::References),
        [(
            "file:src/user.rs".into(),
            "file:src/other.rs".into(),
            Provenance::Lsp
        )]
    );
    let mut reader = store.reader().unwrap();
    let head = reader.head().unwrap().seq;
    let weight = reader
        .edges_at(head)
        .unwrap()
        .into_iter()
        .find(|e| e.kind == EdgeKind::References)
        .map(|e| e.weight);
    assert_eq!(weight, Some(2));
    assert_eq!(
        edges_at_head(&store, EdgeKind::Implements),
        [(
            "rust:demo::user::Square".into(),
            "rust:demo::shape::Shape".into(),
            Provenance::Lsp
        )]
    );
}

#[test]
fn without_definitions_the_usages_stay_those_of_tree_sitter() {
    let (repo, db) = usages_repository();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.index_all().unwrap();
    let mut server = Double::new();
    pipeline.request_refinement("src/user.rs", Priority::Opened);
    let refined = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(
        matches!(refined, Refinement::Verified { usages: 0, .. }),
        "{refined:?}"
    );
    let reader = store.reader().unwrap();
    let scope = ScopeKey::of_file(&NodeId::file("src/user.rs").unwrap()).unwrap();
    assert_eq!(
        reader.verified(&scope, Provenance::Lsp).unwrap(),
        [EdgeKind::Calls]
    );
    assert_eq!(
        edges_at_head(&store, EdgeKind::References),
        [(
            "file:src/user.rs".into(),
            "file:src/shape.rs".into(),
            Provenance::TreeSitter
        )]
    );
}

/// Verifies user.rs, then changes the repository with `away` while the daemon is stopped, and
/// starts it again (ADR 0022: a verification whose ends are gone no longer holds).
/// The repository and the database directories go with the last value: they live as long as
/// the test does.
fn restarted_after(
    away: impl FnOnce(&Path),
) -> (Pipeline, Store, (tempfile::TempDir, tempfile::TempDir)) {
    let (repo, db) = usages_repository();
    let open = || Store::open(db.path().join("graph.sqlite")).expect("test helper");
    {
        let mut pipeline = Pipeline::new(repo.path(), open()).expect("test helper");
        pipeline.index_all().expect("test helper");
        pipeline.request_refinement("src/user.rs", Priority::Opened);
        let refined = pipeline
            .refine_next(&mut usage_server(), || false)
            .expect("test helper");
        assert!(matches!(
            refined,
            Some(Refinement::Verified { usages: 2, .. })
        ));
    }
    away(repo.path());
    let store = open();
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).expect("test helper");
    pipeline.index_all().expect("test helper");
    (pipeline, store, (repo, db))
}

fn user_verified(store: &Store) -> Vec<EdgeKind> {
    let scope =
        ScopeKey::of_file(&NodeId::file("src/user.rs").expect("test helper")).expect("test helper");
    store
        .reader()
        .expect("test helper")
        .verified(&scope, Provenance::Lsp)
        .expect("test helper")
}

#[test]
fn verified_usages_hold_across_a_restart() {
    let (mut pipeline, store, _dirs) = restarted_after(|_| {});
    assert_eq!(pipeline.refinements_waiting(), 0);
    assert_eq!(user_verified(&store).len(), 4);
    // Verified whole before the restart: not asked again (review n° 5, m-β).
    pipeline.request_refinement("src/user.rs", Priority::Opened);
    assert_eq!(pipeline.refinements_waiting(), 0);
}

/// Review n° 6, m6-1: a file whose usages stayed tree-sitter's (a doubtful answer) is asked
/// again when a client asks for it, before a restart and after.
#[test]
fn a_file_verified_in_part_is_asked_again_across_a_restart() {
    let (repo, db) = usages_repository();
    let open = || Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut server = usage_server();
    // `Unit` is « defined » at its import: doubtful, the calls alone are verified.
    server.definitions.as_mut().unwrap().insert(
        ("src/user.rs".into(), Position::new(4, 16)),
        vec![Definition {
            path: Some("src/user.rs".into()),
            at: Position::new(0, 26),
        }],
    );
    {
        let mut pipeline = Pipeline::new(repo.path(), open()).unwrap();
        pipeline.index_all().unwrap();
        pipeline.request_refinement("src/user.rs", Priority::Opened);
        pipeline.refine_next(&mut server, || false).unwrap();
        pipeline.request_refinement("src/user.rs", Priority::Opened);
        assert_eq!(pipeline.refinements_waiting(), 1, "asked again");
    }
    let store = open();
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.index_all().unwrap();
    assert_eq!(user_verified(&store), [EdgeKind::Calls]);
    assert_eq!(pipeline.refinements_waiting(), 0);
    pipeline.request_refinement("src/user.rs", Priority::Opened);
    assert_eq!(
        pipeline.refinements_waiting(),
        1,
        "asked again after the restart"
    );
}

#[test]
fn a_usage_whose_target_left_during_a_restart_is_verified_again() {
    let (pipeline, store, _dirs) = restarted_after(|repo| {
        std::fs::remove_file(repo.join("src/other.rs")).unwrap();
        write(repo, "src/lib.rs", "pub mod shape;\npub mod user;\n");
    });
    assert_eq!(pipeline.refinements_waiting(), 1, "user.rs waits again");
    assert!(user_verified(&store).is_empty());
    assert!(
        !edges_at_head(&store, EdgeKind::References)
            .iter()
            .any(|(_, dst, p)| dst == "file:src/other.rs" && *p == Provenance::Lsp)
    );
}

#[test]
fn a_usage_whose_symbols_were_renamed_during_a_restart_is_verified_again() {
    let (pipeline, store, _dirs) = restarted_after(|repo| {
        write(
            repo,
            "Cargo.toml",
            "[package]\nname = \"demo\"\n[lib]\nname = \"renamed\"\n",
        );
    });
    assert_eq!(pipeline.refinements_waiting(), 1, "user.rs waits again");
    assert!(user_verified(&store).is_empty());
    assert!(
        edges_at_head(&store, EdgeKind::Implements)
            .iter()
            .all(|(_, _, p)| *p == Provenance::TreeSitter)
    );
}

/// ADR 0022: a usage tree-sitter places in another file, that the server places in this one —
/// VSCode's TypeScript syntax server answers so while the project loads. Tree-sitter keeps the
/// usages of the file: a right edge is never lost on a doubtful answer.
#[test]
fn a_doubtful_answer_keeps_the_usages_of_tree_sitter() {
    let (repo, db) = usages_repository();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.index_all().unwrap();
    let mut server = usage_server();
    // `Unit` is « defined » at its import, line 0 of user.rs.
    server.definitions.as_mut().unwrap().insert(
        ("src/user.rs".into(), Position::new(4, 16)),
        vec![Definition {
            path: Some("src/user.rs".into()),
            at: Position::new(0, 26),
        }],
    );
    pipeline.request_refinement("src/user.rs", Priority::Opened);
    let refined = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(
        matches!(
            &refined,
            Refinement::Verified {
                usages: 0,
                unverified: Some(_),
                ..
            }
        ),
        "{refined:?}"
    );
    let scope = ScopeKey::of_file(&NodeId::file("src/user.rs").unwrap()).unwrap();
    assert_eq!(
        store
            .reader()
            .unwrap()
            .verified(&scope, Provenance::Lsp)
            .unwrap(),
        [EdgeKind::Calls]
    );
    assert_eq!(
        edges_at_head(&store, EdgeKind::References),
        [(
            "file:src/user.rs".into(),
            "file:src/shape.rs".into(),
            Provenance::TreeSitter
        )]
    );
}

/// Review n° 4, I-A: the file changes again before its lookup is served, and its position no
/// longer holds. The symbol is found again by its id, where it is now; gone, the lookup falls
/// back on the name. No dependent is lost.
#[test]
fn a_lookup_made_stale_by_another_save_finds_its_symbol_again() {
    let (repo, db) = repository();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store).unwrap();
    pipeline.index_all().unwrap();
    let mut server = Double::new();
    // The server names another user: node.rs stays as it is.
    server.users = Some(vec!["crates/core/src/other.rs".into()]);
    pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
    pipeline.refine_next(&mut server, || false).unwrap();
    let util = PathBuf::from("crates/core/src/util.rs");
    let save = |pipeline: &mut Pipeline, text: &str| {
        write(repo.path(), "crates/core/src/util.rs", text);
        pipeline.refresh(std::slice::from_ref(&util)).unwrap();
    };
    save(&mut pipeline, "pub fn helper(x: u32) -> u32 {\n    x\n}\n");
    // Saved again, a line above it: no new lookup, and the first one is out of date.
    save(
        &mut pipeline,
        "// Helpers.\npub fn helper(x: u32) -> u32 {\n    x + 1\n}\n",
    );
    let looked = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(
        matches!(
            looked,
            Refinement::LookedUp {
                dependents: 0,
                by_name: false,
                ..
            }
        ),
        "{looked:?}"
    );
    assert_eq!(
        server.asked_references,
        [("crates/core/src/util.rs".to_owned(), Position::new(1, 7))]
    );

    // Its signature changes, then it is renamed before the lookup is served: by name.
    save(
        &mut pipeline,
        "// Helpers.\npub fn helper(x: u64) -> u32 {\n    1\n}\n",
    );
    save(&mut pipeline, "pub fn assist(x: u64) -> u32 {\n    1\n}\n");
    let mut looked = Vec::new();
    while let Some(step) = pipeline.refine_next(&mut server, || false).unwrap() {
        if let Refinement::LookedUp { by_name, .. } = step {
            looked.push(by_name);
        }
    }
    assert_eq!(looked, [true], "the symbol is gone");
    assert_eq!(server.asked_references.len(), 1);
}

/// Review n° 5, I-β: a second save puts another symbol where the first one was, before the
/// lookup is served. Two symbols, two lookups: each one's dependents come back.
#[test]
fn lookups_of_two_symbols_at_one_position_stay_apart() {
    let (repo, db) = repository();
    write(
        repo.path(),
        "crates/core/src/lib.rs",
        "pub mod node;\npub mod other;\npub mod util;\n",
    );
    write(
        repo.path(),
        "crates/core/src/other.rs",
        "use crate::util::assist;\n\npub fn other() -> u32 {\n    assist()\n}\n",
    );
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store).unwrap();
    pipeline.index_all().unwrap();
    let mut server = Double::new();
    for file in ["crates/core/src/node.rs", "crates/core/src/other.rs"] {
        pipeline.request_refinement(file, Priority::Opened);
        pipeline.refine_next(&mut server, || false).unwrap();
    }
    let util = PathBuf::from("crates/core/src/util.rs");
    // `helper`'s signature changes, at 0:7.
    write(
        repo.path(),
        "crates/core/src/util.rs",
        "pub fn helper(x: u32) -> u32 {\n    x\n}\n",
    );
    pipeline.refresh(std::slice::from_ref(&util)).unwrap();
    // `assist` appears at 0:7; `helper` goes down to 3:7.
    write(
        repo.path(),
        "crates/core/src/util.rs",
        "pub fn assist() -> u32 {\n    2\n}\npub fn helper(x: u32) -> u32 {\n    x\n}\n",
    );
    pipeline.refresh(std::slice::from_ref(&util)).unwrap();
    server
        .users_at
        .insert(Position::new(0, 7), vec!["crates/core/src/other.rs".into()]);
    server
        .users_at
        .insert(Position::new(3, 7), vec!["crates/core/src/node.rs".into()]);
    let mut looked = Vec::new();
    let mut refined = Vec::new();
    while let Some(step) = pipeline.refine_next(&mut server, || false).unwrap() {
        match step {
            Refinement::LookedUp {
                dependents,
                by_name,
                ..
            } => looked.push((dependents, by_name)),
            Refinement::Verified { path, .. } => refined.push(path),
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(looked, [(1, false), (1, false)]);
    assert_eq!(
        server.asked_references,
        [
            ("crates/core/src/util.rs".to_owned(), Position::new(3, 7)),
            ("crates/core/src/util.rs".to_owned(), Position::new(0, 7))
        ]
    );
    refined.sort();
    assert_eq!(
        refined,
        ["crates/core/src/node.rs", "crates/core/src/other.rs"],
        "each symbol's user is verified again"
    );
}

/// Review n° 5, I-α: the client's documents are not what is on disk (unsaved changes). Nothing
/// it answers is recorded: the file, or the lookup, is held apart, and asked again once the
/// client says its documents are saved.
#[test]
fn unsaved_documents_hold_the_work_until_saved() {
    let (repo, db) = repository();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.index_all().unwrap();
    let mut server = Double::new();
    server.unsaved = true;
    pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
    let held = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(
        matches!(&held, Refinement::Held { reason, .. } if reason.contains("not saved")),
        "{held:?}"
    );
    assert_eq!(pipeline.refinements_waiting(), 0, "nothing keeps it busy");
    assert_eq!(pipeline.refinements_held(), 1);
    assert!(
        calls_at_head(&store)
            .iter()
            .all(|(_, _, p)| *p == Provenance::TreeSitter)
    );
    // Asked again by a client, a held file is not verified: it waits.
    pipeline.request_refinement("crates/core/src/node.rs", Priority::Displayed);
    assert_eq!(pipeline.refinements_waiting(), 1);
    pipeline.refine_next(&mut server, || false).unwrap();
    pipeline.release_held();
    server.unsaved = false;
    let refined = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(
        matches!(&refined, Refinement::Verified { calls: 1, .. }),
        "{refined:?}"
    );
    assert_eq!(pipeline.refinements_held(), 0);

    // A lookup the client cannot answer either.
    server.users = Some(vec!["crates/core/src/node.rs".into()]);
    write(
        repo.path(),
        "crates/core/src/util.rs",
        "pub fn helper(x: u32) -> u32 {\n    x\n}\n",
    );
    pipeline
        .refresh(&[PathBuf::from("crates/core/src/util.rs")])
        .unwrap();
    server.unsaved = true;
    let held = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(matches!(held, Refinement::Held { .. }), "{held:?}");
    pipeline.release_held();
    server.unsaved = false;
    let looked = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(
        matches!(
            looked,
            Refinement::LookedUp {
                dependents: 1,
                by_name: false,
                ..
            }
        ),
        "{looked:?}"
    );
}

/// Review n° 6, m6-1: the client answers the calls, then refuses the definitions, its documents
/// changed meanwhile. Nothing is recorded, not even the calls: the file is held.
#[test]
fn definitions_refused_for_unsaved_documents_hold_the_whole_file() {
    let (repo, db) = usages_repository();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.index_all().unwrap();
    let mut server = usage_server();
    server.unsaved_definitions = true;
    pipeline.request_refinement("src/user.rs", Priority::Opened);
    let held = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(matches!(held, Refinement::Held { .. }), "{held:?}");
    assert!(user_verified(&store).is_empty());
    pipeline.release_held();
    server.unsaved_definitions = false;
    let refined = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(
        matches!(refined, Refinement::Verified { usages: 2, .. }),
        "{refined:?}"
    );
}

/// Review n° 6, m6-1: a lookup answered while the disk holds a change the daemon has yet to
/// index: its position was read in the indexed content, the server's in the new one. Held, the
/// lookup is served again after the change.
#[test]
fn a_lookup_answered_about_a_change_not_indexed_yet_waits_for_it() {
    let (repo, db) = repository();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store).unwrap();
    pipeline.index_all().unwrap();
    let mut server = Double::new();
    server.users = Some(vec!["crates/core/src/node.rs".into()]);
    pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
    pipeline.refine_next(&mut server, || false).unwrap();
    let util = PathBuf::from("crates/core/src/util.rs");
    write(
        repo.path(),
        "crates/core/src/util.rs",
        "pub fn helper(x: u32) -> u32 {\n    x\n}\n",
    );
    pipeline.refresh(std::slice::from_ref(&util)).unwrap();
    // Written again, not indexed yet.
    write(
        repo.path(),
        "crates/core/src/util.rs",
        "// Helpers.\npub fn helper(x: u32) -> u32 {\n    x\n}\n",
    );
    let held = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(matches!(held, Refinement::Held { .. }), "{held:?}");
    pipeline.refresh(std::slice::from_ref(&util)).unwrap();
    let looked = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(
        matches!(
            looked,
            Refinement::LookedUp {
                dependents: 1,
                by_name: false,
                ..
            }
        ),
        "{looked:?}"
    );
    assert_eq!(
        server.asked_references.last(),
        Some(&("crates/core/src/util.rs".to_owned(), Position::new(1, 7)))
    );
}

/// Review n° 6, I6-1: an answer that points into a file of the root the daemon does not index
/// (a dependency in `node_modules`, an ignored file) makes no edge, and holds nothing.
#[test]
fn answers_into_files_not_indexed_hold_nothing() {
    let (repo, db) = repository();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.index_all().unwrap();
    let mut server = Double::new();
    server
        .calls
        .get_mut(&("crates/core/src/node.rs".to_owned(), 2))
        .unwrap()
        .push(CallTarget {
            path: Some("node_modules/lib/index.d.ts".into()),
            at: Position::new(0, 24),
            sites: 1,
        });
    pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
    let refined = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(
        matches!(&refined, Refinement::Verified { calls: 1, .. }),
        "{refined:?}"
    );
    assert_eq!(pipeline.refinements_held(), 0);
    assert_eq!(
        calls_at_head(&store),
        [(
            "rust:demo_core::node::make".into(),
            "rust:demo_core::util::helper".into(),
            Provenance::Lsp
        )]
    );
    // A definition there too: `Unit` from a dependency, `Shape` from the repository.
    let (repo, db) = usages_repository();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.index_all().unwrap();
    let mut server = usage_server();
    server.definitions.as_mut().unwrap().insert(
        ("src/user.rs".into(), Position::new(4, 16)),
        vec![Definition {
            path: Some("node_modules/units/index.d.ts".into()),
            at: Position::new(0, 13),
        }],
    );
    pipeline.request_refinement("src/user.rs", Priority::Opened);
    let refined = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(
        matches!(refined, Refinement::Verified { usages: 1, .. }),
        "{refined:?}"
    );
    assert_eq!(pipeline.refinements_held(), 0);
}

/// Review n° 5, I-α: a file changed on disk that the daemon has yet to index. The answers point
/// into it at positions of another content: held, they are asked again after the change.
#[test]
fn answers_about_a_change_not_indexed_yet_wait_for_it() {
    let (repo, db) = repository();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.index_all().unwrap();
    let mut server = Double::new();
    // util.rs gains a line above `helper`: the server already sees it, the daemon not yet.
    write(
        repo.path(),
        "crates/core/src/util.rs",
        "// Helpers.\npub fn helper() -> u32 {\n    1\n}\n",
    );
    pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
    let held = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(
        matches!(&held, Refinement::Held { reason, .. } if reason.contains("disk")),
        "{held:?}"
    );
    assert!(
        calls_at_head(&store)
            .iter()
            .all(|(_, _, p)| *p == Provenance::TreeSitter)
    );
    server.calls.insert(
        ("crates/core/src/node.rs".to_owned(), 2),
        vec![CallTarget {
            path: Some("crates/core/src/util.rs".into()),
            at: Position::new(1, 7),
            sites: 1,
        }],
    );
    // Read again without waiting for the watcher: the change, indexed, releases it. Once.
    assert!(pipeline.reread_unseen().is_some());
    assert!(pipeline.reread_unseen().is_none(), "nothing more to read");
    assert_eq!(pipeline.refinements_held(), 0);
    let refined = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(
        matches!(&refined, Refinement::Verified { calls: 1, .. }),
        "{refined:?}"
    );
    assert_eq!(
        calls_at_head(&store),
        [(
            "rust:demo_core::node::make".into(),
            "rust:demo_core::util::helper".into(),
            Provenance::Lsp
        )]
    );
}

/// Review n° 5, m-β: a client asking again for a file verified whole, unchanged since, costs no
/// question; one whose usages stayed tree-sitter's is asked again.
#[test]
fn a_file_verified_whole_is_not_refined_again() {
    let (repo, db) = repository();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store).unwrap();
    pipeline.index_all().unwrap();
    let mut server = Double::new();
    pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
    pipeline.refine_next(&mut server, || false).unwrap();
    let asked = server.asked.len();
    pipeline.request_refinement("crates/core/src/node.rs", Priority::Displayed);
    assert_eq!(pipeline.refinements_waiting(), 0);
    assert!(
        pipeline
            .refine_next(&mut server, || false)
            .unwrap()
            .is_none()
    );
    assert_eq!(server.asked.len(), asked);
}

/// Review n° 4, m-B: `Unit` moves from other.rs to a new file while the daemon is away; other.rs
/// stays. The edge to other.rs no longer holds: user.rs is verified again.
#[test]
fn a_usage_whose_target_changed_during_a_restart_is_verified_again() {
    let (pipeline, store, _dirs) = restarted_after(|repo| {
        write(repo, "src/other.rs", "pub struct Other;\n");
        write(repo, "src/third.rs", "pub struct Unit;\n");
        write(
            repo,
            "src/lib.rs",
            "pub mod other;\npub mod shape;\npub mod third;\npub mod user;\n",
        );
    });
    assert_eq!(pipeline.refinements_waiting(), 1, "user.rs waits again");
    assert!(user_verified(&store).is_empty());
}

/// Review n° 5, m-β: a package renamed changes the ids, and every LSP verification is dropped,
/// in the other packages too. Every file is placed again, so changed: a file verified whole
/// before is asked again like any other.
#[test]
fn a_file_whose_verification_was_dropped_is_refined_again() {
    let (repo, db) = repository();
    write(
        repo.path(),
        "crates/app/Cargo.toml",
        "[package]\nname = \"demo-app\"\n",
    );
    write(repo.path(), "crates/app/src/lib.rs", "pub fn start() {}\n");
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.index_all().unwrap();
    let mut server = Double::new();
    pipeline.request_refinement("crates/app/src/lib.rs", Priority::Opened);
    pipeline.refine_next(&mut server, || false).unwrap();
    let app = ScopeKey::of_file(&NodeId::file("crates/app/src/lib.rs").unwrap()).unwrap();
    let verified = |store: &Store| {
        store
            .reader()
            .unwrap()
            .verified(&app, Provenance::Lsp)
            .unwrap()
    };
    assert_eq!(verified(&store).len(), 4, "verified whole");
    write(
        repo.path(),
        "crates/core/Cargo.toml",
        "[package]\nname = \"demo-base\"\n",
    );
    pipeline
        .refresh(&[PathBuf::from("crates/core/Cargo.toml")])
        .unwrap();
    assert!(verified(&store).is_empty(), "dropped");
    pipeline.request_refinement("crates/app/src/lib.rs", Priority::Opened);
    assert_eq!(pipeline.refinements_waiting(), 1);
    pipeline.refine_next(&mut server, || false).unwrap();
    assert_eq!(verified(&store).len(), 4);
}

/// Review n° 8, I8-1: a file the answers point into, that the daemon can no longer read. Read
/// again, it stays out of reach: the file it holds is not released, and not asked about again
/// and again. A change elsewhere, or the client, lets it go once.
#[test]
// File modes: Unix.
#[cfg(unix)]
fn a_file_out_of_reach_holds_without_asking_again() {
    use std::os::unix::fs::PermissionsExt;
    let (repo, db) = repository();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store).unwrap();
    pipeline.index_all().unwrap();
    let util = repo.path().join("crates/core/src/util.rs");
    std::fs::set_permissions(&util, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read(&util).is_ok() {
        // Run as root: nothing is out of reach.
        return;
    }
    let mut server = Double::new();
    pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
    let held = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(matches!(held, Refinement::Held { .. }), "{held:?}");
    assert!(pipeline.reread_unseen().is_some());
    assert_eq!(pipeline.refinements_held(), 1, "still held");
    assert_eq!(pipeline.refinements_waiting(), 0);
    assert!(
        pipeline
            .refine_next(&mut server, || false)
            .unwrap()
            .is_none()
    );
    assert_eq!(server.asked.len(), 2, "asked once, its two functions");
    // Readable again, and a change elsewhere: asked again, verified.
    std::fs::set_permissions(&util, std::fs::Permissions::from_mode(0o644)).unwrap();
    write(
        repo.path(),
        "crates/core/src/lib.rs",
        "pub mod node;\npub mod util;\n\n",
    );
    pipeline
        .refresh(&[repo.path().join("crates/core/src/lib.rs")])
        .unwrap();
    let refined = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(
        matches!(refined, Refinement::Verified { calls: 1, .. }),
        "{refined:?}"
    );
}

/// Review n° 8, m8-2: another client answers now. What the previous one refused (unsaved
/// documents) is asked of it, whether or not the server learnt of the change by a nudge.
#[test]
fn held_work_goes_to_the_client_that_answers_next() {
    let (repo, db) = repository();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store).unwrap();
    pipeline.index_all().unwrap();
    let mut server = Double::new();
    server.unsaved = true;
    pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
    pipeline.refine_next(&mut server, || false).unwrap();
    assert_eq!(pipeline.refinements_held(), 1);
    assert!(
        pipeline
            .refine_next(&mut server, || false)
            .unwrap()
            .is_none()
    );
    server.unsaved = false;
    server.hands_changed = true;
    let refined = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(
        matches!(refined, Refinement::Verified { calls: 1, .. }),
        "{refined:?}"
    );
}

/// Review n° 8: a change the daemon never saw, undone before it reads the file again. Nothing
/// changed, but the file matches the index again: what it held goes.
#[test]
fn a_change_undone_before_it_is_read_releases_what_it_held() {
    let (repo, db) = repository();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store).unwrap();
    pipeline.index_all().unwrap();
    let mut server = Double::new();
    write(repo.path(), "crates/core/src/util.rs", "// Helpers.\n");
    pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
    let held = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(matches!(held, Refinement::Held { .. }), "{held:?}");
    write(repo.path(), "crates/core/src/util.rs", UTIL);
    assert!(pipeline.reread_unseen().is_some());
    assert_eq!(pipeline.refinements_held(), 0);
    let refined = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(
        matches!(refined, Refinement::Verified { calls: 1, .. }),
        "{refined:?}"
    );
}

/// Review n° 9, I9-2: the daemon stops with work owed: a lookup not answered (no relay, or a
/// client with unsaved documents), or a dependent not verified again. Started again, it picks
/// the work up: node.rs, which uses the changed `helper`, does not pass for verified.
#[test]
fn work_owed_survives_a_restart() {
    for case in ["lookup waiting", "lookup held", "dependent waiting"] {
        let (repo, db) = repository();
        let open = || Store::open(db.path().join("graph.sqlite")).unwrap();
        {
            let mut pipeline = Pipeline::new(repo.path(), open()).unwrap();
            pipeline.index_all().unwrap();
            let mut server = Double::new();
            server.users = Some(vec!["crates/core/src/node.rs".into()]);
            pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
            pipeline.refine_next(&mut server, || false).unwrap();
            write(
                repo.path(),
                "crates/core/src/util.rs",
                "pub fn helper(x: u32) -> u32 {\n    x\n}\n",
            );
            pipeline
                .refresh(&[PathBuf::from("crates/core/src/util.rs")])
                .unwrap();
            match case {
                "lookup held" => {
                    server.unsaved = true;
                    let held = pipeline.refine_next(&mut server, || false).unwrap();
                    assert!(matches!(held, Some(Refinement::Held { .. })), "{held:?}");
                }
                "dependent waiting" => {
                    let looked = pipeline.refine_next(&mut server, || false).unwrap();
                    assert!(
                        matches!(looked, Some(Refinement::LookedUp { dependents: 1, .. })),
                        "{looked:?}"
                    );
                }
                _ => {}
            }
            // Stopped here, with node.rs still verified in the store.
        }
        let mut pipeline = Pipeline::new(repo.path(), open()).unwrap();
        pipeline.index_all().unwrap();
        assert_eq!(pipeline.refinements_waiting(), 1, "{case}");
        let mut server = Double::new();
        server.users = Some(vec!["crates/core/src/node.rs".into()]);
        let mut verified = Vec::new();
        while let Some(step) = pipeline.refine_next(&mut server, || false).unwrap() {
            if let Refinement::Verified { path, .. } = step {
                verified.push(path);
            }
        }
        assert_eq!(verified, ["crates/core/src/node.rs"], "{case}");
        // Nothing owed any more: a third start has nothing to do.
        drop(pipeline);
        let mut pipeline = Pipeline::new(repo.path(), open()).unwrap();
        pipeline.index_all().unwrap();
        assert_eq!(pipeline.refinements_waiting(), 0, "{case}");
    }
}

/// Review n° 9, m9-2: the file an answer points into is deleted before the daemon sees it. Read
/// again, it is gone from the index: the file held is released, and verified without it.
#[test]
fn a_target_deleted_before_it_is_seen_releases_what_it_held() {
    let (repo, db) = repository();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store).unwrap();
    pipeline.index_all().unwrap();
    let mut server = Double::new();
    std::fs::remove_file(repo.path().join("crates/core/src/util.rs")).unwrap();
    pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
    let held = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(matches!(held, Refinement::Held { .. }), "{held:?}");
    assert!(pipeline.reread_unseen().is_some());
    assert_eq!(pipeline.refinements_held(), 0);
    let refined = pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .unwrap();
    assert!(
        matches!(refined, Refinement::Verified { calls: 0, .. }),
        "{refined:?}"
    );
}

fn git(root: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// Review n° 11, I11-1: `helper` changes signature on disk, and another file is committed
/// before the daemon reads util.rs. The commit snapshot reads it when the working tree comes
/// back: the change is followed there, node.rs comes back as a dependent.
#[test]
fn a_signature_change_read_when_a_commit_comes_back_is_followed() {
    let (repo, db) = repository();
    let root = repo.path();
    git(root, &["init", "--quiet", "--initial-branch=main"]);
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", "first"]);
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(root, store).unwrap();
    pipeline.index_all().unwrap();
    let mut server = Double::new();
    server.users = Some(vec!["crates/core/src/node.rs".into()]);
    pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
    pipeline.refine_next(&mut server, || false).unwrap();
    // Not read by the daemon yet.
    write(
        root,
        "crates/core/src/util.rs",
        "pub fn helper(x: u32) -> u32 {\n    x\n}\n",
    );
    write(
        root,
        "web/src/a.ts",
        "// Changed.\nexport function a() {}\n",
    );
    git(root, &["add", "web/src/a.ts"]);
    git(root, &["commit", "--quiet", "-m", "second"]);
    assert!(
        pipeline.consolidate().unwrap().snapshot.is_some(),
        "a commit snapshot"
    );
    let mut verified = Vec::new();
    while let Some(step) = pipeline.refine_next(&mut server, || false).unwrap() {
        if let Refinement::Verified { path, .. } = step {
            verified.push(path);
        }
    }
    assert_eq!(verified, ["crates/core/src/node.rs"]);
}

/// Review n° 11, m11-2 (N9): the signature change is committed before the daemon reads it. The
/// commit snapshot catches up with what changed since the last one: the change is followed.
#[test]
fn a_signature_change_committed_before_it_is_read_is_followed() {
    let (repo, db) = repository();
    let root = repo.path();
    git(root, &["init", "--quiet", "--initial-branch=main"]);
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", "first"]);
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(root, store).unwrap();
    pipeline.index_all().unwrap();
    let mut server = Double::new();
    server.users = Some(vec!["crates/core/src/node.rs".into()]);
    pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
    pipeline.refine_next(&mut server, || false).unwrap();
    write(
        root,
        "crates/core/src/util.rs",
        "pub fn helper(x: u32) -> u32 {\n    x\n}\n",
    );
    git(root, &["add", "crates/core/src/util.rs"]);
    git(root, &["commit", "--quiet", "-m", "second"]);
    assert!(
        pipeline.consolidate().unwrap().snapshot.is_some(),
        "a commit snapshot"
    );
    let mut verified = Vec::new();
    while let Some(step) = pipeline.refine_next(&mut server, || false).unwrap() {
        if let Refinement::Verified { path, .. } = step {
            verified.push(path);
        }
    }
    assert_eq!(verified, ["crates/core/src/node.rs"]);
}

/// Review n° 10: a verified file changed while the daemon was stopped. Its verification goes
/// with the new content: started again, the daemon does not take it for verified, and asks
/// about it when a client does.
#[test]
fn a_file_changed_while_away_is_asked_again() {
    let (repo, db) = repository();
    let open = || Store::open(db.path().join("graph.sqlite")).unwrap();
    {
        let mut pipeline = Pipeline::new(repo.path(), open()).unwrap();
        pipeline.index_all().unwrap();
        pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
        pipeline.refine_next(&mut Double::new(), || false).unwrap();
    }
    write(
        repo.path(),
        "crates/core/src/node.rs",
        &format!("// Changed while away.\n{NODE}"),
    );
    let store = open();
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.index_all().unwrap();
    assert!(
        calls_at_head(&store)
            .iter()
            .all(|(_, _, p)| *p == Provenance::TreeSitter)
    );
    pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
    assert_eq!(pipeline.refinements_waiting(), 1, "not taken for verified");
}

/// Review n° 11, m11-1: the debt cannot be written (here, a trigger refuses it). The batch that
/// made it is not recorded either: the store never holds the new util.rs without the lookup it
/// owes. Started again, the daemon finds util.rs changed, and node.rs comes back.
#[test]
fn a_debt_that_cannot_be_written_holds_back_its_batch() {
    let (repo, db) = repository();
    let path = db.path().join("graph.sqlite");
    let util = ScopeKey::of_file(&NodeId::file("crates/core/src/util.rs").unwrap()).unwrap();
    {
        let store = Store::open(&path).unwrap();
        let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
        pipeline.index_all().unwrap();
        pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
        pipeline.refine_next(&mut Double::new(), || false).unwrap();
        let before = store.reader().unwrap().content_hash(&util).unwrap();
        let sql = rusqlite::Connection::open(&path).unwrap();
        sql.execute_batch(
            "CREATE TRIGGER no_debt BEFORE INSERT ON refinement_lookup
             BEGIN SELECT RAISE(ABORT, 'no debt'); END;",
        )
        .unwrap();
        write(
            repo.path(),
            "crates/core/src/util.rs",
            "pub fn helper(x: u32) -> u32 {\n    x\n}\n",
        );
        assert!(
            pipeline
                .refresh(&[PathBuf::from("crates/core/src/util.rs")])
                .is_err()
        );
        assert_eq!(
            store.reader().unwrap().content_hash(&util).unwrap(),
            before,
            "the batch went without its debt"
        );
        sql.execute_batch("DROP TRIGGER no_debt;").unwrap();
        // Stopped here.
    }
    let mut pipeline = Pipeline::new(repo.path(), Store::open(&path).unwrap()).unwrap();
    pipeline.index_all().unwrap();
    assert_eq!(pipeline.refinements_waiting(), 1);
    let refined = pipeline
        .refine_next(&mut Double::new(), || false)
        .unwrap()
        .unwrap();
    assert!(
        matches!(&refined, Refinement::Verified { path, .. } if path == "crates/core/src/node.rs"),
        "{refined:?}"
    );
}

/// Review n° 12, I12-1: a commit of another file, while util.rs holds an uncommitted signature
/// change the LSP already followed. The working tree comes back after the commit snapshot as
/// the index held it: nothing changed for the LSP, nothing to look up.
#[test]
fn a_commit_asks_nothing_about_what_the_working_tree_brings_back() {
    let (repo, db) = repository();
    let root = repo.path();
    git(root, &["init", "--quiet", "--initial-branch=main"]);
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", "first"]);
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(root, store.clone()).unwrap();
    pipeline.index_all().unwrap();
    let mut server = Double::new();
    server.users = Some(vec!["crates/core/src/node.rs".into()]);
    pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
    pipeline.refine_next(&mut server, || false).unwrap();
    write(
        root,
        "crates/core/src/util.rs",
        "pub fn helper(x: u32) -> u32 {\n    x\n}\n",
    );
    pipeline
        .refresh(&[root.join("crates/core/src/util.rs")])
        .unwrap();
    // util.rs itself verified, uncommitted.
    pipeline.request_refinement("crates/core/src/util.rs", Priority::Opened);
    while pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .is_some()
    {}
    let asked = (server.asked.len(), server.asked_references.len());
    let util = ScopeKey::of_file(&NodeId::file("crates/core/src/util.rs").unwrap()).unwrap();
    let verified = || {
        store
            .reader()
            .unwrap()
            .verified(&util, Provenance::Lsp)
            .unwrap()
    };
    assert_eq!(verified().len(), 4);
    write(
        root,
        "web/src/a.ts",
        "// Changed.\nexport function a() {}\n",
    );
    git(root, &["add", "web/src/a.ts"]);
    git(root, &["commit", "--quiet", "-m", "second"]);
    assert!(
        pipeline.consolidate().unwrap().snapshot.is_some(),
        "a commit snapshot"
    );
    // util.rs comes back as it was, with its verification: nothing to ask again.
    assert_eq!(pipeline.refinements_waiting(), 0);
    assert_eq!(verified().len(), 4, "its verification came back");
    while pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .is_some()
    {}
    assert_eq!(
        (server.asked.len(), server.asked_references.len()),
        asked,
        "no question for the commit"
    );
    pipeline.request_refinement("crates/core/src/util.rs", Priority::Opened);
    assert_eq!(pipeline.refinements_waiting(), 0, "still verified whole");
}

/// Review n° 12, m12-1 and m12-2: an update fails, and indexing everything again fails too.
/// What it made owed stays owed (other.rs cites the renamed `helper`, and stays verified), and
/// the next refresh brings the store back in line before anything else.
#[test]
fn an_update_that_failed_twice_loses_nothing() {
    let (repo, db) = repository();
    write(
        repo.path(),
        "crates/core/src/lib.rs",
        "pub mod node;\npub mod other;\npub mod util;\n",
    );
    write(
        repo.path(),
        "crates/core/src/other.rs",
        "pub fn helper() {}\n\npub fn run() {\n    helper();\n}\n",
    );
    let path = db.path().join("graph.sqlite");
    let store = Store::open(&path).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.index_all().unwrap();
    let mut server = Double::new();
    for file in ["crates/core/src/node.rs", "crates/core/src/other.rs"] {
        pipeline.request_refinement(file, Priority::Opened);
        pipeline.refine_next(&mut server, || false).unwrap();
    }
    let util = ScopeKey::of_file(&NodeId::file("crates/core/src/util.rs").unwrap()).unwrap();
    let before = store.reader().unwrap().content_hash(&util).unwrap();
    let sql = rusqlite::Connection::open(&path).unwrap();
    sql.execute_batch(
        "CREATE TRIGGER no_debt BEFORE INSERT ON refinement_dependent
         BEGIN SELECT RAISE(ABORT, 'no debt'); END;",
    )
    .unwrap();
    // `helper` renamed: the files citing its name are its dependents.
    write(
        repo.path(),
        "crates/core/src/util.rs",
        "pub fn assist() -> u32 {\n    1\n}\n",
    );
    assert!(
        pipeline
            .refresh(&[PathBuf::from("crates/core/src/util.rs")])
            .is_err()
    );
    assert_eq!(store.reader().unwrap().content_hash(&util).unwrap(), before);
    assert!(pipeline.store_behind());
    sql.execute_batch("DROP TRIGGER no_debt;").unwrap();
    // Any refresh: the store catches up first, once, and says what that changed, for the
    // views (review n° 15, m15-3).
    let report = pipeline.refresh(&[PathBuf::from("web/src/a.ts")]).unwrap();
    assert!(!pipeline.store_behind(), "caught up for good");
    assert!(!report.apply.is_empty(), "{report:?}");
    assert_ne!(
        store.reader().unwrap().content_hash(&util).unwrap(),
        before,
        "the store still lags"
    );
    let owed = store.reader().unwrap().refinement_debt().unwrap();
    assert!(
        owed.dependents
            .contains(&"crates/core/src/other.rs".to_owned()),
        "{owed:?}"
    );
}

/// Review n° 12, m12-3: the debt cannot be written after a refinement step. The step is done
/// all the same, and the debt goes at the next step.
#[test]
fn a_debt_not_written_after_a_step_goes_at_the_next() {
    let (repo, db) = repository();
    let path = db.path().join("graph.sqlite");
    let store = Store::open(&path).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.index_all().unwrap();
    let mut server = Double::new();
    server.users = Some(vec!["crates/core/src/node.rs".into()]);
    pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
    pipeline.refine_next(&mut server, || false).unwrap();
    write(
        repo.path(),
        "crates/core/src/util.rs",
        "pub fn helper(x: u32) -> u32 {\n    x\n}\n",
    );
    pipeline
        .refresh(&[PathBuf::from("crates/core/src/util.rs")])
        .unwrap();
    let sql = rusqlite::Connection::open(&path).unwrap();
    sql.execute_batch(
        "CREATE TRIGGER no_debt BEFORE INSERT ON refinement_dependent
         BEGIN SELECT RAISE(ABORT, 'no debt'); END;",
    )
    .unwrap();
    // The lookup names node.rs: a dependent owed, that cannot be written.
    let looked = pipeline.refine_next(&mut server, || false).unwrap();
    assert!(
        matches!(looked, Some(Refinement::LookedUp { dependents: 1, .. })),
        "{looked:?}"
    );
    let owed = || store.reader().unwrap().refinement_debt().unwrap();
    assert!(owed().dependents.is_empty());
    sql.execute_batch("DROP TRIGGER no_debt;").unwrap();
    // The next step, though it gives way at once.
    let yielded = pipeline.refine_next(&mut server, || true).unwrap();
    assert!(
        matches!(yielded, Some(Refinement::Yielded { .. })),
        "{yielded:?}"
    );
    assert_eq!(owed().dependents, ["crates/core/src/node.rs"]);
}

/// Review n° 12, m12-4: two starts with no refinement step between them (no client relays):
/// the debt read at the first is owed still at the second.
#[test]
fn a_debt_survives_starts_without_a_step() {
    let (repo, db) = repository();
    let open = || Store::open(db.path().join("graph.sqlite")).unwrap();
    {
        let mut pipeline = Pipeline::new(repo.path(), open()).unwrap();
        pipeline.index_all().unwrap();
        pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
        pipeline.refine_next(&mut Double::new(), || false).unwrap();
        write(
            repo.path(),
            "crates/core/src/util.rs",
            "pub fn helper(x: u32) -> u32 {\n    x\n}\n",
        );
        pipeline
            .refresh(&[PathBuf::from("crates/core/src/util.rs")])
            .unwrap();
    }
    for start in 0..2 {
        let mut pipeline = Pipeline::new(repo.path(), open()).unwrap();
        pipeline.index_all().unwrap();
        assert_eq!(pipeline.refinements_waiting(), 1, "start {start}");
    }
}

/// A repository under git, its first commit, node.rs verified with a server that names node.rs
/// as the user of `helper`.
fn committed_repository() -> (
    tempfile::TempDir,
    tempfile::TempDir,
    Pipeline,
    Store,
    Double,
) {
    let (repo, db) = repository();
    let root = repo.path();
    git(root, &["init", "--quiet", "--initial-branch=main"]);
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", "first"]);
    let store = Store::open(db.path().join("graph.sqlite")).expect("test helper");
    let mut pipeline = Pipeline::new(root, store.clone()).expect("test helper");
    pipeline.index_all().expect("test helper");
    let mut server = Double::new();
    server.users = Some(vec!["crates/core/src/node.rs".into()]);
    pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
    pipeline
        .refine_next(&mut server, || false)
        .expect("test helper");
    (repo, db, pipeline, store, server)
}

/// Review n° 13, I13-1: util.rs holds an uncommitted signature change, followed. Before the
/// daemon reads it again, its signature comes back to the committed one, the file still dirty.
/// At the next commit, that change is followed from what the LSP saw, not from the commit:
/// node.rs comes back as a dependent.
#[test]
fn a_signature_brought_back_to_the_commit_is_followed_at_a_commit() {
    let (repo, _db, mut pipeline, _store, mut server) = committed_repository();
    let root = repo.path();
    write(
        root,
        "crates/core/src/util.rs",
        "pub fn helper(x: u32) -> u32 {\n    x\n}\n",
    );
    pipeline
        .refresh(&[root.join("crates/core/src/util.rs")])
        .unwrap();
    while pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .is_some()
    {}
    // Not read by the daemon: the committed signature, in a file still dirty.
    write(
        root,
        "crates/core/src/util.rs",
        "// Still dirty.\npub fn helper() -> u32 {\n    1\n}\n",
    );
    write(
        root,
        "web/src/a.ts",
        "// Changed.\nexport function a() {}\n",
    );
    git(root, &["add", "web/src/a.ts"]);
    git(root, &["commit", "--quiet", "-m", "second"]);
    let lookups = server.asked_references.len();
    assert!(
        pipeline.consolidate().unwrap().snapshot.is_some(),
        "a commit snapshot"
    );
    let mut verified = Vec::new();
    while let Some(step) = pipeline.refine_next(&mut server, || false).unwrap() {
        if let Refinement::Verified { path, .. } = step {
            verified.push(path);
        }
    }
    assert_eq!(
        server.asked_references.len(),
        lookups + 1,
        "helper looked up"
    );
    assert_eq!(verified, ["crates/core/src/node.rs"]);
}

/// Review n° 13, I13-2: a new package, not seen yet when a commit comes, while extra.rs, new
/// and untracked, is verified. Whatever reads the package first, the queue and the store agree
/// on every file: a file the store no longer holds verified is asked again.
#[test]
fn a_package_found_around_a_commit_keeps_the_queue_and_the_store_in_line() {
    let (repo, _db, mut pipeline, store, mut server) = committed_repository();
    let root = repo.path();
    write(root, "crates/core/src/extra.rs", "pub fn extra() {}\n");
    write(
        root,
        "crates/core/src/lib.rs",
        "pub mod extra;\npub mod node;\npub mod util;\n",
    );
    pipeline
        .refresh(&[
            root.join("crates/core/src/extra.rs"),
            root.join("crates/core/src/lib.rs"),
        ])
        .unwrap();
    for file in ["crates/core/src/extra.rs", "crates/core/src/lib.rs"] {
        pipeline.request_refinement(file, Priority::Opened);
    }
    while pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .is_some()
    {}
    // A new crate, not read yet.
    write(
        root,
        "crates/more/Cargo.toml",
        "[package]\nname = \"demo-more\"\n",
    );
    write(root, "crates/more/src/lib.rs", "pub fn more() {}\n");
    write(
        root,
        "web/src/a.ts",
        "// Changed.\nexport function a() {}\n",
    );
    git(root, &["add", "web/src/a.ts"]);
    git(root, &["commit", "--quiet", "-m", "second"]);
    assert!(
        pipeline.consolidate().unwrap().snapshot.is_some(),
        "a commit snapshot"
    );
    // The watcher brings the new crate.
    pipeline
        .refresh(&[
            root.join("crates/more/Cargo.toml"),
            root.join("crates/more/src/lib.rs"),
        ])
        .unwrap();
    // What the queue holds is done first.
    while pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .is_some()
    {}
    for file in [
        "crates/core/src/extra.rs",
        "crates/core/src/lib.rs",
        "crates/core/src/node.rs",
    ] {
        let scope = ScopeKey::of_file(&NodeId::file(file).unwrap()).unwrap();
        let verified = store
            .reader()
            .unwrap()
            .verified(&scope, Provenance::Lsp)
            .unwrap();
        let before = pipeline.refinements_waiting();
        pipeline.request_refinement(file, Priority::Displayed);
        let asked = pipeline.refinements_waiting() > before;
        assert!(
            !verified.is_empty() || asked,
            "{file}: not verified in the store, and not asked again"
        );
    }
}

/// Review n° 13, m13-2: the store misses a batch (util.rs, back to its committed version, fails
/// twice), then an empty commit comes. The commit snapshot is frozen from a store caught up
/// first: it describes the commit.
#[test]
fn a_commit_while_the_store_is_behind_describes_the_commit() {
    let (repo, db, mut pipeline, store, mut server) = committed_repository();
    let root = repo.path();
    let committed = std::fs::read_to_string(root.join("crates/core/src/util.rs")).unwrap();
    write(
        root,
        "crates/core/src/util.rs",
        "pub fn helper(x: u32) -> u32 {\n    x\n}\n",
    );
    pipeline
        .refresh(&[root.join("crates/core/src/util.rs")])
        .unwrap();
    while pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .is_some()
    {}
    let sql = rusqlite::Connection::open(db.path().join("graph.sqlite")).unwrap();
    sql.execute_batch(
        "CREATE TRIGGER no_lookup BEFORE INSERT ON refinement_lookup
         BEGIN SELECT RAISE(ABORT, 'no lookup'); END;",
    )
    .unwrap();
    write(root, "crates/core/src/util.rs", &committed);
    assert!(
        pipeline
            .refresh(&[root.join("crates/core/src/util.rs")])
            .is_err()
    );
    sql.execute_batch("DROP TRIGGER no_lookup;").unwrap();
    git(root, &["commit", "--quiet", "--allow-empty", "-m", "empty"]);
    let snapshot = pipeline
        .consolidate()
        .unwrap()
        .snapshot
        .expect("a commit snapshot");
    let mut reader = store.reader().unwrap();
    let first = reader
        .snapshots()
        .unwrap()
        .into_iter()
        .find(|s| s.kind == timonier_core::SnapshotKind::Commit)
        .expect("the first commit snapshot");
    let mut signature = |seq: u64| {
        reader
            .nodes_at(seq)
            .unwrap()
            .into_iter()
            .find(|n| n.id.as_str().ends_with("util::helper"))
            .and_then(|n| n.signature)
    };
    let committed_signature = signature(first.seq);
    assert!(committed_signature.is_some());
    assert_eq!(signature(snapshot.seq), committed_signature);
}

/// The signature of `helper` and the file ids in a snapshot.
fn frozen(store: &Store, seq: u64) -> (Option<String>, Vec<String>) {
    let mut reader = store.reader().expect("test helper");
    let nodes = reader.nodes_at(seq).expect("test helper");
    let helper = nodes
        .iter()
        .find(|n| n.id.as_str().ends_with("util::helper"))
        .and_then(|n| n.signature.clone());
    let mut files: Vec<String> = nodes
        .iter()
        .map(|n| n.id.to_string())
        .filter(|id| id.starts_with("file:"))
        .collect();
    files.sort();
    (helper, files)
}

/// Review n° 14, I14-1: files back to their committed version before the daemon reads them
/// again, then a commit of another file. Git calls them clean, so the commit snapshot takes them
/// from the index: it reads them first, and describes the commit.
#[test]
fn files_back_to_their_committed_version_unseen_are_frozen_as_committed() {
    let (repo, _db, mut pipeline, store, mut server) = committed_repository();
    let root = repo.path();
    let first = frozen(
        &store,
        store
            .reader()
            .unwrap()
            .latest(timonier_core::SnapshotKind::Commit)
            .unwrap()
            .unwrap()
            .seq,
    );
    let util = std::fs::read_to_string(root.join("crates/core/src/util.rs")).unwrap();
    let a = std::fs::read_to_string(root.join("web/src/a.ts")).unwrap();
    // Seen: a signature changed, a committed file deleted, an untracked file added.
    write(
        root,
        "crates/core/src/util.rs",
        "pub fn helper(x: u32) -> u32 {\n    x\n}\n",
    );
    std::fs::remove_file(root.join("web/src/a.ts")).unwrap();
    write(root, "web/src/new.ts", "export function fresh() {}\n");
    pipeline
        .refresh(&[
            root.join("crates/core/src/util.rs"),
            root.join("web/src/a.ts"),
            root.join("web/src/new.ts"),
        ])
        .unwrap();
    while pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .is_some()
    {}
    // Unseen: each back as committed.
    write(root, "crates/core/src/util.rs", &util);
    write(root, "web/src/a.ts", &a);
    std::fs::remove_file(root.join("web/src/new.ts")).unwrap();
    write(root, "NOTES.md", "A note.\n");
    git(root, &["add", "NOTES.md"]);
    git(root, &["commit", "--quiet", "-m", "second"]);
    let snapshot = pipeline
        .consolidate()
        .unwrap()
        .snapshot
        .expect("a commit snapshot");
    assert_eq!(frozen(&store, snapshot.seq), first);
}

/// Review n° 14, m14-1: the working tree cannot come back as the index held it (a trigger
/// refuses the LSP verification it would put back, node.rs's call). The daemon reads the disk
/// at once instead: the head is the working tree, and the queue and the store agree.
#[test]
fn a_return_that_fails_brings_the_disk_back_at_once() {
    let (repo, db, mut pipeline, store, mut server) = committed_repository();
    let root = repo.path();
    write(
        root,
        "crates/core/src/util.rs",
        "pub fn helper(x: u32) -> u32 {\n    x\n}\n",
    );
    // node.rs dirty too, verified with its call: that verification is what would come back.
    write(
        root,
        "crates/core/src/node.rs",
        &format!("{NODE}// Uncommitted.\n"),
    );
    pipeline
        .refresh(&[
            root.join("crates/core/src/util.rs"),
            root.join("crates/core/src/node.rs"),
        ])
        .unwrap();
    while pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .is_some()
    {}
    let sql = rusqlite::Connection::open(db.path().join("graph.sqlite")).unwrap();
    sql.execute_batch(
        "CREATE TRIGGER no_lsp BEFORE INSERT ON observation WHEN NEW.source = 'lsp'
         BEGIN SELECT RAISE(ABORT, 'no lsp'); END;",
    )
    .unwrap();
    write(root, "NOTES.md", "A note.\n");
    git(root, &["add", "NOTES.md"]);
    git(root, &["commit", "--quiet", "-m", "second"]);
    assert!(
        pipeline.consolidate().unwrap().snapshot.is_some(),
        "a commit snapshot"
    );
    sql.execute_batch("DROP TRIGGER no_lsp;").unwrap();
    assert!(!pipeline.store_behind());
    let head = store.reader().unwrap().head().unwrap().seq;
    let first = store
        .reader()
        .unwrap()
        .snapshots()
        .unwrap()
        .into_iter()
        .find(|s| s.kind == timonier_core::SnapshotKind::Commit)
        .unwrap();
    assert_ne!(
        frozen(&store, head).0,
        frozen(&store, first.seq).0,
        "the head is the working tree"
    );
    while pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .is_some()
    {}
    for file in ["crates/core/src/util.rs", "crates/core/src/node.rs"] {
        let scope = ScopeKey::of_file(&NodeId::file(file).unwrap()).unwrap();
        let verified = store
            .reader()
            .unwrap()
            .verified(&scope, Provenance::Lsp)
            .unwrap();
        let before = pipeline.refinements_waiting();
        pipeline.request_refinement(file, Priority::Displayed);
        assert!(
            !verified.is_empty() || pipeline.refinements_waiting() > before,
            "{file}: not verified in the store, and not asked again"
        );
    }
}

/// The nodes and the edges at the head, and the scopes the store holds.
fn head_state(store: &Store) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut reader = store.reader().expect("test helper");
    let seq = reader.head().expect("test helper").seq;
    let mut nodes: Vec<String> = reader
        .nodes_at(seq)
        .expect("test helper")
        .into_iter()
        .map(|n| format!("{} {:?}", n.id, n.signature))
        .collect();
    nodes.sort();
    let mut edges: Vec<String> = reader
        .edges_at(seq)
        .expect("test helper")
        .into_iter()
        .map(|e| format!("{} -{:?}-> {} {:?}", e.src, e.kind, e.dst, e.provenance))
        .collect();
    edges.sort();
    let mut scopes: Vec<String> = reader
        .scopes()
        .expect("test helper")
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    scopes.sort();
    (nodes, edges, scopes)
}

/// The repository with `committed` written and committed; then `seen`, read by the daemon, and
/// `verified` verified; then a note committed. Whether the head is the same after the commit
/// snapshot, and what waits then.
fn a_commit_around(
    committed: &[(&str, &str)],
    seen: impl FnOnce(&Path) -> Vec<PathBuf>,
    verified: &[&str],
) -> (bool, usize) {
    let (repo, db) = repository();
    let root = repo.path();
    for (path, text) in committed {
        write(root, path, text);
    }
    git(root, &["init", "--quiet", "--initial-branch=main"]);
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", "first"]);
    let store = Store::open(db.path().join("graph.sqlite")).expect("test helper");
    let mut pipeline = Pipeline::new(root, store.clone()).expect("test helper");
    pipeline.index_all().expect("test helper");
    let paths = seen(root);
    pipeline.refresh(&paths).expect("test helper");
    let mut server = Double::new();
    for file in verified {
        pipeline.request_refinement(file, Priority::Opened);
    }
    while pipeline
        .refine_next(&mut server, || false)
        .expect("test helper")
        .is_some()
    {}
    let before = head_state(&store);
    write(root, "NOTES.md", "A note.\n");
    git(root, &["add", "NOTES.md"]);
    git(root, &["commit", "--quiet", "-m", "second"]);
    assert!(
        pipeline
            .consolidate()
            .expect("test helper")
            .snapshot
            .is_some(),
        "a commit snapshot"
    );
    (head_state(&store) == before, pipeline.refinements_waiting())
}

/// Review n° 14, m14-2, and n° 15, m15-2: the freeze brings back a function the working tree
/// removed, that another file finds by its name. The return resolves that file again: the
/// head is as it was.
#[test]
fn a_commit_leaves_a_name_found_elsewhere_as_it_was() {
    let (same, _) = a_commit_around(
        &[
            (
                "crates/core/src/util.rs",
                "pub fn helper() -> u32 {\n    1\n}\n\npub fn extra() {}\n",
            ),
            (
                "crates/core/src/lib.rs",
                "pub mod node;\npub mod other;\npub mod util;\n",
            ),
            (
                "crates/core/src/other.rs",
                "pub fn run() {\n    extra();\n}\n",
            ),
        ],
        |root| {
            write(
                root,
                "crates/core/src/util.rs",
                "pub fn helper() -> u32 {\n    1\n}\n",
            );
            vec![root.join("crates/core/src/util.rs")]
        },
        &[],
    );
    assert!(same);
}

/// Review n° 14, m14-2, and n° 15, m15-2: the freeze brings back a file the working tree
/// deleted, whose function another file cites. The return takes it away again, and resolves
/// that file again: the head is as it was.
#[test]
fn a_commit_leaves_a_deleted_file_deleted() {
    let (same, _) = a_commit_around(
        &[
            (
                "crates/core/src/lib.rs",
                "pub mod gone;\npub mod node;\npub mod other;\npub mod util;\n",
            ),
            ("crates/core/src/gone.rs", "pub fn gone() {}\n"),
            (
                "crates/core/src/other.rs",
                "pub fn run() {\n    gone();\n}\n",
            ),
        ],
        |root| {
            std::fs::remove_file(root.join("crates/core/src/gone.rs")).expect("test helper");
            vec![root.join("crates/core/src/gone.rs")]
        },
        &[],
    );
    assert!(same);
}

/// Review n° 17, m17-4: a commit brings a change the daemon has yet to read, and git cannot say
/// what is dirty. What the commit's diff made the daemon read is told all the same.
#[test]
fn a_commit_git_cannot_read_tells_what_its_diff_changed() {
    let (repo, _db, mut pipeline, store, _server) = committed_repository();
    let root = repo.path();
    write(
        root,
        "crates/core/src/node.rs",
        &format!("{NODE}\npub fn committed() {{}}\n"),
    );
    git(root, &["commit", "--quiet", "-am", "second"]);
    write(root, ".git/index", "not an index");
    let consolidated = pipeline.consolidate().unwrap();
    assert!(consolidated.snapshot.is_none(), "git status fails");
    assert!(
        head_state(&store)
            .0
            .iter()
            .any(|n| n.contains("::committed ")),
        "read from the commit's diff"
    );
    assert!(!consolidated.head.is_empty(), "{consolidated:?}");
}

/// Review n° 18, m18-3: a manifest changed and committed before the watcher brings it, the
/// package renamed. Read again from the commit's diff, the head and the commit snapshot are
/// what a fresh indexing gives.
#[test]
fn a_manifest_committed_unseen_is_read_from_the_commit() {
    let (repo, _db, mut pipeline, store, _server) = committed_repository();
    let root = repo.path();
    write(
        root,
        "crates/core/Cargo.toml",
        "[package]\nname = \"demo-kernel\"\n",
    );
    git(root, &["commit", "--quiet", "-am", "rename"]);
    let seq = pipeline
        .consolidate()
        .unwrap()
        .snapshot
        .expect("a commit snapshot")
        .seq;
    // The components aside: their ids come from clustering, not from the files.
    let ids = |store: &Store, seq: Option<u64>| {
        let mut reader = store.reader().unwrap();
        let seq = seq.unwrap_or_else(|| reader.head().unwrap().seq);
        let mut ids: Vec<String> = reader
            .nodes_at(seq)
            .unwrap()
            .into_iter()
            .map(|n| n.id.to_string())
            .filter(|id| !id.starts_with("component:"))
            .collect();
        ids.sort();
        ids
    };
    let fresh_db = tempfile::tempdir().unwrap();
    let fresh = Store::open(fresh_db.path().join("graph.sqlite")).unwrap();
    Pipeline::new(root, fresh.clone())
        .unwrap()
        .index_all()
        .unwrap();
    let expected = ids(&fresh, None);
    assert!(
        expected.iter().any(|id| id.contains("demo_kernel")),
        "{expected:?}"
    );
    assert_eq!(ids(&store, None), expected, "the head");
    assert_eq!(ids(&store, Some(seq)), expected, "the commit snapshot");
}

/// Review n° 16, m16-2: a deleted file, next to a file changed and verified. The return takes
/// the deleted file away again in its own batch: the store takes that batch, nothing falls back
/// to indexing everything, and the changed file keeps its LSP calls.
#[test]
fn a_commit_keeps_the_verification_next_to_a_deleted_file() {
    let (same, _) = a_commit_around(
        &[
            (
                "crates/core/src/lib.rs",
                "pub mod gone;\npub mod node;\npub mod util;\n",
            ),
            ("crates/core/src/gone.rs", "pub fn gone() {}\n"),
        ],
        |root| {
            std::fs::remove_file(root.join("crates/core/src/gone.rs")).expect("test helper");
            write(
                root,
                "crates/core/src/lib.rs",
                "pub mod node;\npub mod util;\n",
            );
            write(
                root,
                "crates/core/src/node.rs",
                &format!("{NODE}// changed\n"),
            );
            ["gone.rs", "lib.rs", "node.rs"]
                .map(|f| root.join("crates/core/src").join(f))
                .to_vec()
        },
        &["crates/core/src/node.rs"],
    );
    assert!(same);
}

/// Review n° 14, m14-2: the same, other.rs verified. Taking the file away again is no change:
/// other.rs is no dependent.
#[test]
fn a_commit_makes_no_dependent_of_a_file_it_brings_back() {
    let (_, waiting) = a_commit_around(
        &[
            (
                "crates/core/src/lib.rs",
                "pub mod gone;\npub mod node;\npub mod other;\npub mod util;\n",
            ),
            ("crates/core/src/gone.rs", "pub fn gone() {}\n"),
            (
                "crates/core/src/other.rs",
                "pub fn run() {\n    gone();\n}\n",
            ),
        ],
        |root| {
            std::fs::remove_file(root.join("crates/core/src/gone.rs")).expect("test helper");
            vec![root.join("crates/core/src/gone.rs")]
        },
        &["crates/core/src/other.rs"],
    );
    assert_eq!(waiting, 0, "other.rs is no dependent");
}

/// The file ids at the head.
fn files_at_head(store: &Store) -> Vec<String> {
    head_state(store)
        .0
        .into_iter()
        .filter_map(|n| {
            n.strip_prefix("file:")
                .map(|n| n.split(' ').next().unwrap_or(n).to_owned())
        })
        .collect()
}

/// Review n° 15, I15-1: an ignore file comes to skip an indexed file, then no longer does. The
/// file leaves the head, then comes back, without a restart.
#[test]
fn a_file_an_ignore_file_skips_leaves_the_head() {
    let (repo, db) = repository();
    let root = repo.path();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(root, store.clone()).unwrap();
    pipeline.index_all().unwrap();
    write(root, "crates/core/src/gen.rs", "pub fn generated() {}\n");
    pipeline
        .refresh(&[root.join("crates/core/src/gen.rs")])
        .unwrap();
    let gen_rs = "crates/core/src/gen.rs".to_owned();
    assert!(files_at_head(&store).contains(&gen_rs));
    write(root, ".gitignore", "gen.rs\n");
    pipeline.refresh(&[root.join(".gitignore")]).unwrap();
    assert!(!files_at_head(&store).contains(&gen_rs), "skipped now");
    std::fs::remove_file(root.join(".gitignore")).unwrap();
    pipeline.refresh(&[root.join(".gitignore")]).unwrap();
    assert!(files_at_head(&store).contains(&gen_rs), "back");
}

/// Review n° 16, m16-6: `.ignore`, and a `.gitignore` below the root, are ignore files too.
#[test]
fn every_ignore_file_is_followed() {
    let (repo, db) = repository();
    let root = repo.path();
    write(root, "crates/core/src/gen.rs", "pub fn generated() {}\n");
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(root, store.clone()).unwrap();
    pipeline.index_all().unwrap();
    let gen_rs = "crates/core/src/gen.rs".to_owned();
    for (ignore_file, text) in [
        (".ignore", "gen.rs\n"),
        ("crates/core/.gitignore", "src/gen.rs\n"),
    ] {
        write(root, ignore_file, text);
        pipeline.refresh(&[root.join(ignore_file)]).unwrap();
        assert!(!files_at_head(&store).contains(&gen_rs), "{ignore_file}");
        std::fs::remove_file(root.join(ignore_file)).unwrap();
        pipeline.refresh(&[root.join(ignore_file)]).unwrap();
        assert!(
            files_at_head(&store).contains(&gen_rs),
            "{ignore_file} removed"
        );
    }
}

/// Review n° 16, m16-6: a file an ignore file comes to skip goes as a deleted file goes. Twins:
/// util.rs, whose `helper` node.rs calls and the LSP verified, is ignored in one and deleted in
/// the other. The head, the queue and the debt are the same, then what refinement asks.
#[test]
fn an_ignored_file_goes_as_a_deleted_one() {
    let twin = || {
        let (repo, db) = repository();
        let store = Store::open(db.path().join("graph.sqlite")).unwrap();
        let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
        pipeline.index_all().unwrap();
        let mut server = Double::new();
        pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
        pipeline.refine_next(&mut server, || false).unwrap();
        (repo, db, pipeline, store, server)
    };
    let (ignored, _db_a, mut a, store_a, mut server_a) = twin();
    let (deleted, _db_b, mut b, store_b, mut server_b) = twin();
    assert!(
        head_state(&store_a)
            .1
            .iter()
            .any(|e| e.contains("node::make -Calls-> rust:demo_core::util::helper Lsp")),
        "node.rs verified"
    );
    write(ignored.path(), ".gitignore", "crates/core/src/util.rs\n");
    a.refresh(&[ignored.path().join(".gitignore")]).unwrap();
    std::fs::remove_file(deleted.path().join("crates/core/src/util.rs")).unwrap();
    b.refresh(&[deleted.path().join("crates/core/src/util.rs")])
        .unwrap();
    // The repository node is named after its temporary directory.
    let state = |pipeline: &Pipeline, store: &Store| {
        let (mut nodes, edges, scopes) = head_state(store);
        nodes.retain(|n| !n.starts_with("repo:"));
        (
            (nodes, edges, scopes),
            pipeline.refinements_waiting(),
            store.reader().unwrap().refinement_debt().unwrap(),
        )
    };
    assert_eq!(state(&a, &store_a), state(&b, &store_b));
    assert!(a.refinements_waiting() > 0, "node.rs waits again");
    for (pipeline, server) in [(&mut a, &mut server_a), (&mut b, &mut server_b)] {
        while pipeline.refine_next(server, || false).unwrap().is_some() {}
    }
    assert_eq!(state(&a, &store_a), state(&b, &store_b));
    assert_eq!(server_a.asked, server_b.asked);
}

/// Review n° 15, I15-1: git's own ignore file, inside the git directory, skips an indexed file
/// that git does not track; then a commit comes, before the daemon read that ignore file. The
/// commit snapshot and the head leave the file out.
#[test]
fn a_file_git_skips_now_is_left_out_of_a_commit() {
    let (repo, _db, mut pipeline, store, _server) = committed_repository();
    let root = repo.path();
    write(root, "crates/core/src/gen.rs", "pub fn generated() {}\n");
    pipeline
        .refresh(&[root.join("crates/core/src/gen.rs")])
        .unwrap();
    let gen_rs = "crates/core/src/gen.rs".to_owned();
    assert!(files_at_head(&store).contains(&gen_rs));
    write(root, ".git/info/exclude", "gen.rs\n");
    write(root, "NOTES.md", "A note.\n");
    git(root, &["add", "NOTES.md"]);
    git(root, &["commit", "--quiet", "-m", "second"]);
    let snapshot = pipeline
        .consolidate()
        .unwrap()
        .snapshot
        .expect("a commit snapshot");
    assert!(
        !frozen(&store, snapshot.seq)
            .1
            .contains(&format!("file:{gen_rs}"))
    );
    assert!(!files_at_head(&store).contains(&gen_rs));
}

/// Review n° 15, I15-1, with the watcher: `.gitignore` saved, the file leaves the head.
#[test]
fn the_watch_follows_an_ignore_file() {
    let (repo, db) = repository();
    let root = repo.path();
    write(root, "crates/core/src/gen.rs", "pub fn generated() {}\n");
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(root, store.clone()).unwrap();
    pipeline.index_all().unwrap();
    let (indexed, first_indexing) = mpsc::channel();
    let watch = Watch::start(pipeline, Duration::from_millis(100), None, move |event| {
        if let Event::Indexed(_) = event {
            let _ = indexed.send(());
        }
    })
    .unwrap();
    // After the watch's own first indexing, which would read the ignore file by itself.
    first_indexing
        .recv_timeout(Duration::from_secs(20))
        .unwrap();
    let gen_rs = "crates/core/src/gen.rs".to_owned();
    assert!(files_at_head(&store).contains(&gen_rs));
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    write(root, ".gitignore", "gen.rs\n");
    while files_at_head(&store).contains(&gen_rs) {
        assert!(
            std::time::Instant::now() < deadline,
            "gen.rs still at the head"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    watch.stop().unwrap();
}

/// Review n° 15, m15-1: what is on disk cannot be read into the store before a commit snapshot
/// (a trigger refuses `helper`'s new version). No snapshot then, rather than one from a stale
/// index; the next commit gets a right one.
#[test]
fn a_commit_snapshot_waits_for_the_disk() {
    let (repo, db, mut pipeline, store, mut server) = committed_repository();
    let root = repo.path();
    let committed = std::fs::read_to_string(root.join("crates/core/src/util.rs")).unwrap();
    let first = frozen(
        &store,
        store
            .reader()
            .unwrap()
            .latest(timonier_core::SnapshotKind::Commit)
            .unwrap()
            .unwrap()
            .seq,
    );
    write(
        root,
        "crates/core/src/util.rs",
        "pub fn helper(x: u32) -> u32 {\n    x\n}\n",
    );
    pipeline
        .refresh(&[root.join("crates/core/src/util.rs")])
        .unwrap();
    while pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .is_some()
    {}
    // Back as committed, unseen; its new `helper` refused.
    write(root, "crates/core/src/util.rs", &committed);
    let sql = rusqlite::Connection::open(db.path().join("graph.sqlite")).unwrap();
    sql.execute_batch(
        "CREATE TRIGGER no_helper BEFORE INSERT ON node WHEN NEW.id LIKE '%util::helper'
         BEGIN SELECT RAISE(ABORT, 'no helper'); END;
         CREATE TRIGGER no_helper_again BEFORE UPDATE ON node WHEN NEW.id LIKE '%util::helper'
         BEGIN SELECT RAISE(ABORT, 'no helper'); END;",
    )
    .unwrap();
    write(root, "NOTES.md", "A note.\n");
    git(root, &["add", "NOTES.md"]);
    git(root, &["commit", "--quiet", "-m", "second"]);
    assert!(
        pipeline.consolidate().is_err(),
        "no snapshot from a stale index"
    );
    sql.execute_batch("DROP TRIGGER no_helper; DROP TRIGGER no_helper_again;")
        .unwrap();
    let snapshot = pipeline
        .consolidate()
        .unwrap()
        .snapshot
        .expect("a commit snapshot");
    assert_eq!(frozen(&store, snapshot.seq).0, first.0);
}

/// Review n° 15, I15-1, with the watcher: git's own ignore file, in the git directory, saved;
/// the file leaves the head.
#[test]
fn the_watch_follows_git_s_own_ignore_file() {
    let (repo, db) = repository();
    let root = repo.path();
    git(root, &["init", "--quiet", "--initial-branch=main"]);
    write(root, "crates/core/src/gen.rs", "pub fn generated() {}\n");
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(root, store.clone()).unwrap();
    pipeline.index_all().unwrap();
    let gen_rs = "crates/core/src/gen.rs".to_owned();
    assert!(files_at_head(&store).contains(&gen_rs));
    let (indexed, first_indexing) = mpsc::channel();
    let watch = Watch::start(pipeline, Duration::from_millis(100), None, move |event| {
        if let Event::Indexed(_) = event {
            let _ = indexed.send(());
        }
    })
    .unwrap();
    // The watches are in place once the watch's own first indexing is done.
    first_indexing
        .recv_timeout(Duration::from_secs(20))
        .unwrap();
    write(root, ".git/info/exclude", "gen.rs\n");
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    while files_at_head(&store).contains(&gen_rs) {
        assert!(
            std::time::Instant::now() < deadline,
            "gen.rs still at the head"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    watch.stop().unwrap();
}

/// A watch on `pipeline`, once its own first indexing is done: the watches are in place.
fn watch_indexed(pipeline: Pipeline) -> Watch {
    let (indexed, first_indexing) = mpsc::channel();
    let watch = Watch::start(pipeline, Duration::from_millis(100), None, move |event| {
        if let Event::Indexed(_) = event {
            let _ = indexed.send(());
        }
    })
    .expect("test helper");
    first_indexing
        .recv_timeout(Duration::from_secs(20))
        .expect("test helper");
    watch
}

/// Waits until `file` is at the head, or not.
fn wait_for(store: &Store, file: &str, there: bool, why: &str) {
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    while files_at_head(store).iter().any(|f| f == file) != there {
        assert!(std::time::Instant::now() < deadline, "{why}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Review n° 16, m16-4, and n° 17, m17-4: the root is a folder of the git repository, two levels
/// down, as a workspace folder of a monorepo is. Git's own ignore file, and the ignore files of
/// every folder above the root, are followed all the same.
#[test]
fn the_watch_follows_the_ignore_files_above_the_root() {
    let top = tempfile::tempdir().unwrap();
    let root = top.path().join("apps/project");
    write(&root, "Cargo.toml", "[package]\nname = \"demo\"\n");
    write(&root, "src/lib.rs", "pub mod gen;\n");
    write(&root, "src/gen.rs", "pub fn generated() {}\n");
    git(top.path(), &["init", "--quiet", "--initial-branch=main"]);
    let db = tempfile::tempdir().unwrap();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let pipeline = Pipeline::new(&root, store.clone()).unwrap();
    let watch = watch_indexed(pipeline);
    wait_for(&store, "src/gen.rs", true, "gen.rs indexed");
    write(top.path(), ".git/info/exclude", "gen.rs\n");
    wait_for(&store, "src/gen.rs", false, "skipped by info/exclude");
    write(top.path(), ".git/info/exclude", "");
    wait_for(&store, "src/gen.rs", true, "back");
    write(top.path(), ".gitignore", "apps/project/src/gen.rs\n");
    wait_for(
        &store,
        "src/gen.rs",
        false,
        "skipped by the .gitignore at the top",
    );
    std::fs::remove_file(top.path().join(".gitignore")).unwrap();
    wait_for(&store, "src/gen.rs", true, "back again");
    write(top.path(), "apps/.ignore", "project/src/gen.rs\n");
    wait_for(
        &store,
        "src/gen.rs",
        false,
        "skipped by the .ignore of the folder above",
    );
    watch.stop().unwrap();
}

/// Review n° 16, m16-3: the root is a worktree, as an agent's is (spec). Git's own ignore file
/// is the main repository's: the indexing, the watch and the commit snapshot follow it.
#[test]
fn a_worktree_follows_the_main_repository_s_ignore_file() {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main");
    write(&main, "Cargo.toml", "[package]\nname = \"demo\"\n");
    write(&main, "src/lib.rs", "pub fn lib() {}\n");
    git(&main, &["init", "--quiet", "--initial-branch=main"]);
    git(&main, &["add", "."]);
    git(&main, &["commit", "--quiet", "-m", "first"]);
    let root = dir.path().join("wt");
    git(
        &main,
        &["worktree", "add", "--quiet", root.to_str().unwrap()],
    );
    write(&root, "src/gen.rs", "pub fn generated() {}\n");
    write(&root, "src/other.rs", "pub fn other() {}\n");
    write(&main, ".git/info/exclude", "gen.rs\n");
    // Indexed from scratch, then committed.
    let db = tempfile::tempdir().unwrap();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(&root, store.clone()).unwrap();
    pipeline.index_all().unwrap();
    let files = files_at_head(&store);
    assert!(files.contains(&"src/other.rs".to_owned()));
    assert!(!files.contains(&"src/gen.rs".to_owned()), "{files:?}");
    git(&root, &["add", "src/other.rs"]);
    git(&root, &["commit", "--quiet", "-m", "second"]);
    let snapshot = pipeline
        .consolidate()
        .unwrap()
        .snapshot
        .expect("a commit snapshot");
    let frozen = frozen(&store, snapshot.seq).1;
    assert!(
        frozen.contains(&"file:src/other.rs".to_owned()),
        "{frozen:?}"
    );
    assert!(
        !frozen.contains(&"file:src/gen.rs".to_owned()),
        "{frozen:?}"
    );
    // Watched: the ignore file changes.
    let watch = watch_indexed(pipeline);
    write(&main, ".git/info/exclude", "");
    wait_for(&store, "src/gen.rs", true, "no longer skipped");
    write(&main, ".git/info/exclude", "gen.rs\n");
    wait_for(&store, "src/gen.rs", false, "skipped again");
    watch.stop().unwrap();
}

/// Makes the store miss a batch twice: `later`, added to util.rs, cannot be written.
fn store_left_behind(root: &Path, db: &Path, pipeline: &mut Pipeline) {
    let sql = rusqlite::Connection::open(db.join("graph.sqlite")).expect("test helper");
    sql.execute_batch(
        "CREATE TRIGGER no_later BEFORE INSERT ON node WHEN NEW.id LIKE '%::later'
         BEGIN SELECT RAISE(ABORT, 'no later'); END;",
    )
    .expect("test helper");
    write(
        root,
        "crates/core/src/util.rs",
        &format!("{UTIL}\npub fn later() {{}}\n"),
    );
    assert!(
        pipeline
            .refresh(&[root.join("crates/core/src/util.rs")])
            .is_err()
    );
    assert!(pipeline.store_behind());
    sql.execute_batch("DROP TRIGGER no_later;")
        .expect("test helper");
}

fn later_at_head(store: &Store) -> bool {
    head_state(store).0.iter().any(|n| n.contains("::later "))
}

/// ADR 0024: a store behind the indexer catches up before the head is marked as reviewed: the
/// review is the working tree, and the content kept for it too.
#[test]
fn a_review_mark_catches_the_store_up_first() {
    let (repo, db, mut pipeline, store, _server) = committed_repository();
    store_left_behind(repo.path(), db.path(), &mut pipeline);
    let review = pipeline.mark_reviewed().unwrap();
    assert!(!pipeline.store_behind());
    let mut reader = store.reader().unwrap();
    assert!(
        reader
            .nodes_at(review.seq)
            .unwrap()
            .iter()
            .any(|n| n.id.as_str().ends_with("::later")),
        "the review has what the disk has"
    );
    let reviewed = reader.kept_contents(review.seq).unwrap();
    assert_eq!(reviewed, reader.file_contents().unwrap());
    assert!(reviewed.contains_key("file:crates/core/src/util.rs"));
}

/// Review n° 1 of Phase 5, m10: a save the watcher has yet to bring when the head is marked as
/// reviewed is in the review, with its content: it does not show as changed since.
#[test]
fn a_review_mark_takes_what_the_disk_holds() {
    let (repo, _db, mut pipeline, store, _server) = committed_repository();
    let root = repo.path();
    write(
        root,
        "crates/core/src/util.rs",
        &format!("{UTIL}\npub fn saved() {{}}\n"),
    );
    let review = pipeline.mark_reviewed().unwrap();
    let mut reader = store.reader().unwrap();
    assert!(
        reader
            .nodes_at(review.seq)
            .unwrap()
            .iter()
            .any(|n| n.id.as_str().ends_with("::saved")),
        "the save is in the review"
    );
    assert_eq!(
        reader.kept_contents(review.seq).unwrap(),
        reader.file_contents().unwrap()
    );
    // The watcher's late event changes nothing.
    let late = pipeline
        .refresh(&[root.join("crates/core/src/util.rs")])
        .unwrap();
    assert!(late.apply.is_empty(), "{late:?}");
}

/// Review n° 16, m16-1: the store fell behind, and something moves in the git directory with
/// no new commit. The store catches up, and says what that changed, for the views (m16-5).
#[test]
fn a_git_change_without_a_new_commit_catches_the_store_up() {
    let (repo, db, mut pipeline, store, _server) = committed_repository();
    store_left_behind(repo.path(), db.path(), &mut pipeline);
    assert!(!later_at_head(&store));
    let consolidated = pipeline.consolidate().unwrap();
    assert!(consolidated.snapshot.is_none(), "no new commit");
    assert!(!pipeline.store_behind());
    assert!(later_at_head(&store));
    assert!(!consolidated.head.is_empty(), "{consolidated:?}");
}

/// Review n° 16, m16-5: a new commit, and git cannot say what is dirty. No snapshot, but what
/// the consolidation changed in the head on the way is told: a change on disk the watcher had
/// yet to bring.
#[test]
fn a_commit_without_a_snapshot_tells_what_changed_the_head() {
    let (repo, _db, mut pipeline, store, _server) = committed_repository();
    let root = repo.path();
    git(
        root,
        &["commit", "--quiet", "--allow-empty", "-m", "second"],
    );
    write(
        root,
        "crates/core/src/node.rs",
        &format!("{NODE}\npub fn unseen() {{}}\n"),
    );
    write(root, ".git/index", "not an index");
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("status")
        .output()
        .unwrap();
    assert!(!status.status.success(), "git status fails");
    let consolidated = pipeline.consolidate().unwrap();
    assert!(consolidated.snapshot.is_none());
    assert!(
        head_state(&store).0.iter().any(|n| n.contains("::unseen ")),
        "the change on disk"
    );
    assert!(!consolidated.head.is_empty(), "{consolidated:?}");
}

/// Nodes and edges, as text.
type Facts = (Vec<String>, Vec<String>);

/// The graph as the map's facts: the nodes and edges, components aside (their ids come from
/// clustering), and the repository named after its directory.
fn facts_at(store: &Store, seq: Option<u64>) -> Facts {
    let mut reader = store.reader().expect("test helper");
    let seq = seq.unwrap_or_else(|| reader.head().expect("test helper").seq);
    let kept =
        |id: &NodeId| !id.as_str().starts_with("component:") && !id.as_str().starts_with("repo:");
    let mut nodes: Vec<String> = reader
        .nodes_at(seq)
        .expect("test helper")
        .into_iter()
        .filter(|n| kept(&n.id))
        .map(|n| format!("{} {:?} {} {:?}", n.id, n.kind, n.name, n.signature))
        .collect();
    nodes.sort();
    let mut edges: Vec<String> = reader
        .edges_at(seq)
        .expect("test helper")
        .into_iter()
        .filter(|e| kept(&e.src) && kept(&e.dst))
        .map(|e| format!("{} -{:?}-> {} {:?}", e.src, e.kind, e.dst, e.provenance))
        .collect();
    edges.sort();
    (nodes, edges)
}

/// What a fresh indexing of `root` gives.
fn fresh_facts(root: &Path) -> Facts {
    let db = tempfile::tempdir().expect("test helper");
    let store = Store::open(db.path().join("graph.sqlite")).expect("test helper");
    Pipeline::new(root, store.clone())
        .expect("test helper")
        .index_all()
        .expect("test helper");
    facts_at(&store, None)
}

/// What a fresh indexing of the commit HEAD points at gives: a clone of it.
fn committed_facts(root: &Path) -> Facts {
    let clone = tempfile::tempdir().expect("test helper");
    git(
        clone.path(),
        &["clone", "--quiet", root.to_str().expect("utf-8"), "."],
    );
    fresh_facts(clone.path())
}

/// Q23, PLAN.md 5.4: `change` touches the working tree without a commit, then another file is
/// committed. The commit snapshot is the commit, as a fresh indexing of it says; the head is
/// the working tree, as a fresh indexing of it says.
fn a_commit_around_packages(
    committed: &[(&str, Option<&str>)],
    change: &[(&str, Option<&str>)],
) -> (Facts, Facts) {
    let (repo, db) = repository();
    let root = repo.path();
    let apply = |files: &[(&str, Option<&str>)]| {
        for (path, text) in files {
            match text {
                Some(text) => write(root, path, text),
                None => {
                    let _ = std::fs::remove_file(root.join(path));
                }
            }
        }
    };
    apply(committed);
    git(root, &["init", "--quiet", "--initial-branch=main"]);
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", "first"]);
    let store = Store::open(db.path().join("graph.sqlite")).expect("test helper");
    let mut pipeline = Pipeline::new(root, store.clone()).expect("test helper");
    pipeline.index_all().expect("test helper");
    apply(change);
    let paths: Vec<PathBuf> = change.iter().map(|(p, _)| root.join(p)).collect();
    pipeline.refresh(&paths).expect("test helper");
    write(root, "NOTES.md", "A note.\n");
    git(root, &["add", "NOTES.md"]);
    git(root, &["commit", "--quiet", "-m", "second"]);
    let seq = pipeline
        .consolidate()
        .expect("test helper")
        .snapshot
        .expect("a commit snapshot")
        .seq;
    assert_eq!(
        facts_at(&store, Some(seq)),
        committed_facts(root),
        "the commit snapshot"
    );
    assert_eq!(facts_at(&store, None), fresh_facts(root), "the head");
    (facts_at(&store, Some(seq)), facts_at(&store, None))
}

/// Q23, PLAN.md 5.4: the crate renamed in the working tree only. The commit snapshot keeps the
/// commit's name, the head the new one.
#[test]
fn a_commit_snapshot_has_the_commit_s_crate_name() {
    let ((commit, _), (head, _)) = a_commit_around_packages(
        &[],
        &[(
            "crates/core/Cargo.toml",
            Some("[package]\nname = \"demo-kernel\"\n"),
        )],
    );
    let named = |nodes: &[String], name: &str| nodes.iter().any(|n| n.contains(name));
    assert!(named(&commit, "rust:demo_core::util::helper"), "{commit:?}");
    assert!(!named(&commit, "demo_kernel"));
    assert!(named(&head, "rust:demo_kernel::util::helper"), "{head:?}");
}

/// Q23, PLAN.md 5.4: a dependency added in the working tree only.
#[test]
fn a_commit_snapshot_has_the_commit_s_dependencies() {
    let ((commit, commit_edges), (head, head_edges)) = a_commit_around_packages(
        &[],
        &[(
            "crates/core/Cargo.toml",
            Some("[package]\nname = \"demo-core\"\n\n[dependencies]\nserde = \"1\"\n"),
        )],
    );
    let serde = |nodes: &[String]| nodes.iter().any(|n| n.starts_with("pkg:cargo/serde "));
    assert!(!serde(&commit) && serde(&head));
    let depends = |edges: &[String]| {
        edges
            .iter()
            .any(|e| e.starts_with("pkg:cargo/demo-core -DependsOn-> pkg:cargo/serde"))
    };
    assert!(
        !depends(&commit_edges) && depends(&head_edges),
        "{head_edges:?}"
    );
}

/// Q23, PLAN.md 5.4: `src/lib.rs` deleted in the working tree only, then created there only.
/// It decides how `src/main.rs` is named (ADR 0013).
#[test]
fn a_commit_snapshot_has_the_commit_s_library() {
    let main = ("crates/core/src/main.rs", Some("fn main() {}\n"));
    let ((commit, _), (head, _)) =
        a_commit_around_packages(&[main], &[("crates/core/src/lib.rs", None)]);
    let file = |nodes: &[String], path: &str| {
        nodes
            .iter()
            .any(|n| n.starts_with(&format!("file:{path} ")))
    };
    assert!(file(&commit, "crates/core/src/lib.rs") && !file(&head, "crates/core/src/lib.rs"));
    assert_ne!(commit, head);
    let ((commit, _), (head, _)) = a_commit_around_packages(
        &[main, ("crates/core/src/lib.rs", None)],
        &[(
            "crates/core/src/lib.rs",
            Some("pub mod node;\npub mod util;\n"),
        )],
    );
    assert!(!file(&commit, "crates/core/src/lib.rs") && file(&head, "crates/core/src/lib.rs"));
}

/// Q23, PLAN.md 5.4: the crate renamed in the working tree, and node.rs verified since. The
/// commit snapshot has none of that verification, whose ids the commit has not; the head gets
/// it back whole, and nothing is asked again.
#[test]
fn a_commit_with_other_packages_keeps_the_verification() {
    let (repo, db, mut pipeline, store, mut server) = committed_repository();
    let root = repo.path();
    write(
        root,
        "crates/core/Cargo.toml",
        "[package]\nname = \"demo-kernel\"\n",
    );
    pipeline
        .refresh(&[root.join("crates/core/Cargo.toml")])
        .unwrap();
    pipeline.request_refinement("crates/core/src/node.rs", Priority::Opened);
    while pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .is_some()
    {}
    let verified = "rust:demo_kernel::node::make -Calls-> rust:demo_kernel::util::helper Lsp";
    assert!(facts_at(&store, None).1.iter().any(|e| e == verified));
    let asked = server.asked.len();
    write(root, "NOTES.md", "A note.\n");
    git(root, &["add", "NOTES.md"]);
    git(root, &["commit", "--quiet", "-m", "second"]);
    let seq = pipeline
        .consolidate()
        .unwrap()
        .snapshot
        .expect("a commit snapshot")
        .seq;
    assert_eq!(
        facts_at(&store, Some(seq)),
        committed_facts(root),
        "the commit snapshot"
    );
    let (_, edges) = facts_at(&store, None);
    assert!(edges.iter().any(|e| e == verified), "{edges:?}");
    assert_eq!(pipeline.refinements_waiting(), 0);
    while pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .is_some()
    {}
    assert_eq!(server.asked.len(), asked, "nothing asked again");
    drop(db);
}

/// Q23, PLAN.md 5.4: a crate added in the working tree only, not committed yet.
#[test]
fn a_commit_snapshot_has_not_the_crate_added_since() {
    let ((commit, _), (head, _)) = a_commit_around_packages(
        &[],
        &[
            (
                "crates/extra/Cargo.toml",
                Some("[package]\nname = \"demo-extra\"\n"),
            ),
            ("crates/extra/src/lib.rs", Some("pub fn extra() {}\n")),
        ],
    );
    let extra = |nodes: &[String]| nodes.iter().any(|n| n.starts_with("pkg:cargo/demo-extra "));
    assert!(!extra(&commit) && extra(&head), "{head:?}");
}

/// Q23, PLAN.md 5.4: a package deleted in the working tree only: the commit still has it.
#[test]
fn a_commit_snapshot_has_the_package_deleted_since() {
    let ((commit, _), (head, _)) =
        a_commit_around_packages(&[], &[("web/package.json", None), ("web/src/a.ts", None)]);
    let web = |nodes: &[String]| nodes.iter().any(|n| n.starts_with("pkg:npm/web "));
    assert!(web(&commit) && !web(&head), "{commit:?}");
}

/// Review n° 1 of Phase 5, m2: a manifest in a hidden folder, which the walk never enters,
/// changed since the commit: it makes no package of the commit.
#[test]
fn a_commit_snapshot_ignores_a_hidden_manifest() {
    let (commit, head) = a_commit_around_packages(
        &[(".hidden/Cargo.toml", Some("[package]\nname = \"hidden\"\n"))],
        &[(
            ".hidden/Cargo.toml",
            Some("[package]\nname = \"hidden-two\"\n"),
        )],
    );
    assert_eq!(commit, head);
}

/// Review n° 2 of Phase 5, m-7: a manifest git tracks in a folder the walk skips (forced past
/// `.gitignore`), changed since the commit, makes no package of the commit: the commit is
/// frozen under the working tree's packages, its files not placed again, their verification
/// kept (ADR 0025).
#[test]
fn a_commit_snapshot_ignores_a_manifest_the_walk_skips() {
    let (repo, _db, mut pipeline, store, mut server) = committed_repository();
    let root = repo.path();
    while pipeline
        .refine_next(&mut server, || false)
        .unwrap()
        .is_some()
    {}
    let verified = "rust:demo_core::node::make -Calls-> rust:demo_core::util::helper Lsp";
    assert!(facts_at(&store, None).1.iter().any(|e| e == verified));
    write(root, ".gitignore", "vendor/\n");
    write(
        root,
        "vendor/Cargo.toml",
        "[package]\nname = \"vendored\"\n",
    );
    write(root, "vendor/src/lib.rs", "pub fn vendored() {}\n");
    git(root, &["add", ".gitignore"]);
    git(root, &["add", "--force", "vendor"]);
    git(root, &["commit", "--quiet", "-m", "vendor"]);
    pipeline.consolidate().unwrap();
    write(
        root,
        "vendor/Cargo.toml",
        "[package]\nname = \"vendored-two\"\n",
    );
    write(root, "NOTES.md", "A note.\n");
    git(root, &["add", "NOTES.md"]);
    git(root, &["commit", "--quiet", "-m", "second"]);
    let seq = pipeline
        .consolidate()
        .unwrap()
        .snapshot
        .expect("a commit snapshot")
        .seq;
    let (nodes, edges) = facts_at(&store, Some(seq));
    assert!(edges.iter().any(|e| e == verified), "{edges:?}");
    assert!(!nodes.iter().any(|n| n.contains("vendored")), "{nodes:?}");
}

/// Review n° 1 of Phase 5, m2: the crate renamed in the working tree only, the crate large
/// enough to have components. Back from the commit snapshot, its files are in components again,
/// under the working tree's package.
#[test]
fn a_commit_with_other_packages_gives_the_components_back() {
    let (repo, db) = repository();
    let root = repo.path();
    for folder in ["a", "b"] {
        for n in 0..7 {
            write(
                root,
                &format!("crates/core/src/{folder}/f{n}.rs"),
                &format!("pub fn f{n}() {{}}\n"),
            );
        }
    }
    git(root, &["init", "--quiet", "--initial-branch=main"]);
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", "first"]);
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(root, store.clone()).unwrap();
    pipeline.index_all().unwrap();
    write(
        root,
        "crates/core/Cargo.toml",
        "[package]\nname = \"demo-kernel\"\n",
    );
    pipeline
        .refresh(&[root.join("crates/core/Cargo.toml")])
        .unwrap();
    // Each file and the chain of what holds it.
    let placement = |store: &Store, seq: Option<u64>| {
        let mut reader = store.reader().unwrap();
        let seq = seq.unwrap_or_else(|| reader.head().unwrap().seq);
        let nodes = reader.nodes_at(seq).unwrap();
        let parent: HashMap<String, Option<String>> = nodes
            .iter()
            .map(|n| (n.id.to_string(), n.parent.as_ref().map(|p| p.to_string())))
            .collect();
        let mut files: Vec<String> = nodes
            .iter()
            .filter(|n| n.id.as_str().starts_with("file:crates/core/src/a/"))
            .map(|n| {
                let mut chain = vec![n.id.to_string()];
                while let Some(Some(up)) = parent.get(chain.last().expect("one at least")) {
                    chain.push(up.clone());
                }
                chain.join(" < ")
            })
            .collect();
        files.sort();
        files
    };
    let before = placement(&store, None);
    assert!(
        before[0].contains("component:") && before[0].contains("pkg:cargo/demo-kernel"),
        "{before:?}"
    );
    write(root, "NOTES.md", "A note.\n");
    git(root, &["add", "NOTES.md"]);
    git(root, &["commit", "--quiet", "-m", "second"]);
    let seq = pipeline
        .consolidate()
        .unwrap()
        .snapshot
        .expect("a commit snapshot")
        .seq;
    assert!(placement(&store, Some(seq))[0].contains("pkg:cargo/demo-core"));
    assert_eq!(placement(&store, None), before);
}
