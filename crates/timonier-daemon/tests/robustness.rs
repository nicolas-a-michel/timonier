//! What the first review of Phase 3 found (PLAN.md): each scenario it reproduced, kept as a
//! test.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use timonier_core::{NodeId, NodeKind, Position, Provenance, ScopeKey};
use timonier_daemon::Pipeline;
use timonier_index::lsp::{CallTarget, LanguageServer, LspError, Priority};
use timonier_store::Store;

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().expect("a file has a parent")).expect("create dirs");
    std::fs::write(path, text).expect("write file");
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.name=T",
            "-c",
            "user.email=t@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn head_ids(store: &Store) -> Vec<String> {
    let mut reader = store.reader().expect("reader");
    let head = reader.head().expect("head").seq;
    reader
        .nodes_at(head)
        .expect("nodes")
        .into_iter()
        .map(|n| n.id.to_string())
        .collect()
}

/// A crate of 12 source files and 2 tests: 14 > 12, so `tests/` is a component.
fn big_crate(root: &Path, name: &str) {
    write(
        root,
        "crates/big/Cargo.toml",
        &format!("[package]\nname = \"{name}\"\n"),
    );
    let mut lib = String::new();
    for i in 0..11 {
        let _ = writeln!(lib, "pub mod m{i};");
        write(
            root,
            &format!("crates/big/src/m{i}.rs"),
            &format!("pub fn f{i}() {{}}\n"),
        );
    }
    write(root, "crates/big/src/lib.rs", &lib);
    write(root, "crates/big/tests/a.rs", "#[test]\nfn a() {}\n");
    write(root, "crates/big/tests/b.rs", "#[test]\nfn b() {}\n");
}

/// B1: renaming a package that holds components, or removing its manifest, must not leave a
/// component under a package that is gone.
#[test]
fn renaming_or_removing_a_package_with_components() {
    let repo = tempfile::tempdir().unwrap();
    let db = tempfile::tempdir().unwrap();
    write(
        repo.path(),
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\"]\n",
    );
    big_crate(repo.path(), "big");
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.index_all().unwrap();
    assert!(head_ids(&store).contains(&"component:dir/crates/big/tests".to_owned()));

    write(
        repo.path(),
        "crates/big/Cargo.toml",
        "[package]\nname = \"huge\"\n",
    );
    pipeline
        .refresh(&[PathBuf::from("crates/big/Cargo.toml")])
        .unwrap();
    let ids = head_ids(&store);
    assert!(ids.contains(&"pkg:cargo/huge".to_owned()), "{ids:?}");
    assert!(!ids.contains(&"pkg:cargo/big".to_owned()), "{ids:?}");
    let reader = store.reader().unwrap();
    let head = reader.head().unwrap().seq;
    let tests = reader
        .node_at(&NodeId::component("dir/crates/big/tests").unwrap(), head)
        .unwrap()
        .unwrap();
    assert_eq!(
        tests.parent.as_ref().map(NodeId::as_str),
        Some("pkg:cargo/huge")
    );
    // Later batches touching the structure still go through.
    write(repo.path(), "crates/big/src/m11.rs", "pub fn f11() {}\n");
    pipeline
        .refresh(&[PathBuf::from("crates/big/src/m11.rs")])
        .unwrap();

    std::fs::remove_file(repo.path().join("crates/big/Cargo.toml")).unwrap();
    pipeline
        .refresh(&[PathBuf::from("crates/big/Cargo.toml")])
        .unwrap();
    let ids = head_ids(&store);
    assert!(
        !ids.iter().any(|i| i.starts_with("pkg:cargo/huge")),
        "{ids:?}"
    );
    // Its 17 files now hang from the repo: more than 12, so directories group them again.
    let reader = store.reader().unwrap();
    let head = reader.head().unwrap().seq;
    let tests = reader
        .node_at(&NodeId::component("dir/crates/big/tests").unwrap(), head)
        .unwrap()
        .unwrap();
    assert!(
        tests
            .parent
            .as_ref()
            .is_some_and(|p| !p.as_str().starts_with("pkg:")),
        "{tests:?}"
    );
    assert!(
        ids.contains(&"file:crates/big/tests/a.rs".to_owned()),
        "{ids:?}"
    );
}

