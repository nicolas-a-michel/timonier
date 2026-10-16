//! Indexing timings on a real repository (PLAN.md 3.5 and 3.11, H2 and H3).
//!
//! `cargo run --release -p timonier-daemon --example index_bench [repository] [saves]`
//!
//! The repository (the current directory by default) is copied to a temporary directory, as
//! the walk sees it, so that its files are never touched. Then:
//! - cold indexing: empty database, packs loaded, every file parsed;
//! - warm indexing: a restart on the same database, with the extraction cache;
//! - saves: each one edits a file, then the pipeline brings the graph up to date. The time
//!   runs from the start of the pass (the end of the debounce, H2) to the commit in SQLite.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use timonier_daemon::Pipeline;
use timonier_store::Store;

fn main() {
    let mut args = std::env::args().skip(1);
    let source = PathBuf::from(args.next().unwrap_or_else(|| ".".into()));
    let saves: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(200);
    // The repository itself, read only, git included: the first indexing also gives HEAD its
    // commit snapshot (ADR 0015).
    {
        let db = tempfile::tempdir().expect("temp dir");
        let start = Instant::now();
        let store = Store::open(db.path().join("graph.sqlite")).expect("store");
        let mut pipeline = Pipeline::new(&source, store).expect("pipeline");
        let report = pipeline.index_all().expect("index");
        println!(
            "cold indexing of the repository itself, git included: {} ({} files)",
            ms(start.elapsed()),
            report.index.files
        );
    }
    let copy = tempfile::tempdir().expect("temp dir");
    let db = tempfile::tempdir().expect("temp dir");
    let copied = copy_repository(&source, copy.path());
    println!("repository: {} ({copied} files copied)", source.display());

    let start = Instant::now();
    let store = Store::open(db.path().join("graph.sqlite")).expect("store");
    let mut pipeline = Pipeline::new(copy.path(), store.clone()).expect("pipeline");
    let report = pipeline.index_all().expect("index");
    let cold = start.elapsed();
    println!(
        "cold indexing: {} ({} files, {} parsed; {} nodes added, {} edges added)",
        ms(cold),
        report.index.files,
        report.index.parsed,
        report.apply.nodes_added,
        report.apply.edges_added
    );
    drop(pipeline);
    drop(store);

    let start = Instant::now();
    let store = Store::open(db.path().join("graph.sqlite")).expect("store");
    let mut pipeline = Pipeline::new(copy.path(), store.clone()).expect("pipeline");
    let report = pipeline.index_all().expect("index");
    println!(
        "warm indexing (restart, cache): {} ({} parsed, {} cached, {} rows written)",
        ms(start.elapsed()),
        report.index.parsed,
        report.index.cached,
        store.total_changes().expect("changes")
    );

    // Components belong to the last commit: cluster the copy as it is, for a baseline.
    pipeline.recluster().expect("recluster");
    let before = pipeline.indexer().project().components().assignment.clone();

    let files: Vec<String> = pipeline
        .indexer()
        .project()
        .files()
        .map(|f| f.path.clone())
        .collect();
    let callees: std::collections::HashMap<String, String> = callees(pipeline.indexer().project())
        .into_iter()
        .map(|(file, callee)| (file.to_owned(), callee))
        .collect();
    let mut body = Vec::new();
    let mut symbols = Vec::new();
    for i in 0..saves {
        let path = &files[i % files.len()];
        let full = copy.path().join(path);
        let mut text = std::fs::read_to_string(&full).expect("read");
        // Half the saves change a body only; the other half add a definition.
        let adds = i % 2 == 1;
        text.push_str(&edit(
            path,
            i,
            adds,
            callees.get(path.as_str()).map(String::as_str),
        ));
        std::fs::write(&full, text).expect("write");
        let report = pipeline.refresh(&[PathBuf::from(path)]).expect("refresh");
        assert_eq!(report.index.parsed, 1, "{path}");
        if adds { &mut symbols } else { &mut body }.push(report.elapsed);
    }
    // Stability (ADR 0016): what the saves changed in the components, from the previous ones.
    let start = Instant::now();
    pipeline.recluster().expect("recluster");
    let after = &pipeline.indexer().project().components().assignment;
    let elements: std::collections::BTreeSet<_> = before.keys().chain(after.keys()).collect();
    let moved = elements
        .iter()
        .filter(|e| before.get(**e) != after.get(**e))
        .count();
    println!(
        "re-clustering after the saves: {} ({moved} of {} assignments changed)",
        ms(start.elapsed()),
        elements.len()
    );
    let mut all: Vec<Duration> = body.iter().chain(&symbols).copied().collect();
    print_percentiles("saves, all", &mut all);
    print_percentiles("saves, comment or body only", &mut body);
    print_percentiles("saves, a definition added", &mut symbols);
}

