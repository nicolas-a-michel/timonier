//! Baseline timings of the store on a synthetic repository (PLAN.md task 2.12).
//!
//! cargo run --release -p timonier-store --example bench [packages] [files per package] [symbols per file]
//!
//! Each file holds `symbols` functions. Tree-sitter reports three file → file calls per file; the
//! LSP reports two function → function calls per function, verified, on the same content.

use std::error::Error;
use std::time::{Duration, Instant};

use timonier_core::{
    ContentHash, EdgeKind, Freeze, Language, Node, NodeId, NodeKind, Observation, Provenance,
    ScopeKey, ScopeUpdate, SourceObservations, SymbolKind,
};
use timonier_store::Store;

/// A small deterministic generator (xorshift), so that runs are comparable.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % n as u64).unwrap_or(0)
    }
}

struct Shape {
    packages: usize,
    files: usize,
    symbols: usize,
}

fn hash(seed: u64) -> ContentHash {
    let mut bytes = [0_u8; 32];
    let mut rng = Rng(seed | 1);
    for chunk in bytes.chunks_mut(8) {
        chunk.copy_from_slice(&rng.next().to_le_bytes());
    }
    ContentHash::from_bytes(bytes)
}

fn file_id(p: usize, f: usize) -> Result<NodeId, Box<dyn Error>> {
    Ok(NodeId::file(&format!("crates/p{p}/src/f{f}.rs"))?)
}

fn symbol_id(rust: &Language, p: usize, f: usize, s: usize) -> Result<NodeId, Box<dyn Error>> {
    Ok(NodeId::qualified(rust, &format!("p{p}::f{f}::s{s}"))?)
}

fn structure(shape: &Shape) -> Result<ScopeUpdate, Box<dyn Error>> {
    let repo = NodeId::repo("bench")?;
    let mut nodes = vec![Node::new(repo.clone(), NodeKind::Repo, "bench")];
    for p in 0..shape.packages {
        let package = NodeId::package("cargo", &format!("p{p}"))?;
        nodes.push(
            Node::new(package.clone(), NodeKind::Package, format!("p{p}"))
                .with_parent(repo.clone()),
        );
        for f in 0..shape.files {
            nodes.push(
                Node::new(file_id(p, f)?, NodeKind::File, format!("f{f}.rs"))
                    .with_parent(package.clone()),
            );
        }
    }
    Ok(ScopeUpdate::new(ScopeKey::structure()).with_nodes(nodes))
}

/// The tree-sitter pass and the LSP pass of one file, as two updates of its scope.
fn file_updates(
    shape: &Shape,
    p: usize,
    f: usize,
    version: u64,
    rng: &mut Rng,
) -> Result<[ScopeUpdate; 2], Box<dyn Error>> {
    let rust = Language::new("rust")?;
    let file = file_id(p, f)?;
    let scope = ScopeKey::of_file(&file)?;
    let content = hash(version.wrapping_mul(31) + (p * 1_000 + f) as u64);
    let mut nodes = Vec::with_capacity(shape.symbols);
    let mut precise = Vec::with_capacity(shape.symbols * 2);
    for s in 0..shape.symbols {
        let id = symbol_id(&rust, p, f, s)?;
        let body = (p * 1_000_000 + f * 1_000 + s) as u64 + version * 7_919;
        nodes.push(
            Node::new(
                id.clone(),
                NodeKind::Symbol(SymbolKind::Function),
                format!("s{s}"),
            )
            .with_parent(file.clone())
            .with_signature(format!("fn s{s}()"))
            .with_body_hash(hash(body)),
        );
        for _ in 0..2 {
            let target = symbol_id(
                &rust,
                rng.below(shape.packages),
                rng.below(shape.files),
                rng.below(shape.symbols),
            )?;
            precise.push(Observation::new(id.clone(), target, EdgeKind::Calls));
        }
    }
    let mut approximate = Vec::new();
    for _ in 0..3 {
        let target = file_id(rng.below(shape.packages), rng.below(shape.files))?;
        if target != file {
            approximate
                .push(Observation::new(file.clone(), target, EdgeKind::Calls).with_weight(2));
        }
    }
    let tree_sitter = ScopeUpdate::new(scope.clone())
        .with_content_hash(content)
        .with_nodes(nodes)
        .with_observations(SourceObservations::new(Provenance::TreeSitter, approximate));
    let lsp = ScopeUpdate::new(scope).with_observations(
        SourceObservations::new(Provenance::Lsp, precise)
            .verifying([EdgeKind::Calls])
            .computed_on(content),
    );
    Ok([tree_sitter, lsp])
}

