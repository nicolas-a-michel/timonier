//! The cost of a commit snapshot (ADR 0015, 0025): under the working tree's packages, and
//! under the commit's own when a manifest changed without being committed (Q23).
//!
//! `cargo run --release -p timonier-daemon --example commit_bench [CxDxF] [runs]`
//!
//! A synthetic Cargo workspace (`50x8x5` by default, see `support/synthetic.rs`) is written to
//! a temporary directory, committed, and indexed by a pipeline. Then, `runs` times (5 by
//! default):
//! - same packages: a source file edited and committed, then the consolidation;
//! - other packages: a crate renamed in its manifest and read, not committed; another file
//!   committed, then the consolidation, which freezes the commit under the commit's packages
//!   and gives the working tree's back. The manifest is put back before the next run.

use std::path::Path;
use std::time::{Duration, Instant};

use timonier_daemon::Pipeline;
use timonier_store::Store;

#[path = "support/synthetic.rs"]
mod synthetic;

fn main() {
    let mut args = std::env::args().skip(1);
    let size = args.next().unwrap_or_else(|| "50x8x5".into());
    let runs: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(5);
    let repo = tempfile::tempdir().expect("temp dir");
    let db = tempfile::tempdir().expect("temp dir");
    let root = repo.path();
    let files = synthetic::write(&size, root);
    git(root, &["init", "--quiet", "--initial-branch=main"]);
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", "synthetic"]);
    let store = Store::open(db.path().join("graph.sqlite")).expect("store");
    let mut pipeline = Pipeline::new(root, store).expect("pipeline");
    let start = Instant::now();
    pipeline.index_all().expect("index");
    println!(
        "synthetic:{size}: {files} files, indexed in {}",
        ms(start.elapsed())
    );

    let edited = root.join("crates/c1/src/d0/f0.rs");
    let manifest = root.join("crates/c2/Cargo.toml");
    let original = std::fs::read_to_string(&manifest).expect("manifest");
    let mut same = Vec::new();
    let mut other = Vec::new();
    for run in 0..runs {
        // Same packages.
        let mut text = std::fs::read_to_string(&edited).expect("source");
        text.push_str(&format!("\npub fn commit_bench_{run}() {{}}\n"));
        std::fs::write(&edited, text).expect("write");
        git(root, &["commit", "--quiet", "--all", "-m", "edit"]);
        same.push(consolidate(&mut pipeline));
        // Other packages: c2 renamed in the working tree only.
        std::fs::write(
            &manifest,
            original.replace("name = \"c2\"", &format!("name = \"c2-renamed-{run}\"")),
        )
        .expect("write");
        pipeline
            .refresh(std::slice::from_ref(&manifest))
            .expect("refresh");
        let notes = root.join("NOTES.md");
        std::fs::write(&notes, format!("Run {run}.\n")).expect("write");
        git(root, &["add", "NOTES.md"]);
        git(root, &["commit", "--quiet", "-m", "notes"]);
        other.push(consolidate(&mut pipeline));
        std::fs::write(&manifest, &original).expect("write");
        pipeline
            .refresh(std::slice::from_ref(&manifest))
            .expect("refresh");
    }
    println!(
        "commit snapshot, same packages: p50 {} (max {})",
        ms(percentile(&mut same, 50)),
        ms(percentile(&mut same, 100))
    );
    println!(
        "commit snapshot, other packages (a crate renamed, not committed): p50 {} (max {})",
        ms(percentile(&mut other, 50)),
        ms(percentile(&mut other, 100))
    );
}

/// The consolidation of the commit just made, which must freeze it.
fn consolidate(pipeline: &mut Pipeline) -> Duration {
    let start = Instant::now();
    let consolidated = pipeline.consolidate().expect("consolidate");
    let elapsed = start.elapsed();
    assert!(consolidated.snapshot.is_some(), "a commit snapshot");
    elapsed
}

fn git(root: &Path, args: &[&str]) {
    // A throwaway repository: no signing, a fixed author.
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.name=Bench",
            "-c",
            "user.email=bench@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

fn percentile(values: &mut [Duration], p: usize) -> Duration {
    values.sort();
    let index = (values.len() * p / 100).min(values.len().saturating_sub(1));
    values.get(index).copied().unwrap_or_default()
}

fn ms(duration: Duration) -> String {
    format!("{:.1} ms", duration.as_secs_f64() * 1000.0)
}