/// I2: a broken manifest, or a file whose path makes no id, is left out; the rest is indexed.
#[test]
fn local_problems_do_not_stop_the_indexing() {
    let repo = tempfile::tempdir().unwrap();
    let db = tempfile::tempdir().unwrap();
    write(repo.path(), "Cargo.toml", "[package]\nname = \"ok\"\n");
    write(repo.path(), "src/lib.rs", "pub fn f() {}\n");
    write(
        repo.path(),
        "tests/fixtures/bad/package.json",
        r#"{"name": "bad",}"#,
    );
    if cfg!(unix) {
        write(repo.path(), "src/we\\ird.rs", "pub fn w() {}\n");
    }
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.index_all().unwrap();
    assert!(head_ids(&store).contains(&"rust:ok::f".to_owned()));
    // A manifest saved mid-edit keeps the packages as they were.
    write(repo.path(), "Cargo.toml", "[package\nname = \"ok\"\n");
    write(repo.path(), "src/lib.rs", "pub fn f() {}\npub fn g() {}\n");
    pipeline
        .refresh(&[PathBuf::from("Cargo.toml"), PathBuf::from("src/lib.rs")])
        .unwrap();
    let ids = head_ids(&store);
    assert!(ids.contains(&"pkg:cargo/ok".to_owned()), "{ids:?}");
    assert!(ids.contains(&"rust:ok::g".to_owned()), "{ids:?}");
}