fn time<T>(
    label: &str,
    f: impl FnOnce() -> Result<T, Box<dyn Error>>,
) -> Result<T, Box<dyn Error>> {
    let start = Instant::now();
    let value = f()?;
    println!(
        "{label:<56} {:>10.2} ms",
        start.elapsed().as_secs_f64() * 1000.0
    );
    Ok(value)
}

fn report(label: &str, samples: &mut [Duration]) {
    samples.sort();
    let median = samples[samples.len() / 2];
    let worst = samples[samples.len() - 1];
    println!(
        "{label:<56} {:>10.2} ms  (max {:.2} ms)",
        median.as_secs_f64() * 1000.0,
        worst.as_secs_f64() * 1000.0
    );
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<usize> = std::env::args()
        .skip(1)
        .filter_map(|a| a.parse().ok())
        .collect();
    let shape = Shape {
        packages: args.first().copied().unwrap_or(20),
        files: args.get(1).copied().unwrap_or(50),
        symbols: args.get(2).copied().unwrap_or(20),
    };
    let total_files = shape.packages * shape.files;
    println!(
        "synthetic repo: {} packages × {} files × {} symbols = {} files, {} symbols\n",
        shape.packages,
        shape.files,
        shape.symbols,
        total_files,
        total_files * shape.symbols
    );

    let dir = std::env::temp_dir().join(format!("timonier-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let store = Store::open(dir.join("graph.sqlite"))?;
    let mut rng = Rng(0x5eed);

    let mut updates = vec![structure(&shape)?];
    for p in 0..shape.packages {
        for f in 0..shape.files {
            updates.extend(file_updates(&shape, p, f, 0, &mut rng)?);
        }
    }
    time("initial load (tree-sitter + LSP), one transaction", || {
        Ok(store.apply_all(updates)?)
    })?;
    let head = store.reader()?.head()?.seq;
    println!(
        "{:<56} {:>10}",
        "  … stored edges",
        store.reader()?.edges_at(head)?.len()
    );

    // From the first sample on, every application is identical.
    let same = file_updates(&shape, 3, 7, 1, &mut rng)?;
    store.apply_all(same.to_vec())?;
    let mut samples = Vec::new();
    for _ in 0..20 {
        let start = Instant::now();
        store.apply_all(same.to_vec())?;
        samples.push(start.elapsed());
    }
    report("re-apply an identical file (median of 20)", &mut samples);

    let mut samples = Vec::new();
    for version in 2..22 {
        let update = file_updates(&shape, 3, 7, version, &mut rng)?;
        let start = Instant::now();
        store.apply_all(update.to_vec())?;
        samples.push(start.elapsed());
    }
    report(
        "change a file, rows born in the head (median of 20)",
        &mut samples,
    );

    // A freeze before each sample: every change closes old versions and opens new ones.
    let mut samples = Vec::new();
    for version in 22..42 {
        store.freeze(Freeze::WorkingTree)?;
        let update = file_updates(&shape, 4, 9, version, &mut rng)?;
        let start = Instant::now();
        store.apply_all(update.to_vec())?;
        samples.push(start.elapsed());
    }
    report("change a file, after a freeze (median of 20)", &mut samples);

    let review = store.freeze(Freeze::ReviewMark)?;
    // 10 % of the files, all distinct.
    let mut changed = Vec::new();
    for i in 0..total_files / 10 {
        changed.extend(file_updates(
            &shape,
            i / shape.files,
            i % shape.files,
            99,
            &mut rng,
        )?);
    }
    let label = format!(
        "change {} distinct files (10 %), one transaction",
        total_files / 10
    );
    time(&label, || Ok(store.apply_all(changed)?))?;

    let head = store.reader()?.head()?.seq;
    let mut reader = store.reader()?;
    let diff = time("diff since the review mark", || {
        Ok(reader.diff(review.seq, head)?)
    })?;
    println!(
        "{:<56} {:>10}",
        "  … node changes / edge changes",
        format!("{} / {}", diff.nodes.len(), diff.edges.len())
    );
    let nodes = time("read every node at the head", || Ok(reader.nodes_at(head)?))?;
    println!("{:<56} {:>10}", "  … nodes", nodes.len());
    let packages: Vec<NodeId> = (0..shape.packages)
        .map(|p| NodeId::package("cargo", &format!("p{p}")))
        .collect::<Result<_, _>>()?;
    let edges = time("aggregate the edges at the package level", || {
        Ok(reader.aggregate(head, &packages)?)
    })?;
    println!("{:<56} {:>10}", "  … aggregated edges", edges.len());

    drop(reader);
    drop(store);
    std::fs::remove_dir_all(&dir)?;
    Ok(())
}
