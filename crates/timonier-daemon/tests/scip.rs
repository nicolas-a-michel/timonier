//! A SCIP index read into the graph by the pipeline (ADR 0073; review of Phase 11, I1 and I3):
//! applied when it confirms enough of tree-sitter's links, refused otherwise, ignored for a
//! file changed since, and made in a worktree without the files the working tree changed.

use std::path::Path;
use std::process::Command;

use protobuf::Message;
use scip::types::{Document, Index, Occurrence, PositionEncoding};
use timonier_core::{EdgeKind, Provenance};
use timonier_daemon::Pipeline;
use timonier_daemon::scip::Produced;
use timonier_store::Store;

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().expect("a file has a parent")).expect("create dirs");
    std::fs::write(path, text).expect("write file");
}

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
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
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

const LIB: &str = "pub mod b;\npub fn a() {\n    b::f();\n}\n";
const B: &str = "pub fn f() {}\n";
const F: &str = "rust-analyzer cargo demo 0.1.0 b/f().";
const A: &str = "rust-analyzer cargo demo 0.1.0 a().";

/// A committed Rust crate where `a` calls `b::f`, indexed.
fn project() -> ([tempfile::TempDir; 2], Store, Pipeline, String) {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    write(root, "Cargo.toml", "[package]\nname = \"demo\"\n");
    write(root, "src/lib.rs", LIB);
    write(root, "src/b.rs", B);
    git(root, &["init", "--quiet", "--initial-branch=main"]);
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", "first"]);
    let sha = git(root, &["rev-parse", "HEAD"]);
    let db = tempfile::tempdir().expect("temp dir");
    let store = Store::open(db.path().join("graph.sqlite")).expect("store");
    let mut pipeline = Pipeline::new(root, store.clone()).expect("pipeline");
    pipeline.index_all().expect("index");
    ([dir, db], store, pipeline, sha)
}

fn occurrence(symbol: &str, line: i32, start: i32, definition: bool) -> Occurrence {
    Occurrence {
        symbol: symbol.to_owned(),
        symbol_roles: i32::from(definition),
        range: vec![line, start, start + 1],
        ..Occurrence::default()
    }
}

/// rust-analyzer's index of the crate, UTF-8 columns; without the call when `call` is false.
fn index(call: bool) -> Vec<u8> {
    let document = |path: &str, occurrences: Vec<Occurrence>| Document {
        relative_path: path.to_owned(),
        occurrences,
        position_encoding: PositionEncoding::UTF8CodeUnitOffsetFromLineStart.into(),
        ..Document::default()
    };
    let mut lib = vec![occurrence(A, 1, 7, true)];
    if call {
        lib.push(occurrence(F, 2, 7, false));
    }
    Index {
        documents: vec![
            document("src/b.rs", vec![occurrence(F, 0, 7, true)]),
            document("src/lib.rs", lib),
        ],
        ..Index::default()
    }
    .write_to_bytes()
    .expect("an index")
}

fn produced(pipeline: &Pipeline, sha: &str, bytes: Vec<u8>) -> Produced {
    let job = pipeline.scip_job(sha).expect("a job for the crate");
    let rust = job
        .languages
        .into_iter()
        .find(|l| l.language == "rust")
        .expect("the crate's language");
    Produced {
        sha: sha.to_owned(),
        language: "rust".to_owned(),
        indexer: "test".to_owned(),
        seconds: 0.0,
        bytes,
        files: rust.files,
    }
}

/// The calls between the two functions, by provenance.
fn calls(store: &Store) -> Vec<Provenance> {
    let mut reader = store.reader().expect("reader");
    let head = reader.head().expect("head").seq;
    reader
        .edges_at(head)
        .expect("edges")
        .into_iter()
        .filter(|e| e.kind == EdgeKind::Calls && e.dst.as_str().contains("b"))
        .map(|e| e.provenance)
        .collect()
}

#[test]
fn an_index_that_confirms_tree_sitter_replaces_its_calls() {
    let (_dir, store, mut pipeline, sha) = project();
    assert_eq!(calls(&store), [Provenance::TreeSitter]);
    let applied = pipeline
        .apply_scip(&produced(&pipeline, &sha, index(true)))
        .expect("apply");
    assert_eq!(applied.confirmed, Some(1.0));
    assert!(applied.apply.is_some());
    let mut reader = store.reader().expect("reader");
    let head = reader.head().expect("head").seq;
    let edges = reader.edges_at(head).expect("edges");
    assert!(
        edges.iter().any(|e| e.kind == EdgeKind::Calls
            && e.provenance == Provenance::Scip
            && e.src.as_str() == "rust:demo::a"
            && e.dst.as_str() == "rust:demo::b::f"),
        "{edges:#?}"
    );
    // The file-to-file call tree-sitter saw is verified, so it goes (ADR 0009).
    assert_eq!(calls(&store), [Provenance::Scip]);
}

#[test]
fn an_index_that_confirms_too_little_is_not_applied() {
    let (_dir, store, mut pipeline, sha) = project();
    let applied = pipeline
        .apply_scip(&produced(&pipeline, &sha, index(false)))
        .expect("apply");
    assert_eq!(applied.confirmed, Some(0.0));
    assert!(applied.apply.is_none());
    assert_eq!(calls(&store), [Provenance::TreeSitter]);
}

#[test]
fn a_file_changed_since_the_indexer_read_it_keeps_its_links() {
    let ([dir, _db], store, mut pipeline, sha) = project();
    let produced = produced(&pipeline, &sha, index(true));
    // Changed while the indexer ran: the index describes another content.
    write(
        dir.path(),
        "src/lib.rs",
        &format!("{LIB}\npub fn c() {{}}\n"),
    );
    pipeline.index_all().expect("index again");
    let applied = pipeline.apply_scip(&produced).expect("apply");
    let stats = applied.apply.expect("applied to the other files");
    assert!(stats.stale_observations >= 1, "{stats:?}");
    assert_eq!(calls(&store), [Provenance::TreeSitter]);
}

/// An indexer that reads the commit in a worktree (scip-java) gets no file the working tree
/// changed since: its facts would be the commit's, under the current content's hash.
#[test]
fn a_worktree_index_leaves_out_the_files_changed_since_the_commit() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    write(
        root,
        "pom.xml",
        "<project><groupId>demo</groupId><artifactId>demo</artifactId><version>1</version></project>\n",
    );
    write(
        root,
        "src/main/java/demo/A.java",
        "package demo;\npublic class A { B b; }\n",
    );
    write(
        root,
        "src/main/java/demo/B.java",
        "package demo;\npublic class B {}\n",
    );
    git(root, &["init", "--quiet", "--initial-branch=main"]);
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", "first"]);
    let sha = git(root, &["rev-parse", "HEAD"]);
    write(
        root,
        "src/main/java/demo/B.java",
        "package demo;\npublic class B { int x; }\n",
    );
    let db = tempfile::tempdir().expect("temp dir");
    let store = Store::open(db.path().join("graph.sqlite")).expect("store");
    let mut pipeline = Pipeline::new(root, store).expect("pipeline");
    pipeline.index_all().expect("index");
    let job = pipeline.scip_job(&sha).expect("a job");
    let java = job
        .languages
        .iter()
        .find(|l| l.language == "java")
        .expect("java's job");
    assert!(
        !java.rule.in_place,
        "scip-java reads the commit in a worktree"
    );
    let files: Vec<&str> = java.files.keys().map(String::as_str).collect();
    assert_eq!(files, ["src/main/java/demo/A.java"]);
}
