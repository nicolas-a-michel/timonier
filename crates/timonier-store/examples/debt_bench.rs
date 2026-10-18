//! The cost of a refinement step's debt write (ADR 0022, review n° 10, m10-2): debts of
//! growing size, one lookup changing at each step, the difference computed and written.
//!
//! `cargo run --release -p timonier-store --example debt_bench`

use std::time::Instant;

use timonier_core::{ContentHash, Position};
use timonier_store::{PendingLookup, RefinementDebt, Store};

/// About ten rows per lookup, its citers, as on Timonier (8.7 citers on average).
fn debt(rows: usize, generation: u8) -> RefinementDebt {
    let lookups = rows / 10;
    RefinementDebt {
        dependents: (0..lookups).map(|i| format!("src/dep{i}.rs")).collect(),
        lookups: (0..lookups)
            .map(|i| PendingLookup {
                path: format!("src/f{i}.rs"),
                symbol: format!("rust:demo::f{i}::s"),
                at: Position::new(1, 7),
                content_hash: ContentHash::from_bytes([if i == 0 { generation } else { 0 }; 32]),
                by_name: (0..8).map(|c| format!("src/c{c}.rs")).collect(),
            })
            .collect(),
    }
}

fn main() {
    println!("{:>8} {:>16} {:>12}", "rows", "step p50 (ms)", "max (ms)");
    for rows in [110, 1_100, 11_000, 55_000] {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = Store::open(dir.path().join("graph.sqlite")).expect("store");
        let mut owed = debt(rows, 1);
        store
            .update_refinement_debt(owed.changes_from(&RefinementDebt::default()))
            .expect("first write");
        let mut times = Vec::new();
        for step in 0..20 {
            let next = debt(rows, 2 + step);
            let start = Instant::now();
            store
                .update_refinement_debt(next.changes_from(&owed))
                .expect("write");
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            owed = next;
        }
        times.sort_by(f64::total_cmp);
        println!("{rows:>8} {:>16.3} {:>12.3}", times[10], times[19]);
    }
}