/// I3: a commit does not bring in files the walk excludes (hidden directory, `.gitignore`).
#[test]
fn a_commit_brings_in_no_excluded_file() {
    let repo = tempfile::tempdir().unwrap();
    let db = tempfile::tempdir().unwrap();
    let root = repo.path();
    write(root, "web/package.json", r#"{"name": "web"}"#);
    write(root, "web/src/a.ts", "export function a() {}\n");
    write(root, "web/.storybook/main.ts", "export const s = 1;\n");
    write(root, "web/ignored.ts", "export const i = 1;\n");
    write(root, ".gitignore", "ignored.ts\n");
    git(root, &["init", "--quiet"]);
    git(root, &["add", "."]);
    git(root, &["add", "--force", "web/ignored.ts"]);
    git(root, &["commit", "--quiet", "-m", "one"]);
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(root, store.clone()).unwrap();
    pipeline.index_all().unwrap();
    write(root, "web/.storybook/main.ts", "export const s = 2;\n");
    write(root, "web/ignored.ts", "export const i = 2;\n");
    write(
        root,
        "web/src/a.ts",
        "export function a() {}\nexport function b() {}\n",
    );
    git(root, &["add", "web/src/a.ts"]);
    git(root, &["commit", "--quiet", "-m", "two"]);
    let consolidation = pipeline.consolidate().unwrap().expect("a new commit");
    let mut reader = store.reader().unwrap();
    let commit: Vec<String> = reader
        .nodes_at(consolidation.seq)
        .unwrap()
        .into_iter()
        .map(|n| n.id.to_string())
        .collect();
    for ids in [commit, head_ids(&store)] {
        assert!(
            ids.contains(&"typescript:web/src/a#b".to_owned()),
            "{ids:?}"
        );
        assert!(
            !ids.iter()
                .any(|i| i.contains("storybook") || i.contains("ignored")),
            "{ids:?}"
        );
    }
}

/// I4: saving a Cargo library again parses that file only.
#[test]
fn saving_a_library_is_an_ordinary_save() {
    let repo = tempfile::tempdir().unwrap();
    let db = tempfile::tempdir().unwrap();
    write(repo.path(), "Cargo.toml", "[package]\nname = \"k\"\n");
    write(repo.path(), "src/lib.rs", "pub mod a;\n");
    write(repo.path(), "src/a.rs", "pub fn f() {}\n");
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.index_all().unwrap();
    write(repo.path(), "src/lib.rs", "pub mod a;\n// note\n");
    let report = pipeline.refresh(&[PathBuf::from("src/lib.rs")]).unwrap();
    assert_eq!(report.index.parsed, 1);
    assert_eq!(
        report.apply.nodes_changed + report.apply.nodes_added,
        0,
        "{report:?}"
    );
}

/// Answers `make` (line 0 of node.rs) calls `helper` (line 0 of util.rs).
struct Double;

impl LanguageServer for Double {
    fn outgoing_calls(&mut self, path: &str, at: Position) -> Result<Vec<CallTarget>, LspError> {
        Ok(if path == "src/node.rs" && at.line == 0 {
            vec![CallTarget {
                path: Some("src/util.rs".into()),
                at: Position::new(0, 7),
                sites: 1,
            }]
        } else {
            Vec::new()
        })
    }
}

fn lsp_calls(store: &Store) -> Vec<(String, String)> {
    let mut reader = store.reader().expect("reader");
    let head = reader.head().expect("head").seq;
    reader
        .edges_at(head)
        .expect("edges")
        .into_iter()
        .filter(|e| e.provenance == Provenance::Lsp)
        .map(|e| (e.src.to_string(), e.dst.to_string()))
        .collect()
}

/// I5: the LSP verifications survive a restart: dependents still come back, the budget still
/// counts them; a rename while away drops calls between symbols that no longer exist.
#[test]
fn lsp_state_survives_a_restart() {
    let repo = tempfile::tempdir().unwrap();
    let db = tempfile::tempdir().unwrap();
    write(repo.path(), "Cargo.toml", "[package]\nname = \"demo\"\n");
    write(repo.path(), "src/lib.rs", "pub mod node;\npub mod util;\n");
    write(
        repo.path(),
        "src/node.rs",
        "pub fn make() { crate::util::helper(); }\n",
    );
    write(repo.path(), "src/util.rs", "pub fn helper() {}\n");
    let open = || Store::open(db.path().join("graph.sqlite")).expect("store");
    {
        let store = open();
        let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
        pipeline.index_all().unwrap();
        pipeline.request_refinement("src/node.rs", Priority::Opened);
        pipeline.refine_next(&mut Double, || false).unwrap();
        assert_eq!(pipeline.symbol_edges(), (1, 0));
    }
    let store = open();
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.index_all().unwrap();
    assert_eq!(pipeline.symbol_edges(), (1, 0), "the detail is reloaded");
    write(repo.path(), "src/util.rs", "pub fn helper(x: u32) {}\n");
    pipeline.refresh(&[PathBuf::from("src/util.rs")]).unwrap();
    assert_eq!(
        pipeline.refinements_waiting(),
        1,
        "node.rs depends on the new signature"
    );
    drop(pipeline);

    // The library is renamed while the daemon is away: ids change.
    write(
        repo.path(),
        "Cargo.toml",
        "[package]\nname = \"demo\"\n[lib]\nname = \"renamed\"\n",
    );
    let mut pipeline = Pipeline::new(repo.path(), store.clone()).unwrap();
    pipeline.index_all().unwrap();
    assert!(lsp_calls(&store).is_empty(), "{:?}", lsp_calls(&store));
    let scope = ScopeKey::of_file(&NodeId::file("src/node.rs").unwrap()).unwrap();
    assert!(
        store
            .reader()
            .unwrap()
            .verified(&scope, Provenance::Lsp)
            .unwrap()
            .is_empty()
    );
    assert_eq!(pipeline.refinements_waiting(), 1);
    pipeline.refine_next(&mut Double, || false).unwrap();
    assert_eq!(
        lsp_calls(&store),
        [(
            "rust:renamed::node::make".into(),
            "rust:renamed::util::helper".into()
        )]
    );
    // While running, a rename drops them too.
    write(
        repo.path(),
        "Cargo.toml",
        "[package]\nname = \"demo\"\n[lib]\nname = \"again\"\n",
    );
    pipeline.refresh(&[PathBuf::from("Cargo.toml")]).unwrap();
    assert!(lsp_calls(&store).is_empty(), "{:?}", lsp_calls(&store));
}

/// I7: components are those of the last commit; a restart does not compute them again, even
/// with work in progress.
#[test]
fn a_restart_keeps_the_components() {
    let repo = tempfile::tempdir().unwrap();
    let db = tempfile::tempdir().unwrap();
    let root = repo.path();
    write(
        root,
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\"]\n",
    );
    big_crate(root, "big");
    git(root, &["init", "--quiet"]);
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", "one"]);
    // Work in progress that a clustering would change: a third test, a new directory.
    write(root, "crates/big/tests/c.rs", "#[test]\nfn c() {}\n");
    write(root, "crates/big/src/views/v1.rs", "pub fn v1() {}\n");
    write(root, "crates/big/src/views/v2.rs", "pub fn v2() {}\n");
    let open = || Store::open(db.path().join("graph.sqlite")).expect("store");
    let components = |store: &Store| -> BTreeMap<String, Option<String>> {
        let mut reader = store.reader().expect("reader");
        let head = reader.head().expect("head").seq;
        reader
            .nodes_at(head)
            .expect("nodes")
            .into_iter()
            .filter(|n| n.kind != NodeKind::Symbol(timonier_core::SymbolKind::Function))
            .map(|n| (n.id.to_string(), n.parent.map(|p| p.to_string())))
            .collect()
    };
    let store = open();
    Pipeline::new(root, store.clone())
        .unwrap()
        .index_all()
        .unwrap();
    let first = components(&store);
    drop(store);
    let store = open();
    Pipeline::new(root, store.clone())
        .unwrap()
        .index_all()
        .unwrap();
    assert_eq!(components(&store), first);
    assert_eq!(store.total_changes().unwrap(), 0);
}

/// Second review, N1: a directory removed then created again (a branch switch) is watched again.
#[test]
fn a_directory_created_again_is_watched_again() {
    use std::sync::mpsc;
    use std::time::Duration;
    let repo = tempfile::tempdir().unwrap();
    let db = tempfile::tempdir().unwrap();
    let root = repo.path().to_path_buf();
    write(&root, "Cargo.toml", "[package]\nname = \"w\"\n");
    write(&root, "src/lib.rs", "pub mod views;\n");
    write(&root, "src/views/mod.rs", "pub fn v() {}\n");
    git(&root, &["init", "--quiet", "--initial-branch=main"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "one"]);
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let pipeline = Pipeline::new(&root, store.clone()).unwrap();
    let (send, receive) = mpsc::channel::<()>();
    let watch =
        timonier_daemon::Watch::start(pipeline, Duration::from_millis(100), None, move |_| {
            let _ = send.send(());
        })
        .unwrap();
    let wait_for = |id: &str| {
        for _ in 0..200 {
            if head_ids(&store).iter().any(|i| i == id) {
                return true;
            }
            let _ = receive.recv_timeout(Duration::from_millis(100));
        }
        false
    };
    assert!(wait_for("rust:w::views::v"));
    // The directory goes away and comes back, then a file in it changes.
    std::fs::remove_dir_all(root.join("src/views")).unwrap();
    std::thread::sleep(Duration::from_millis(400));
    write(&root, "src/views/mod.rs", "pub fn v() {}\n");
    assert!(wait_for("rust:w::views::v"));
    std::thread::sleep(Duration::from_millis(400));
    write(
        &root,
        "src/views/mod.rs",
        "pub fn v() {}\npub fn edited() {}\n",
    );
    assert!(wait_for("rust:w::views::edited"), "{:?}", head_ids(&store));
    // A branch where the directory does not exist, and back.
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "two"]);
    git(&root, &["checkout", "--quiet", "-b", "bare"]);
    git(&root, &["rm", "--quiet", "-r", "src/views"]);
    git(&root, &["commit", "--quiet", "-m", "no views"]);
    std::thread::sleep(Duration::from_millis(400));
    git(&root, &["checkout", "--quiet", "main"]);
    assert!(wait_for("rust:w::views::edited"));
    std::thread::sleep(Duration::from_millis(400));
    write(
        &root,
        "src/views/mod.rs",
        "pub fn v() {}\npub fn again() {}\n",
    );
    assert!(wait_for("rust:w::views::again"), "{:?}", head_ids(&store));
    watch.stop().unwrap();
}

