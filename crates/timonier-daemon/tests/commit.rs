//! Speed 3 (ADR 0015) on a temporary git repository: the commit snapshot describes the
//! committed tree, uncommitted work stays in the head.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

use timonier_core::SnapshotKind;
use timonier_daemon::{Event, Pipeline, Watch};
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
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

fn repository() -> (tempfile::TempDir, tempfile::TempDir) {
    let repo = tempfile::tempdir().expect("temp dir");
    let root = repo.path();
    write(root, "Cargo.toml", "[package]\nname = \"demo\"\n");
    write(root, "src/lib.rs", "pub mod b;\npub fn a() {}\n");
    write(root, "src/b.rs", "pub fn b() {}\n");
    git(root, &["init", "--quiet", "--initial-branch=main"]);
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", "first"]);
    (repo, tempfile::tempdir().expect("temp dir"))
}

fn ids_at(store: &Store, seq: u64) -> Vec<String> {
    let mut reader = store.reader().expect("test helper");
    reader
        .nodes_at(seq)
        .expect("test helper")
        .into_iter()
        .map(|n| n.id.to_string())
        .collect()
}

#[test]
fn the_commit_snapshot_is_the_committed_tree_and_the_head_keeps_the_work() {
    let (repo, db) = repository();
    let root = repo.path();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(root, store.clone()).unwrap();
    pipeline.index_all().unwrap();
    // The first indexing gives HEAD its snapshot.
    let first = store
        .reader()
        .unwrap()
        .latest(SnapshotKind::Commit)
        .unwrap()
        .unwrap();
    assert_eq!(
        first.commit_sha.as_deref(),
        Some(git(root, &["rev-parse", "HEAD"]).as_str())
    );
    assert!(
        pipeline.consolidate().unwrap().snapshot.is_none(),
        "nothing new"
    );

    // Work in progress: lib.rs modified, new.rs untracked. Only b.rs is committed.
    write(
        root,
        "src/lib.rs",
        "pub mod b;\npub fn a() {}\npub fn wip() {}\n",
    );
    write(root, "src/new.rs", "pub fn n() {}\n");
    write(root, "src/b.rs", "pub fn b() {}\npub fn b2() {}\n");
    git(root, &["add", "src/b.rs"]);
    git(root, &["commit", "--quiet", "-m", "second"]);
    let sha = git(root, &["rev-parse", "HEAD"]);
    pipeline
        .refresh(&[
            PathBuf::from("src/lib.rs"),
            PathBuf::from("src/new.rs"),
            PathBuf::from("src/b.rs"),
        ])
        .unwrap();
    let consolidation = pipeline
        .consolidate()
        .unwrap()
        .snapshot
        .expect("a new commit");
    assert_eq!(consolidation.sha, sha);
    assert_eq!(consolidation.dirty, 2);

    let committed = ids_at(&store, consolidation.seq);
    assert!(
        committed.contains(&"rust:demo::b::b2".to_owned()),
        "{committed:?}"
    );
    for absent in ["rust:demo::wip", "rust:demo::new::n", "file:src/new.rs"] {
        assert!(
            !committed.contains(&absent.to_owned()),
            "{absent} in {committed:?}"
        );
    }
    let head = store.reader().unwrap().head().unwrap().seq;
    let working = ids_at(&store, head);
    for present in [
        "rust:demo::b::b2",
        "rust:demo::wip",
        "rust:demo::new::n",
        "file:src/new.rs",
    ] {
        assert!(
            working.contains(&present.to_owned()),
            "{present} in {working:?}"
        );
    }
    // The working tree is back as it was: indexing again writes nothing.
    let changes = store.total_changes().unwrap();
    pipeline
        .refresh(&[PathBuf::from("src/lib.rs"), PathBuf::from("src/new.rs")])
        .unwrap();
    assert_eq!(store.total_changes().unwrap(), changes);
}

#[test]
fn the_watcher_sees_a_commit() {
    let (repo, db) = repository();
    let root = repo.path().to_path_buf();
    let store = Store::open(db.path().join("graph.sqlite")).unwrap();
    let mut pipeline = Pipeline::new(&root, store.clone()).unwrap();
    pipeline.without_scip();
    pipeline.index_all().unwrap();
    let (send, receive) = mpsc::channel();
    let watch = Watch::start(pipeline, Duration::from_millis(100), None, move |event| {
        // The commits that got a snapshot, and the failures.
        if let Event::Committed(result) = event
            && let Some(result) = result.map(|c| c.snapshot).transpose()
        {
            let _ = send.send(result);
        }
    })
    .unwrap();
    std::thread::sleep(Duration::from_millis(500));
    write(&root, "src/b.rs", "pub fn b() {}\npub fn later() {}\n");
    git(&root, &["commit", "--quiet", "-am", "third"]);
    let sha = git(&root, &["rev-parse", "HEAD"]);
    let consolidation = receive
        .recv_timeout(Duration::from_secs(20))
        .expect("the commit is seen")
        .unwrap();
    assert_eq!(consolidation.sha, sha);
    assert!(ids_at(&store, consolidation.seq).contains(&"rust:demo::b::later".to_owned()));
    watch.stop().unwrap();
}
