//! The technology inventory (ADR 0055): the dependencies read in the lockfiles (`cargo
//! metadata`, else `Cargo.lock`; `package-lock.json`), the languages of the graph, the
//! frameworks and functions of the packs, what was added since the review, and the known
//! vulnerabilities (`osv`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use timonier_core::inventory::{
    Added, Dependency, Ecosystem, GetTechInventory, Inventory, LanguageCount, Scope,
};
use timonier_index::pack::InventoryRules;

pub use timonier_index::tooling::Lockfiles;

/// Reads the lockfiles under `root`, and gives each dependency its function from the packs.
pub fn lockfiles(root: &Path, rules: &BTreeMap<Ecosystem, InventoryRules>) -> Lockfiles {
    // The tools some ecosystems ask (`cargo metadata`): the daemon runs them for the index.
    let run = |dir: &Path, program: &str, args: &[&str]| {
        Command::new(program)
            .args(args)
            .current_dir(dir)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| output.stdout)
    };
    let mut out = Lockfiles::default();
    for tooling in timonier_index::tooling::all() {
        out.merge(tooling.dependencies(root, &run));
    }
    for dependency in &mut out.dependencies {
        dependency.function = rules.get(&dependency.ecosystem).and_then(|r| {
            r.functions
                .iter()
                .find(|(_, packages)| packages.contains(&dependency.name))
                .map(|(function, _)| function.clone())
        });
    }
    out.dependencies.sort_by(|a, b| {
        (!a.direct, a.ecosystem, &a.name, &a.version).cmp(&(
            !b.direct,
            b.ecosystem,
            &b.name,
            &b.version,
        ))
    });
    out
}

/// The languages of the graph, by number of files, the most first.
pub fn languages(graph: &timonier_core::graph::Graph) -> Vec<LanguageCount> {
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    for node in graph.nodes() {
        if node.kind == timonier_core::NodeKind::File
            && let Some(language) = &node.language
        {
            *counts.entry(language.as_str().to_owned()).or_insert(0) += 1;
        }
    }
    let mut out: Vec<LanguageCount> = counts
        .into_iter()
        .map(|(language, files)| LanguageCount { language, files })
        .collect();
    out.sort_by(|a, b| {
        b.files
            .cmp(&a.files)
            .then_with(|| a.language.cmp(&b.language))
    });
    out
}

/// The direct dependencies the packs call frameworks.
pub fn frameworks(
    dependencies: &[Dependency],
    rules: &BTreeMap<Ecosystem, InventoryRules>,
) -> Vec<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    for dependency in dependencies
        .iter()
        .filter(|d| d.direct && d.scope == Scope::Normal)
    {
        if rules
            .get(&dependency.ecosystem)
            .is_some_and(|r| r.frameworks.contains(&dependency.name))
        {
            out.insert(dependency.name.clone());
        }
    }
    out.into_iter().collect()
}

/// The external packages a package of the repository depends on since the review, by the
/// manifests' edges between the review mark and the head (ADR 0055); every one before a
/// first review.
pub fn added_since(
    reader: &mut timonier_store::Reader,
    review: Option<u64>,
) -> crate::Result<Vec<Added>> {
    use timonier_core::diff::EdgeChange;
    let head = reader.head()?.seq;
    let Some(from) = review.or_else(|| reader.snapshots().ok()?.first().map(|s| s.seq)) else {
        return Ok(Vec::new());
    };
    let name = |id: &timonier_core::NodeId| {
        id.as_str()
            .strip_prefix("pkg:")
            .and_then(|rest| rest.split_once('/'))
            .map(|(_, name)| name.to_owned())
    };
    let externals: BTreeSet<String> = reader
        .nodes_at(head)?
        .into_iter()
        .filter(|n| {
            n.kind == timonier_core::NodeKind::Package
                && n.parent
                    .as_ref()
                    .is_some_and(|p| p.as_str().starts_with("component:external"))
        })
        .map(|n| n.id.as_str().to_owned())
        .collect();
    let mut out: BTreeSet<(String, String)> = BTreeSet::new();
    let edges = if review.is_some() && from < head {
        reader
            .diff(from, head)?
            .edges
            .into_iter()
            .filter_map(|change| match change {
                EdgeChange::Added { edge } => Some(edge),
                _ => None,
            })
            .collect()
    } else if review.is_some() {
        Vec::new()
    } else {
        reader.edges_at(head)?
    };
    for edge in edges {
        if edge.kind != timonier_core::EdgeKind::DependsOn
            || edge.provenance != timonier_core::Provenance::Manifest
            || !externals.contains(edge.dst.as_str())
        {
            continue;
        }
        if let (Some(dep), Some(by)) = (name(&edge.dst), name(&edge.src)) {
            out.insert((dep, by));
        }
    }
    Ok(out
        .into_iter()
        .map(|(name, by)| Added { name, by })
        .collect())
}