/// Second review, N2: a broken manifest elsewhere does not freeze the other packages.
#[test]
fn a_broken_fixture_does_not_freeze_the_packages() {
    let repo = tempfile::tempdir().unwrap();
    let db = tempfile::tempdir().unwrap();
    let root = repo.path();
    write(
        root,
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\"]\n",
    );
    write(root, "crates/a/Cargo.toml", "[package]\nname = \"a\"\n");
    write(root, "crates/a/src/lib.rs", "pub fn f() {}\n");
    write(
        root,
        "tests/fixtures/bad/package.json",
        r#"{"name": "bad",}"#,
    );
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(root, store.clone()).unwrap();
    pipeline.index_all().unwrap();
    write(root, "crates/b/Cargo.toml", "[package]\nname = \"b\"\n");
    write(root, "crates/b/src/lib.rs", "pub fn g() {}\n");
    write(
        root,
        "crates/a/Cargo.toml",
        "[package]\nname = \"renamed\"\n",
    );
    pipeline
        .refresh(&[
            PathBuf::from("crates/b"),
            PathBuf::from("crates/a/Cargo.toml"),
        ])
        .unwrap();
    let ids = head_ids(&store);
    for expected in [
        "pkg:cargo/b",
        "pkg:cargo/renamed",
        "rust:b::g",
        "rust:renamed::f",
    ] {
        assert!(ids.contains(&expected.to_owned()), "{expected} in {ids:?}");
    }
}

