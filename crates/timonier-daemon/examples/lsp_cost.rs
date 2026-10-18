//! What refinement asks the language servers about a repository (ADR 0014, 0022): for each
//! language, the call hierarchies, then the usages other than calls, counted per site, per path
//! cited, and as asked once the paths found in the file itself, in a dependency, or in the
//! standard library are left out. Then what a changed signature costs: the symbols whose name
//! another file cites are looked up (one `lsp/references` each), and the others are not; the
//! files citing the name are what marking by name would verify again.
//!
//! `cargo run --release -p timonier-daemon --example lsp_cost -- [repository]`
//!
//! The repository (the current directory by default) is indexed into a temporary database.

use std::collections::BTreeMap;
use std::path::PathBuf;

use timonier_core::EdgeKind;
use timonier_daemon::Pipeline;
use timonier_store::Store;

#[derive(Default)]
struct Cost {
    files: usize,
    functions: usize,
    sites: u64,
    paths: usize,
    asked: usize,
    symbols: usize,
    cited: usize,
    citers: usize,
}

fn main() {
    let root = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    let db = tempfile::tempdir().expect("temp dir");
    let store = Store::open(db.path().join("graph.sqlite")).expect("store");
    let mut pipeline = Pipeline::new(&root, store).expect("pipeline");
    pipeline.index_all().expect("index");
    let project = pipeline.indexer().project();
    let mut costs: BTreeMap<String, Cost> = BTreeMap::new();
    for file in project.files() {
        let Some(job) = project.refine_job(&file.path) else {
            continue;
        };
        let cost = costs.entry(file.language.to_string()).or_default();
        cost.files += 1;
        cost.functions += job.functions.len();
        for reference in &file.facts.references {
            if matches!(
                reference.kind,
                EdgeKind::References | EdgeKind::Implements | EdgeKind::Inherits
            ) {
                cost.sites += u64::from(reference.count);
                cost.paths += 1;
            }
        }
        cost.asked += job.usages.len();
        for symbol in &file.facts.symbols {
            cost.symbols += 1;
            let citers = project
                .citing(symbol.name())
                .filter(|path| **path != file.path)
                .count();
            if citers > 0 {
                cost.cited += 1;
                cost.citers += citers;
            }
        }
    }
    println!("{}", root.display());
    println!(
        "{:<12} {:>6} {:>16} {:>14} {:>14} {:>10}",
        "language", "files", "call hierarchies", "usage sites", "paths cited", "asked"
    );
    for (language, cost) in &costs {
        println!(
            "{language:<12} {:>6} {:>16} {:>14} {:>14} {:>10}",
            cost.files, cost.functions, cost.sites, cost.paths, cost.asked
        );
    }
    println!(
        "Definitions go in one batch per file: as many round trips as files, at most {}.",
        costs.values().map(|c| c.files).sum::<usize>()
    );
    println!();
    println!("A changed signature:");
    println!(
        "{:<12} {:>8} {:>18} {:>22}",
        "language", "symbols", "looked up (cited)", "files citing, on average"
    );
    for (language, cost) in &costs {
        let average = if cost.cited == 0 {
            0.0
        } else {
            cost.citers as f64 / cost.cited as f64
        };
        println!(
            "{language:<12} {:>8} {:>18} {average:>22.1}",
            cost.symbols, cost.cited
        );
    }
}
