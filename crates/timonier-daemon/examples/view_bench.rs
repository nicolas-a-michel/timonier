//! Timings of the views (PLAN.md 4.2): what a subscription costs, and how long a save takes to
//! reach a client as a delta.
//!
//! `cargo run --release -p timonier-daemon --example view_bench [repository] [saves]`
//!
//! The repository (the current directory by default) is copied to a temporary directory, so
//! that its files are never touched, and a real daemon serves the copy. Then:
//! - the head read and the views computed directly, as the views thread does on each change;
//! - `view/subscribe` from a client, for the top of the map and with every package and
//!   component open, which shows every file;
//! - saves: each one adds a function to a file, which changes its number of children on the
//!   map; the time runs from the write to the delta received, debounce included.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;
use timonier_core::NodeKind;
use timonier_core::protocol::{
    StatusResult, SubscribeParams, ViewDelta, ViewSnapshot, ViewSpec, methods,
};
use timonier_daemon::view::{Graph, ViewState};
use timonier_daemon::{Client, Daemon, Options};
use timonier_store::Store;

fn main() {
    // `RUST_LOG=timonier_daemon=debug` shows what the daemon does.
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
    let mut args = std::env::args().skip(1);
    let source = PathBuf::from(args.next().unwrap_or_else(|| ".".into()));
    let saves: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(30);
    let copy = tempfile::tempdir().expect("temp dir");
    let runtime = tempfile::tempdir().expect("temp dir");
    let copied = copy_repository(&source, copy.path());
    println!("repository: {} ({copied} files copied)", source.display());

    let mut options = Options::new(copy.path());
    options.runtime = Some(runtime.path().join("run"));
    options.signals = false;
    let daemon = Daemon::bind(options)
        .expect("bind")
        .expect("no other daemon");
    let paths = daemon.paths().clone();
    let serving = std::thread::spawn(move || daemon.run());
    let mut client = Client::connect(&paths.socket).expect("connect");
    client.initialize("view_bench").expect("initialize");
    let start = Instant::now();
    let mut files = 0;
    while files == 0 {
        std::thread::sleep(Duration::from_millis(20));
        let status: StatusResult = client
            .request(methods::STATUS, &Value::Null)
            .expect("status");
        files = status.files;
    }
    println!(
        "first indexing seen by a client after {}: {files} files",
        ms(start.elapsed())
    );

    // Directly: what the views thread does on each change.
    // A second handle on the daemon's database, only read: its writer stays idle.
    let store = Store::open(&paths.database).expect("store");
    let mut reader = store.reader().expect("reader");
    let mut loads = Vec::new();
    let mut graph = Graph::default();
    for _ in 0..10 {
        let start = Instant::now();
        graph = Graph::load(&mut reader).expect("load");
        loads.push(start.elapsed());
    }
    let head = reader.head().expect("head").seq;
    let nodes = reader.nodes_at(head).expect("nodes");
    let edges = reader.edges_at(head).expect("edges").len();
    let packages: Vec<_> = nodes
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::Package | NodeKind::Component))
        .map(|n| n.id.clone())
        .collect();
    println!(
        "head: {} nodes, {edges} edges; read in p50 {} (max {})",
        nodes.len(),
        ms(percentile(&mut loads, 50)),
        ms(percentile(&mut loads, 100))
    );
    let top = ViewSpec::default();
    let open = ViewSpec {
        expanded: packages.clone(),
    };
    for (name, spec) in [("top", &top), ("every package and component open", &open)] {
        let mut computes = Vec::new();
        let mut state = ViewState::default();
        for _ in 0..10 {
            let start = Instant::now();
            state = ViewState::compute(&graph, spec);
            computes.push(start.elapsed());
        }
        let snapshot = state.snapshot(0, 0);
        let bytes = serde_json::to_vec(&snapshot).expect("json").len();
        println!(
            "view {name}: {} boxes, {} edges, {} KiB of JSON; computed in p50 {}",
            snapshot.nodes.len(),
            snapshot.edges.len(),
            bytes / 1024,
            ms(percentile(&mut computes, 50))
        );
    }

    // Through the socket.
    for (name, spec) in [("top", &top), ("every package and component open", &open)] {
        let start = Instant::now();
        let snapshot: ViewSnapshot = client
            .request(methods::SUBSCRIBE, &SubscribeParams { view: spec.clone() })
            .expect("subscribe");
        println!(
            "view/subscribe {name}: {} ({} boxes)",
            ms(start.elapsed()),
            snapshot.nodes.len()
        );
    }
    let snapshot: ViewSnapshot = client
        .request(methods::SUBSCRIBE, &SubscribeParams { view: open })
        .expect("subscribe");
    // Settle: what the first indexing still sends.
    while client
        .next_incoming(Duration::from_millis(1000))
        .expect("read")
        .is_some()
    {}

    let target = pick_file(copy.path());
    let mut content = std::fs::read_to_string(copy.path().join(&target)).expect("read");
    println!("saves: {saves}, each adding a function to {target}");
    let mut latencies = Vec::new();
    for i in 0..saves {
        // One more function each time: the file's children on the map change.
        content.push_str(&format!("\npub fn view_bench_{i}() {{}}\n"));
        let start = Instant::now();
        std::fs::write(copy.path().join(&target), &content).expect("write");
        let deadline = start + Duration::from_secs(10);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let Some(incoming) = client.next_incoming(left).expect("read") else {
                panic!("no delta within 10 s");
            };
            let delta: ViewDelta = serde_json::from_value(incoming.params).expect("delta");
            if delta.subscription == snapshot.subscription {
                latencies.push(start.elapsed());
                break;
            }
        }
        // Other views' deltas, and nothing else.
        while client
            .next_incoming(Duration::from_millis(400))
            .expect("read")
            .is_some()
        {}
    }
    println!(
        "save → delta received: p50 {}, p95 {}, max {} (debounce of {} included)",
        ms(percentile(&mut latencies, 50)),
        ms(percentile(&mut latencies, 95)),
        ms(percentile(&mut latencies, 100)),
        ms(timonier_daemon::watch::DEBOUNCE)
    );
    let _: Value = client
        .request(methods::SHUTDOWN, &Value::Null)
        .expect("shutdown");
    serving.join().expect("thread").expect("daemon");
}

/// A Rust source file of the copy, the same on each run.
fn pick_file(root: &Path) -> String {
    for candidate in [
        "crates/timonier-core/src/lib.rs",
        "src/lib.rs",
        "src/main.rs",
    ] {
        if root.join(candidate).is_file() {
            return candidate.to_owned();
        }
    }
    panic!("no Rust file to edit in {}", root.display());
}

fn percentile(values: &mut [Duration], p: usize) -> Duration {
    values.sort();
    let index = (values.len() * p / 100).min(values.len() - 1);
    values[index]
}

fn ms(duration: Duration) -> String {
    format!("{:.1} ms", duration.as_secs_f64() * 1000.0)
}

fn copy_repository(from: &Path, to: &Path) -> usize {
    let mut copied = 0;
    let walker = ignore::WalkBuilder::new(from)
        .hidden(false)
        .require_git(false)
        .filter_entry(|e| e.file_name() != ".git" && e.file_name() != ".timonier")
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