/// Second review, N3: a rename committed before its file events are seen.
#[test]
fn a_committed_rename_leaves_no_old_path_in_the_snapshot() {
    let repo = tempfile::tempdir().unwrap();
    let db = tempfile::tempdir().unwrap();
    let root = repo.path();
    write(root, "Cargo.toml", "[package]\nname = \"r\"\n");
    write(root, "src/lib.rs", "pub mod a;\n");
    write(root, "src/a.rs", "pub fn f() {}\n");
    git(root, &["init", "--quiet"]);
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", "one"]);
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(root, store.clone()).unwrap();
    pipeline.index_all().unwrap();
    git(root, &["mv", "src/a.rs", "src/b.rs"]);
    write(root, "src/lib.rs", "pub mod b;\n");
    git(root, &["commit", "--quiet", "-am", "rename"]);
    // No refresh: the commit is seen first.
    let consolidation = pipeline.consolidate().unwrap().expect("a new commit");
    let mut reader = store.reader().unwrap();
    let ids: Vec<String> = reader
        .nodes_at(consolidation.seq)
        .unwrap()
        .into_iter()
        .map(|n| n.id.to_string())
        .collect();
    assert!(ids.contains(&"file:src/b.rs".to_owned()), "{ids:?}");
    assert!(!ids.contains(&"file:src/a.rs".to_owned()), "{ids:?}");
}

/// Second review, R1: a head without components stays so after a restart, even when the work
/// in progress would make some.
#[test]
fn a_restart_without_components_writes_nothing() {
    let repo = tempfile::tempdir().unwrap();
    let db = tempfile::tempdir().unwrap();
    let root = repo.path();
    write(root, "Cargo.toml", "[package]\nname = \"s\"\n");
    write(root, "src/lib.rs", "pub fn f() {}\n");
    git(root, &["init", "--quiet"]);
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", "one"]);
    // Uncommitted: 13 more files with a `tests/` pair, enough for components.
    for i in 0..11 {
        write(
            root,
            &format!("src/m{i}.rs"),
            &format!("pub fn g{i}() {{}}\n"),
        );
    }
    write(root, "tests/a.rs", "#[test]\nfn a() {}\n");
    write(root, "tests/b.rs", "#[test]\nfn b() {}\n");
    let open = || Store::open(db.path().join("graph.sqlite")).expect("store");
    let store = open();
    Pipeline::new(root, store.clone())
        .unwrap()
        .index_all()
        .unwrap();
    let components = |store: &Store| {
        head_ids(store)
            .into_iter()
            .filter(|i| i.starts_with("component:dir"))
            .count()
    };
    assert_eq!(components(&store), 0, "the commit has none");
    drop(store);
    let store = open();
    Pipeline::new(root, store.clone())
        .unwrap()
        .index_all()
        .unwrap();
    assert_eq!(store.total_changes().unwrap(), 0);
    assert_eq!(components(&store), 0);
}

