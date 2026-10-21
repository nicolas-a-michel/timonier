//! `timonier inventory` (ADR 0055): the technology inventory in a few lines, and the CycloneDX
//! SBOM written where `--sbom` says. The daemon reads the lockfiles and asks OSV.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Args;
use timonier_core::inventory::{CveCheck, GetTechInventory, Inventory};
use timonier_core::protocol::{StatusResult, methods};

use crate::daemon;

#[derive(Debug, Args)]
pub struct InventoryArgs {
    /// Write the CycloneDX 1.5 SBOM there
    #[arg(long)]
    sbom: Option<PathBuf>,
    /// Do not ask OSV.dev for known vulnerabilities; a fresh cache is still read (Q79)
    #[arg(long)]
    no_cve: bool,
    #[command(flatten)]
    target: daemon::Target,
}

const CLIENT: &str = "timonier-inventory";

/// How long the first indexing may take before the inventory reads the graph.
const INDEXING: Duration = Duration::from_secs(60);

pub fn run(args: &InventoryArgs) -> ExitCode {
    match inventory(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("timonier: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn inventory(args: &InventoryArgs) -> Result<()> {
    let paths = daemon::resolve(&args.target)?;
    daemon::launch(&paths)?;
    let mut client = daemon::connect_as(&paths, CLIENT).context("the daemon does not answer")?;
    // OSV may take a while: 50 details at most, 20 s each at worst.
    client.set_timeout(Duration::from_secs(120));
    let deadline = Instant::now() + INDEXING;
    loop {
        let status: StatusResult = client.request(methods::STATUS, &serde_json::Value::Null)?;
        if status.indexed || Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let input = GetTechInventory {
        cve: args.no_cve.then_some(false),
        limit: Some(1),
        cursor: None,
    };
    let inventory: Inventory = client.request(methods::INVENTORY, &input)?;
    print!("{}", words(&inventory));
    if let Some(path) = &args.sbom {
        let sbom: serde_json::Value = client.request(methods::INVENTORY_SBOM, &input)?;
        let mut text = serde_json::to_string_pretty(&sbom)?;
        text.push('\n');
        std::fs::write(path, text).with_context(|| format!("cannot write {}", path.display()))?;
        println!("SBOM (CycloneDX 1.5): {}", path.display());
    }
    Ok(())
}

/// The inventory in a few lines, as a terminal shows it.
fn words(inventory: &Inventory) -> String {
    let mut out = String::new();
    let languages: Vec<String> = inventory
        .languages
        .iter()
        .map(|l| format!("{} ({} files)", l.language, l.files))
        .collect();
    out.push_str(&format!("Languages: {}\n", languages.join(", ")));
    if !inventory.frameworks.is_empty() {
        out.push_str(&format!(
            "Frameworks: {}\n",
            inventory.frameworks.join(", ")
        ));
    }
    let counts: Vec<String> = inventory
        .counts
        .iter()
        .map(|(e, n)| format!("{n} {e}"))
        .collect();
    out.push_str(&format!("Dependencies: {}\n", counts.join(", ")));
    let mut licenses: Vec<(&String, &u32)> = inventory.licenses.iter().collect();
    licenses.sort_by(|a, b| b.1.cmp(a.1));
    let licenses: Vec<String> = licenses
        .iter()
        .take(6)
        .map(|(l, n)| format!("{l} {n}"))
        .collect();
    out.push_str(&format!("Licences: {}\n", licenses.join(", ")));
    for duplicate in &inventory.duplicates {
        out.push_str(&format!(
            "Duplicate ({}): {}, in {}\n",
            duplicate.function,
            duplicate.packages.join(" and "),
            duplicate.used_by.join(", ")
        ));
    }
    if !inventory.added_since_review.is_empty() {
        let added: Vec<String> = inventory
            .added_since_review
            .iter()
            .map(|a| format!("{} ({})", a.name, a.by))
            .collect();
        out.push_str(&format!(
            "Added {}: {}\n",
            if inventory.reviewed {
                "since the review"
            } else {
                "(no review yet)"
            },
            added.join(", ")
        ));
    }
    match &inventory.cve {
        CveCheck::Checked { .. } if inventory.vulnerabilities.is_empty() => {
            out.push_str("Known vulnerabilities: none (OSV.dev)\n");
        }
        CveCheck::Checked { .. } => {
            for v in &inventory.vulnerabilities {
                out.push_str(&format!(
                    "Vulnerability {} in {} {}: {}\n",
                    v.id,
                    v.package,
                    v.version,
                    v.summary.as_deref().unwrap_or("")
                ));
            }
        }
        CveCheck::NotChecked { reason } => {
            out.push_str(&format!("Known vulnerabilities: not checked ({reason})\n"));
        }
    }
    for note in &inventory.notes {
        out.push_str(&format!("Note: {note}\n"));
    }
    out
}