/// The inventory rules of the built-in packs, by ecosystem.
pub fn rules() -> BTreeMap<Ecosystem, InventoryRules> {
    let mut out = BTreeMap::new();
    if let Ok(packs) = timonier_index::Packs::builtin() {
        for pack in packs.iter() {
            let Some(tooling) = timonier_index::tooling::get(pack.manifest.ecosystem.as_str())
            else {
                continue;
            };
            // Languages that share an ecosystem: their rules add up.
            for packages in tooling.packages() {
                let rules: &mut InventoryRules = out.entry(packages.ecosystem).or_default();
                let more = &pack.manifest.inventory;
                rules.frameworks.extend(more.frameworks.iter().cloned());
                for (function, packages) in &more.functions {
                    rules
                        .functions
                        .entry(function.clone())
                        .or_default()
                        .extend(packages.iter().cloned());
                }
            }
        }
    }
    out
}

/// Everything the inventory reads, before it is paged: for `get_tech_inventory` and the SBOM.
pub struct Gathered {
    pub lockfiles: Lockfiles,
    pub inventory: Inventory,
}

/// The default page of dependencies.
const PAGE: u32 = 100;

/// The inventory of the repository at `root` (ADR 0055): `ask` lets it query OSV (Q79).
pub fn gather(
    root: &Path,
    store: &timonier_store::Store,
    input: &GetTechInventory,
    now_ms: i64,
    fetch: crate::osv::Fetch<'_>,
) -> crate::Result<Gathered> {
    let rules = rules();
    let lockfiles = lockfiles(root, &rules);
    let mut reader = store.reader()?;
    let graph = crate::view::load(&mut reader, root)?;
    let review = reader
        .latest(timonier_core::SnapshotKind::ReviewMark)?
        .map(|mark| mark.seq);
    let added = added_since(&mut reader, review)?;
    let (vulnerabilities, cve) = crate::osv::check(
        &lockfiles.dependencies,
        &root.join(".timonier").join("cache").join("osv.json"),
        now_ms,
        input.cve != Some(false),
        fetch,
    );
    let mut counts = BTreeMap::new();
    for dependency in &lockfiles.dependencies {
        *counts
            .entry(format!("{:?}", dependency.ecosystem).to_lowercase())
            .or_insert(0) += 1;
    }
    let total = u32::try_from(lockfiles.dependencies.len()).unwrap_or(u32::MAX);
    let start: u32 = input
        .cursor
        .as_deref()
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    let limit = input.limit.unwrap_or(PAGE).clamp(1, 500);
    let page: Vec<Dependency> = lockfiles
        .dependencies
        .iter()
        .skip(start as usize)
        .take(limit as usize)
        .cloned()
        .collect();
    let end = start.saturating_add(limit);
    let inventory = Inventory {
        languages: languages(&graph),
        frameworks: frameworks(&lockfiles.dependencies, &rules),
        counts,
        licenses: timonier_core::inventory::licenses(&lockfiles.dependencies),
        duplicates: timonier_core::inventory::duplicates(&lockfiles.dependencies),
        added_since_review: added,
        reviewed: review.is_some(),
        vulnerabilities,
        cve,
        dependencies: page,
        total,
        next_cursor: (end < total).then(|| end.to_string()),
        notes: lockfiles.notes.clone(),
    };
    Ok(Gathered {
        lockfiles,
        inventory,
    })
}

/// One line for the agent: what the inventory found first.
pub fn summary(inventory: &Inventory) -> String {
    let languages: Vec<String> = inventory
        .languages
        .iter()
        .map(|l| format!("{} ({} files)", l.language, l.files))
        .collect();
    let cve = match &inventory.cve {
        timonier_core::inventory::CveCheck::Checked { .. } => {
            format!("{} known vulnerabilities", inventory.vulnerabilities.len())
        }
        timonier_core::inventory::CveCheck::NotChecked { .. } => {
            "vulnerabilities not checked".to_owned()
        }
    };
    format!(
        "{} dependencies; {}; {} duplicates by function; {} added {}; {cve}.",
        inventory.total,
        if languages.is_empty() {
            "no language".to_owned()
        } else {
            languages.join(", ")
        },
        inventory.duplicates.len(),
        inventory.added_since_review.len(),
        if inventory.reviewed {
            "since the review"
        } else {
            "(no review yet)"
        },
    )
}
