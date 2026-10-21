//! `timonier check` (ADR 0056): what the CI replays after the tests — the whiteboard's budgets on
//! the last runs (ADR 0052) and its architecture rules (ADR 0042): forbidden edges crossed,
//! required edges missing. The exit status says whether all holds.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Args;
use timonier_core::budgets::{BudgetState, BudgetsView};
use timonier_core::protocol::{StatusResult, methods};

use crate::daemon;

#[derive(Debug, Args)]
pub struct CheckArgs {
    #[command(flatten)]
    target: daemon::Target,
}

const CLIENT: &str = "timonier-check";

/// How long the first indexing may take before the rules read the graph.
const INDEXING: Duration = Duration::from_secs(120);

pub fn run(args: &CheckArgs) -> ExitCode {
    match check(args) {
        Ok(broken) if broken.is_empty() => {
            println!("timonier: the budgets and the architecture rules hold");
            ExitCode::SUCCESS
        }
        Ok(broken) => {
            for line in &broken {
                eprintln!("timonier: {line}");
            }
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("timonier: {error:#}");
            ExitCode::from(2)
        }
    }
}

/// What does not hold, one line each.
fn check(args: &CheckArgs) -> Result<Vec<String>> {
    let paths = daemon::resolve(&args.target)?;
    daemon::launch(&paths)?;
    let mut client = daemon::connect_as(&paths, CLIENT).context("the daemon does not answer")?;
    let deadline = Instant::now() + INDEXING;
    loop {
        let status: StatusResult = client.request(methods::STATUS, &serde_json::Value::Null)?;
        if status.indexed {
            break;
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "the project is still being indexed"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    let mut broken = Vec::new();
    let budgets: Option<BudgetsView> =
        client.request(methods::PERF_BUDGETS, &serde_json::Value::Null)?;
    for check in budgets.map(|b| b.checks).unwrap_or_default() {
        if check.state != BudgetState::Exceeded {
            continue;
        }
        let over: Vec<String> = check
            .limits
            .iter()
            .filter(|l| l.exceeded)
            .map(|l| {
                let kind = serde_json::to_value(l.kind).unwrap_or_default();
                format!(
                    "{} {} > {}",
                    kind.as_str().unwrap_or("limit"),
                    l.measured.unwrap_or_default(),
                    l.limit
                )
            })
            .collect();
        broken.push(format!(
            "budget {} exceeded on {}: {}",
            check.id,
            check.on,
            over.join(", ")
        ));
    }
    let drift: serde_json::Value = client.request(
        methods::TOOLS_CALL,
        &serde_json::json!({ "name": "get_drift", "arguments": {} }),
    )?;
    let drift = &drift["structuredContent"]["result"];
    for (key, what) in [
        ("forbidden", "forbidden edge crossed"),
        ("required", "required edge missing"),
    ] {
        for crossing in drift[key].as_array().into_iter().flatten() {
            broken.push(format!(
                "{what}: {} ({} → {})",
                crossing["edge"].as_str().unwrap_or("?"),
                crossing["src"].as_str().unwrap_or("?"),
                crossing["dst"].as_str().unwrap_or("?"),
            ));
        }
    }
    Ok(broken)
}