/// Second review, m-a: a package renamed while the daemon was away keeps its components.
#[test]
fn a_package_renamed_while_away_keeps_its_components() {
    let repo = tempfile::tempdir().unwrap();
    let db = tempfile::tempdir().unwrap();
    write(
        repo.path(),
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\"]\n",
    );
    big_crate(repo.path(), "big");
    let open = || Store::open(db.path().join("graph.sqlite")).expect("store");
    let store = open();
    Pipeline::new(repo.path(), store.clone())
        .unwrap()
        .index_all()
        .unwrap();
    drop(store);
    write(
        repo.path(),
        "crates/big/Cargo.toml",
        "[package]\nname = \"huge\"\n",
    );
    let store = open();
    Pipeline::new(repo.path(), store.clone())
        .unwrap()
        .index_all()
        .unwrap();
    let reader = store.reader().unwrap();
    let head = reader.head().unwrap().seq;
    let tests = reader
        .node_at(&NodeId::component("dir/crates/big/tests").unwrap(), head)
        .unwrap()
        .expect("the component is still there");
    assert_eq!(
        tests.parent.as_ref().map(NodeId::as_str),
        Some("pkg:cargo/huge")
    );
}

/// Third review, F1: the daemon's own reads are not changes. At rest, the watch does no pass;
/// a save still gets one; a slow refinement finishes.
#[test]
fn the_watch_rests_when_nothing_changes() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use timonier_daemon::{Event, Refinement, Watch};

    /// Every function calls `helper`, slowly.
    struct Slow;
    impl LanguageServer for Slow {
        fn outgoing_calls(&mut self, _: &str, _: Position) -> Result<Vec<CallTarget>, LspError> {
            std::thread::sleep(Duration::from_millis(20));
            Ok(vec![CallTarget {
                path: Some("src/util.rs".into()),
                at: Position::new(0, 7),
                sites: 1,
            }])
        }
    }

    let repo = tempfile::tempdir().unwrap();
    let db = tempfile::tempdir().unwrap();
    let root = repo.path().to_path_buf();
    write(&root, "Cargo.toml", "[package]\nname = \"rest\"\n");
    write(&root, "src/lib.rs", "pub mod util;\npub mod many;\n");
    write(&root, "src/util.rs", "pub fn helper() {}\n");
    let many: String = (0..10)
        .map(|i| format!("pub fn f{i}() {{ crate::util::helper(); }}\n"))
        .collect();
    write(&root, "src/many.rs", &many);
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let pipeline = Pipeline::new(&root, store.clone()).unwrap();
    let passes = Arc::new(AtomicUsize::new(0));
    let verified = Arc::new(AtomicUsize::new(0));
    let (p, v) = (Arc::clone(&passes), Arc::clone(&verified));
    let watch = Watch::start(
        pipeline,
        Duration::from_millis(100),
        Some(Box::new(Slow)),
        move |event| match event {
            Event::Indexed(_) => {
                p.fetch_add(1, Ordering::SeqCst);
            }
            Event::Refined(Ok(Refinement::Verified { .. })) => {
                v.fetch_add(1, Ordering::SeqCst);
            }
            _ => {}
        },
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    let settled = passes.load(Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(2000));
    assert_eq!(
        passes.load(Ordering::SeqCst),
        settled,
        "passes while nothing changed"
    );

    watch.request_refinement("src/many.rs", Priority::Opened);
    for _ in 0..100 {
        if verified.load(Ordering::SeqCst) > 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(
        verified.load(Ordering::SeqCst),
        1,
        "the refinement finished"
    );

    write(
        &root,
        "src/util.rs",
        "pub fn helper() {}\npub fn saved() {}\n",
    );
    for _ in 0..100 {
        if head_ids(&store)
            .iter()
            .any(|i| i == "rust:rest::util::saved")
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        head_ids(&store)
            .iter()
            .any(|i| i == "rust:rest::util::saved")
    );
    watch.stop().unwrap();
}

/// Third review, m-f: a crate created while the daemon runs is a package.
#[test]
fn a_new_crate_is_a_package() {
    use std::time::Duration;
    let repo = tempfile::tempdir().unwrap();
    let db = tempfile::tempdir().unwrap();
    let root = repo.path().to_path_buf();
    write(
        &root,
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\"]\n",
    );
    write(&root, "crates/a/Cargo.toml", "[package]\nname = \"a\"\n");
    write(&root, "crates/a/src/lib.rs", "pub fn f() {}\n");
    // Without the watcher: a refresh of the new directory finds its manifest.
    {
        let store = Store::open(db.path().join("direct.sqlite")).unwrap();
        let mut pipeline = Pipeline::new(&root, store.clone()).unwrap();
        pipeline.index_all().unwrap();
        write(&root, "crates/b/Cargo.toml", "[package]\nname = \"b\"\n");
        write(&root, "crates/b/src/lib.rs", "pub fn g() {}\n");
        pipeline.refresh(&[PathBuf::from("crates/b")]).unwrap();
        let ids = head_ids(&store);
        assert!(
            ids.contains(&"pkg:cargo/b".to_owned()) && ids.contains(&"rust:b::g".to_owned()),
            "{ids:?}"
        );
        std::fs::remove_dir_all(root.join("crates/b")).unwrap();
    }
    // Through the watcher.
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let pipeline = Pipeline::new(&root, store.clone()).unwrap();
    let watch =
        timonier_daemon::Watch::start(pipeline, Duration::from_millis(100), None, |_| {}).unwrap();
    std::thread::sleep(Duration::from_millis(800));
    write(&root, "crates/c/Cargo.toml", "[package]\nname = \"c\"\n");
    write(&root, "crates/c/src/lib.rs", "pub fn h() {}\n");
    let mut found = false;
    for _ in 0..100 {
        let ids = head_ids(&store);
        if ids.contains(&"pkg:cargo/c".to_owned()) && ids.contains(&"rust:c::h".to_owned()) {
            found = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(found, "{:?}", head_ids(&store));
    watch.stop().unwrap();
}

/// Counter-check of m-f: a walk that does not rediscover the packages (a new file's, the
/// watcher's, the consolidation's) sees the new crate first. The packages still follow it.
#[test]
fn a_new_crate_seen_by_another_walk_first_is_a_package() {
    let repo = tempfile::tempdir().unwrap();
    let db = tempfile::tempdir().unwrap();
    let root = repo.path().to_path_buf();
    write(
        &root,
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\"]\n",
    );
    write(&root, "crates/a/Cargo.toml", "[package]\nname = \"a\"\n");
    write(&root, "crates/a/src/lib.rs", "pub mod x;\npub fn a() {}\n");
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(&root, store.clone()).unwrap();
    pipeline.index_all().unwrap();
    write(&root, "crates/a/src/x.rs", "pub fn x() {}\n");
    write(&root, "crates/b/Cargo.toml", "[package]\nname = \"b\"\n");
    write(&root, "crates/b/src/lib.rs", "pub fn b() {}\n");
    // The watcher walks to find the directories to watch.
    pipeline.walk_dirs();
    // The new file comes first: its walk finds the new manifest.
    pipeline
        .refresh(&[PathBuf::from("crates/a/src/x.rs")])
        .unwrap();
    let ids = head_ids(&store);
    assert!(ids.contains(&"pkg:cargo/b".to_owned()), "{ids:?}");
    pipeline.refresh(&[PathBuf::from("crates/b")]).unwrap();
    // As a fresh indexing.
    let fresh = Store::open(db.path().join("fresh.sqlite")).unwrap();
    Pipeline::new(&root, fresh.clone())
        .unwrap()
        .index_all()
        .unwrap();
    assert_eq!(head_ids(&store), head_ids(&fresh));
}