/// What a save appends: a comment, or a new definition.
/// What a save appends: a comment, or a new definition that calls a function of another file
/// of the package, so that the edges change too.
fn edit(path: &str, i: usize, adds: bool, callee: Option<&str>) -> String {
    let rust = path.ends_with(".rs");
    let call = callee.map_or(String::new(), |c| format!("{c}(); "));
    match (rust, adds) {
        (true, false) | (false, false) => format!("\n// bench edit {i}\n"),
        (true, true) => format!("\npub fn bench_added_{i}() -> u32 {{ {call}{i} }}\n"),
        (false, true) => {
            format!("\nexport function benchAdded{i}(): number {{ {call}return {i}; }}\n")
        }
    }
}

/// For each file, a function of another file of its package whose name is unique there:
/// tree-sitter resolves a call to it by name (ADR 0013).
fn callees(project: &timonier_index::project::Project) -> std::collections::HashMap<&str, String> {
    use std::collections::HashMap;
    let mut defined: HashMap<(Option<usize>, &str), Vec<&str>> = HashMap::new();
    for file in project.files() {
        for symbol in &file.facts.symbols {
            if symbol.kind == timonier_core::SymbolKind::Function && symbol.parent.is_none() {
                defined
                    .entry((file.package, symbol.name()))
                    .or_default()
                    .push(file.path.as_str());
            }
        }
    }
    let mut callees = HashMap::new();
    for file in project.files() {
        let other = defined
            .iter()
            .filter(|((package, name), files)| {
                *package == file.package
                    && files.len() == 1
                    && files[0] != file.path
                    && *name != "main"
            })
            .map(|((_, name), _)| *name)
            .min();
        if let Some(name) = other {
            callees.insert(file.path.as_str(), name.to_owned());
        }
    }
    callees
}

fn print_percentiles(label: &str, samples: &mut [Duration]) {
    if samples.is_empty() {
        return;
    }
    samples.sort();
    let at = |q: f64| samples[((samples.len() as f64 - 1.0) * q).round() as usize];
    println!(
        "{label}: {} saves, p50 {}, p95 {}, max {}",
        samples.len(),
        ms(at(0.5)),
        ms(at(0.95)),
        ms(at(1.0))
    );
}

fn ms(duration: Duration) -> String {
    format!("{:.2} ms", duration.as_secs_f64() * 1000.0)
}

/// Copies what the walk keeps (`.gitignore` respected) plus the ignore files themselves.
fn copy_repository(from: &Path, to: &Path) -> usize {
    let mut copied = 0;
    let walker = ignore::WalkBuilder::new(from)
        .hidden(false)
        .require_git(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build();
    for entry in walker.flatten() {
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let relative = entry.path().strip_prefix(from).expect("inside the root");
        let target = to.join(relative);
        std::fs::create_dir_all(target.parent().expect("parent")).expect("create dir");
        std::fs::copy(entry.path(), &target).expect("copy");
        copied += 1;
    }
    copied
}
