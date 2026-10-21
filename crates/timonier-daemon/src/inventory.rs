//! The technology inventory (ADR 0055): the dependencies read in the lockfiles (`cargo
//! metadata`, else `Cargo.lock`; `package-lock.json`), the languages of the graph, the
//! frameworks and functions of the packs, what was added since the review, and the known
//! vulnerabilities (`osv`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use serde_json::Value;
use timonier_core::inventory::{
    Added, Dependency, Ecosystem, GetTechInventory, Inventory, LanguageCount, Scope,
};
use timonier_index::pack::InventoryRules;

/// What the lockfiles of the repository say: its own packages and their dependencies.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lockfiles {
    /// The repository's packages: name, version, ecosystem.
    pub packages: Vec<(String, Option<String>, Ecosystem)>,
    pub dependencies: Vec<Dependency>,
    pub notes: Vec<String>,
}

/// Reads the lockfiles under `root`, and gives each dependency its function from the packs.
pub fn lockfiles(root: &Path, rules: &BTreeMap<Ecosystem, InventoryRules>) -> Lockfiles {
    let mut out = Lockfiles::default();
    if root.join("Cargo.toml").is_file() {
        let metadata = Command::new("cargo")
            .args(["metadata", "--format-version", "1", "--offline"])
            .current_dir(root)
            .output();
        match metadata {
            Ok(output) if output.status.success() => {
                match serde_json::from_slice::<Value>(&output.stdout) {
                    Ok(json) => merge(&mut out, cargo_metadata(&json)),
                    Err(error) => out
                        .notes
                        .push(format!("cargo metadata did not read: {error}")),
                }
            }
            _ => match std::fs::read_to_string(root.join("Cargo.lock")) {
                Ok(text) => {
                    out.notes.push(
                        "cargo metadata failed (no cargo, or a dependency not downloaded): \
                         versions from Cargo.lock, licences unknown"
                            .to_owned(),
                    );
                    merge(&mut out, cargo_lock(&text, &cargo_members(root)));
                }
                Err(_) => out.notes.push(
                    "Cargo.toml without Cargo.lock: run `cargo generate-lockfile`".to_owned(),
                ),
            },
        }
    }
    if root.join("package.json").is_file() {
        match std::fs::read_to_string(root.join("package-lock.json")) {
            Ok(text) => match serde_json::from_str::<Value>(&text) {
                Ok(json) => merge(&mut out, npm_lock(&json)),
                Err(error) => out
                    .notes
                    .push(format!("package-lock.json did not read: {error}")),
            },
            Err(_) => out
                .notes
                .push("package.json without package-lock.json: run `npm install`".to_owned()),
        }
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

fn merge(into: &mut Lockfiles, from: Lockfiles) {
    into.packages.extend(from.packages);
    into.dependencies.extend(from.dependencies);
    into.notes.extend(from.notes);
}

/// The narrowest scope among a dependency's declarations: normal wins over build over dev.
fn widest(a: Scope, b: Scope) -> Scope {
    a.min(b)
}

/// What a crate's declarations give: per dependency name, its scope and who declares it.
type Declared = BTreeMap<String, (Scope, BTreeSet<String>)>;

fn declare(declared: &mut Declared, name: &str, scope: Scope, by: &str) {
    let entry = declared
        .entry(name.to_owned())
        .or_insert((scope, BTreeSet::new()));
    entry.0 = widest(entry.0, scope);
    entry.1.insert(by.to_owned());
}

/// `cargo metadata --format-version 1`: the workspace members, and every package of the
/// resolution with its licence.
pub fn cargo_metadata(json: &Value) -> Lockfiles {
    let members: BTreeSet<&str> = json["workspace_members"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let mut out = Lockfiles::default();
    let mut declared = Declared::new();
    let packages = json["packages"].as_array().cloned().unwrap_or_default();
    for package in &packages {
        let id = package["id"].as_str().unwrap_or_default();
        let name = package["name"].as_str().unwrap_or_default();
        if !members.contains(id) {
            continue;
        }
        out.packages.push((
            name.to_owned(),
            package["version"].as_str().map(str::to_owned),
            Ecosystem::Cargo,
        ));
        for dependency in package["dependencies"].as_array().into_iter().flatten() {
            let scope = match dependency["kind"].as_str() {
                Some("dev") => Scope::Dev,
                Some("build") => Scope::Build,
                _ => Scope::Normal,
            };
            if let Some(dep) = dependency["name"].as_str() {
                declare(&mut declared, dep, scope, name);
            }
        }
    }
    let own: BTreeSet<&str> = out.packages.iter().map(|(n, _, _)| n.as_str()).collect();
    for package in &packages {
        let id = package["id"].as_str().unwrap_or_default();
        let name = package["name"].as_str().unwrap_or_default();
        // The repository's own crates, and path dependencies of it, are no third parties.
        if members.contains(id) || package["source"].is_null() || own.contains(name) {
            continue;
        }
        let (scope, used_by) = declared
            .get(name)
            .cloned()
            .unwrap_or((Scope::Normal, BTreeSet::new()));
        out.dependencies.push(Dependency {
            ecosystem: Ecosystem::Cargo,
            name: name.to_owned(),
            version: package["version"].as_str().map(str::to_owned),
            license: package["license"].as_str().map(str::to_owned),
            direct: !used_by.is_empty(),
            scope,
            used_by: used_by.into_iter().collect(),
            function: None,
        });
    }
    out
}

/// The crates of the workspace and what each declares, read in their manifests: for
/// `Cargo.lock` alone, which says neither.
fn cargo_members(root: &Path) -> Declared {
    let mut declared = Declared::new();
    let Ok(text) = std::fs::read_to_string(root.join("Cargo.toml")) else {
        return declared;
    };
    let Ok(manifest) = text.parse::<toml::Table>() else {
        return declared;
    };
    let mut manifests = vec![manifest.clone()];
    for member in manifest
        .get("workspace")
        .and_then(|w| w.get("members"))
        .and_then(|m| m.as_array())
        .into_iter()
        .flatten()
        .filter_map(|m| m.as_str())
    {
        // Members by path; a glob (`crates/*`) is read as its folder's children.
        let paths: Vec<std::path::PathBuf> = match member.strip_suffix("/*") {
            Some(folder) => std::fs::read_dir(root.join(folder))
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .collect(),
            None => vec![root.join(member)],
        };
        for path in paths {
            if let Some(member) = std::fs::read_to_string(path.join("Cargo.toml"))
                .ok()
                .and_then(|t| t.parse::<toml::Table>().ok())
            {
                manifests.push(member);
            }
        }
    }
    for manifest in &manifests {
        let Some(name) = manifest
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
        else {
            continue;
        };
        for (section, scope) in [
            ("dependencies", Scope::Normal),
            ("dev-dependencies", Scope::Dev),
            ("build-dependencies", Scope::Build),
        ] {
            for (dep, spec) in manifest
                .get(section)
                .and_then(|d| d.as_table())
                .into_iter()
                .flatten()
            {
                // A renamed dependency (`package = "…"`) is the package's name.
                let real = spec.get("package").and_then(|p| p.as_str()).unwrap_or(dep);
                declare(&mut declared, real, scope, name);
            }
        }
        declare(&mut declared, &format!("\u{0}{name}"), Scope::Normal, name);
    }
    declared
}

/// `Cargo.lock`: every package of the resolution, versions without licences; `declared` from
/// the manifests says which are direct (and which are the workspace's, under `\0name`).
pub fn cargo_lock(text: &str, declared: &Declared) -> Lockfiles {
    let mut out = Lockfiles::default();
    let Ok(lock) = text.parse::<toml::Table>() else {
        out.notes.push("Cargo.lock did not read".to_owned());
        return out;
    };
    for package in lock
        .get("package")
        .and_then(|p| p.as_array())
        .into_iter()
        .flatten()
    {
        let name = package
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or_default();
        let version = package
            .get("version")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        if declared.contains_key(&format!("\u{0}{name}")) {
            out.packages
                .push((name.to_owned(), version, Ecosystem::Cargo));
            continue;
        }
        if package.get("source").is_none() {
            continue;
        }
        let (scope, used_by) = declared
            .get(name)
            .cloned()
            .unwrap_or((Scope::Normal, BTreeSet::new()));
        out.dependencies.push(Dependency {
            ecosystem: Ecosystem::Cargo,
            name: name.to_owned(),
            version,
            license: None,
            direct: !used_by.is_empty(),
            scope,
            used_by: used_by.into_iter().collect(),
            function: None,
        });
    }
    out
}

/// `package-lock.json`, versions 2 and 3: the workspaces (keys outside `node_modules/`) with
/// what they declare, and each installed package with its version and licence.
pub fn npm_lock(json: &Value) -> Lockfiles {
    let mut out = Lockfiles::default();
    let Some(entries) = json["packages"].as_object() else {
        out.notes.push(
            "package-lock.json has no `packages` (lockfile version 1): run `npm install`"
                .to_owned(),
        );
        return out;
    };
    let mut declared = Declared::new();
    for (key, entry) in entries {
        if key.contains("node_modules/") {
            continue;
        }
        let name = entry["name"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| {
                if key.is_empty() {
                    "root".to_owned()
                } else {
                    key.clone()
                }
            });
        out.packages.push((
            name.clone(),
            entry["version"].as_str().map(str::to_owned),
            Ecosystem::Npm,
        ));
        for (section, scope) in [
            ("dependencies", Scope::Normal),
            ("optionalDependencies", Scope::Normal),
            ("peerDependencies", Scope::Normal),
            ("devDependencies", Scope::Dev),
        ] {
            for dep in entry[section]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(d, _)| d)
            {
                declare(&mut declared, dep, scope, &name);
            }
        }
    }
    let own: BTreeSet<String> = out.packages.iter().map(|(n, _, _)| n.clone()).collect();
    for (key, entry) in entries {
        let Some(at) = key.rfind("node_modules/") else {
            continue;
        };
        let name = &key[at + "node_modules/".len()..];
        // A workspace, linked into node_modules: one of the repository's own.
        if entry["link"].as_bool() == Some(true) || own.contains(name) {
            continue;
        }
        // Nested (`a/node_modules/b`): pulled by `a`, never declared by the repository.
        let nested = key[..at].contains("node_modules/");
        let (scope, used_by) = if nested {
            (Scope::Normal, BTreeSet::new())
        } else {
            declared
                .get(name)
                .cloned()
                .unwrap_or((Scope::Normal, BTreeSet::new()))
        };
        out.dependencies.push(Dependency {
            ecosystem: Ecosystem::Npm,
            name: name.to_owned(),
            version: entry["version"].as_str().map(str::to_owned),
            license: entry["license"].as_str().map(str::to_owned),
            direct: !used_by.is_empty(),
            scope: if used_by.is_empty() && entry["dev"].as_bool() == Some(true) {
                Scope::Dev
            } else {
                scope
            },
            used_by: used_by.into_iter().collect(),
            function: None,
        });
    }
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
            let ecosystem = match pack.manifest.ecosystem {
                timonier_index::pack::Ecosystem::Cargo => Ecosystem::Cargo,
                timonier_index::pack::Ecosystem::Npm => Ecosystem::Npm,
            };
            out.insert(ecosystem, pack.manifest.inventory.clone());
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
    let graph = crate::view::load(&mut reader)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_metadata_gives_the_members_and_their_dependencies() {
        let json = serde_json::json!({
            "workspace_members": ["path+file:///r#app@0.1.0"],
            "packages": [
                {"id": "path+file:///r#app@0.1.0", "name": "app", "version": "0.1.0", "source": null,
                 "license": "MIT", "dependencies": [
                    {"name": "serde", "kind": null}, {"name": "tempfile", "kind": "dev"},
                    {"name": "cc", "kind": "build"}, {"name": "serde", "kind": "dev"}]},
                {"id": "registry+…#serde@1.0.0", "name": "serde", "version": "1.0.0",
                 "source": "registry+https://github.com/rust-lang/crates.io-index", "license": "MIT OR Apache-2.0",
                 "dependencies": []},
                {"id": "registry+…#tempfile@3.0.0", "name": "tempfile", "version": "3.0.0",
                 "source": "registry+…", "license": "MIT OR Apache-2.0", "dependencies": []},
                {"id": "registry+…#cc@1.0.0", "name": "cc", "version": "1.0.0",
                 "source": "registry+…", "license": null, "dependencies": []},
                {"id": "registry+…#itoa@1.0.0", "name": "itoa", "version": "1.0.0",
                 "source": "registry+…", "license": "MIT", "dependencies": []}
            ]
        });
        let read = cargo_metadata(&json);
        assert_eq!(
            read.packages,
            [("app".to_owned(), Some("0.1.0".to_owned()), Ecosystem::Cargo)]
        );
        let of = |name: &str| read.dependencies.iter().find(|d| d.name == name).unwrap();
        // Declared for the code and for the tests: it ships.
        assert_eq!(
            (of("serde").direct, of("serde").scope),
            (true, Scope::Normal)
        );
        assert_eq!(of("tempfile").scope, Scope::Dev);
        assert_eq!(of("cc").scope, Scope::Build);
        assert_eq!(of("cc").license, None);
        assert!(!of("itoa").direct);
        assert_eq!(of("serde").used_by, ["app"]);
    }

    #[test]
    fn cargo_lock_alone_gives_versions_without_licences() {
        let mut declared = Declared::new();
        declare(&mut declared, "\u{0}app", Scope::Normal, "app");
        declare(&mut declared, "serde", Scope::Normal, "app");
        let lock = r#"
version = 4

[[package]]
name = "app"
version = "0.1.0"

[[package]]
name = "serde"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "itoa"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#;
        let read = cargo_lock(lock, &declared);
        assert_eq!(read.packages.len(), 1);
        assert_eq!(read.dependencies.len(), 2);
        assert!(read.dependencies.iter().all(|d| d.license.is_none()));
        assert!(
            read.dependencies
                .iter()
                .any(|d| d.name == "serde" && d.direct)
        );
    }

    #[test]
    fn package_lock_gives_the_workspaces_and_their_dependencies() {
        let json = serde_json::json!({
            "lockfileVersion": 3,
            "packages": {
                "": {"name": "root", "workspaces": ["web"], "devDependencies": {"vitest": "^4"}},
                "web": {"name": "web", "version": "1.0.0", "dependencies": {"react": "^19", "axios": "^1"}},
                "node_modules/web": {"resolved": "web", "link": true},
                "node_modules/react": {"version": "19.3.0", "license": "MIT"},
                "node_modules/axios": {"version": "1.7.0", "license": "MIT"},
                "node_modules/vitest": {"version": "4.1.0", "license": "MIT", "dev": true},
                "node_modules/loose-envify": {"version": "1.4.0", "license": "MIT"},
                "node_modules/axios/node_modules/react": {"version": "18.0.0", "license": "MIT"}
            }
        });
        let read = npm_lock(&json);
        assert_eq!(read.packages.len(), 2);
        let all: Vec<(&str, Option<&str>, bool, Scope)> = read
            .dependencies
            .iter()
            .map(|d| (d.name.as_str(), d.version.as_deref(), d.direct, d.scope))
            .collect();
        assert!(
            all.contains(&("react", Some("19.3.0"), true, Scope::Normal)),
            "{all:?}"
        );
        assert!(
            all.contains(&("vitest", Some("4.1.0"), true, Scope::Dev)),
            "{all:?}"
        );
        assert!(
            all.contains(&("loose-envify", Some("1.4.0"), false, Scope::Normal)),
            "{all:?}"
        );
        // The nested copy is pulled by axios, not declared.
        assert!(
            all.contains(&("react", Some("18.0.0"), false, Scope::Normal)),
            "{all:?}"
        );
        assert!(!all.iter().any(|(n, ..)| *n == "web"));
    }
}
