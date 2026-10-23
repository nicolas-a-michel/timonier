//! The technology inventory (ADR 0055): the dependencies of the repository with their versions
//! and licences, its languages and frameworks, the duplicates by function, what was added since
//! the review, and the known vulnerabilities. Pure: the daemon reads the lockfiles and asks OSV.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `get_tech_inventory`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetTechInventory {
    /// `false`: no query to OSV for known vulnerabilities (Q79); the cache is still read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cve: Option<bool>,
    /// How many dependencies the page lists (100 by default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1, max = 500))]
    pub limit: Option<u32>,
    /// Where the previous page ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 32))]
    pub cursor: Option<String>,
}

/// A package ecosystem the packs read (ADR 0055, 0077), by its name (`cargo`, `npm`): what
/// `timonier-index` gives, with its rank, the order the packs list them, which orders the
/// inventory. Read back from JSON, it has its name only and ranks after the others: one read
/// back and one of the packs are never compared (review of Phase 11, m6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ecosystem {
    rank: u16,
    name: &'static str,
}

impl Ecosystem {
    pub const fn new(name: &'static str, rank: u16) -> Self {
        Self { rank, name }
    }

    pub const fn name(self) -> &'static str {
        self.name
    }
}

impl std::fmt::Display for Ecosystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name)
    }
}

impl Serialize for Ecosystem {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name)
    }
}

impl<'de> Deserialize<'de> for Ecosystem {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        Ok(Self::new(intern(name), u16::MAX))
    }
}

/// The ecosystems' names read back, kept once each: a handful at most.
fn intern(name: String) -> &'static str {
    static NAMES: std::sync::Mutex<BTreeSet<&'static str>> = std::sync::Mutex::new(BTreeSet::new());
    let mut names = NAMES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(known) = names.get(name.as_str()) {
        return known;
    }
    let kept: &'static str = Box::leak(name.into_boxed_str());
    names.insert(kept);
    kept
}

/// What a dependency is for, as the manifest declares it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Normal,
    Dev,
    Build,
}

/// A dependency, resolved by the lockfile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    pub ecosystem: Ecosystem,
    pub name: String,
    /// `None`: the lockfile does not say (a manifest without lockfile).
    pub version: Option<String>,
    /// SPDX expression; `None` when unknown.
    pub license: Option<String>,
    /// Declared by a package of the repository, not only pulled by another dependency.
    pub direct: bool,
    /// Its narrowest use among the declarations: `normal` when any package needs it to run.
    pub scope: Scope,
    /// The packages of the repository that declare it.
    pub used_by: Vec<String>,
    /// Its function, as the pack knows it (`http-client`…).
    pub function: Option<String>,
}

/// Two dependencies or more serving one function (spec: « two libraries for the same thing »).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Duplicate {
    pub ecosystem: Ecosystem,
    pub function: String,
    pub packages: Vec<String>,
    /// The packages of the repository that declare them.
    pub used_by: Vec<String>,
}

/// A dependency a package of the repository declares since the last review (ADR 0024).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Added {
    pub name: String,
    pub by: String,
}

/// A known vulnerability of a dependency, from OSV.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vulnerability {
    pub id: String,
    pub ecosystem: Ecosystem,
    pub package: String,
    pub version: String,
    pub aliases: Vec<String>,
    pub summary: Option<String>,
    /// The advisory's severity word (`HIGH`), else its CVSS vector.
    pub severity: Option<String>,
}

/// Whether the known vulnerabilities were looked for (Q79).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CveCheck {
    /// OSV answered, at that time (milliseconds since the epoch), now or from the cache.
    Checked { at_ms: i64, cached: bool },
    /// Not looked for: why. The vulnerabilities are unknown, not absent.
    NotChecked { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageCount {
    pub language: String,
    pub files: u32,
}

/// What `get_tech_inventory` answers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory {
    pub languages: Vec<LanguageCount>,
    pub frameworks: Vec<String>,
    /// Dependencies by ecosystem, all of them, direct or not.
    pub counts: BTreeMap<String, u32>,
    /// Dependencies by licence; `unknown` without one.
    pub licenses: BTreeMap<String, u32>,
    pub duplicates: Vec<Duplicate>,
    /// Since the last review; every direct dependency before the first one.
    pub added_since_review: Vec<Added>,
    pub reviewed: bool,
    pub vulnerabilities: Vec<Vulnerability>,
    pub cve: CveCheck,
    /// A page of the dependencies, direct ones first.
    pub dependencies: Vec<Dependency>,
    pub total: u32,
    pub next_cursor: Option<String>,
    /// What could not be read, in words: a lockfile missing, `cargo` absent.
    pub notes: Vec<String>,
}

/// The packages serving one function, and who declares them.
type Serving<'a> = (BTreeSet<&'a str>, BTreeSet<&'a str>);

/// The duplicates among the direct dependencies that ship (normal scope), by function.
pub fn duplicates(dependencies: &[Dependency]) -> Vec<Duplicate> {
    let mut by_function: BTreeMap<(Ecosystem, &str), Serving<'_>> = BTreeMap::new();
    for dependency in dependencies
        .iter()
        .filter(|d| d.direct && d.scope == Scope::Normal)
    {
        let Some(function) = dependency.function.as_deref() else {
            continue;
        };
        let entry = by_function
            .entry((dependency.ecosystem, function))
            .or_default();
        entry.0.insert(&dependency.name);
        entry
            .1
            .extend(dependency.used_by.iter().map(String::as_str));
    }
    by_function
        .into_iter()
        .filter(|(_, (packages, _))| packages.len() > 1)
        .map(|((ecosystem, function), (packages, used_by))| Duplicate {
            ecosystem,
            function: function.to_owned(),
            packages: packages.into_iter().map(str::to_owned).collect(),
            used_by: used_by.into_iter().map(str::to_owned).collect(),
        })
        .collect()
}

/// Dependencies by licence, `unknown` without one.
pub fn licenses(dependencies: &[Dependency]) -> BTreeMap<String, u32> {
    let mut out = BTreeMap::new();
    for dependency in dependencies {
        *out.entry(
            dependency
                .license
                .clone()
                .unwrap_or_else(|| "unknown".to_owned()),
        )
        .or_insert(0) += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const CARGO: Ecosystem = Ecosystem::new("cargo", 0);

    fn dependency(name: &str, function: Option<&str>, scope: Scope, direct: bool) -> Dependency {
        Dependency {
            ecosystem: CARGO,
            name: name.to_owned(),
            version: Some("1.0.0".to_owned()),
            license: (name != "mystery").then(|| "MIT".to_owned()),
            direct,
            scope,
            used_by: vec![format!("app-{name}")],
            function: function.map(str::to_owned),
        }
    }

    #[test]
    fn two_direct_dependencies_of_one_function_are_a_duplicate() {
        let deps = [
            dependency("reqwest", Some("http-client"), Scope::Normal, true),
            dependency("ureq", Some("http-client"), Scope::Normal, true),
            // Pulled by another dependency, or for the tests only: not the project's choice.
            dependency("isahc", Some("http-client"), Scope::Normal, false),
            dependency("attohttpc", Some("http-client"), Scope::Dev, true),
            dependency("tokio", Some("async-runtime"), Scope::Normal, true),
            dependency("mystery", None, Scope::Normal, true),
        ];
        assert_eq!(
            duplicates(&deps),
            [Duplicate {
                ecosystem: CARGO,
                function: "http-client".to_owned(),
                packages: vec!["reqwest".to_owned(), "ureq".to_owned()],
                used_by: vec!["app-reqwest".to_owned(), "app-ureq".to_owned()],
            }]
        );
        let licenses = licenses(&deps);
        assert_eq!(licenses["MIT"], 5);
        assert_eq!(licenses["unknown"], 1);
    }

    #[test]
    fn the_same_function_in_two_ecosystems_is_no_duplicate() {
        let mut npm = dependency("axios", Some("http-client"), Scope::Normal, true);
        npm.ecosystem = Ecosystem::new("npm", 1);
        let deps = [
            dependency("reqwest", Some("http-client"), Scope::Normal, true),
            npm,
        ];
        assert!(duplicates(&deps).is_empty());
    }

    #[test]
    fn an_ecosystem_is_written_by_its_name_and_ordered_by_its_rank() {
        let json = serde_json::to_string(&[CARGO, Ecosystem::new("composer", 9)]).unwrap();
        assert_eq!(json, r#"["cargo","composer"]"#);
        assert!(CARGO < Ecosystem::new("composer", 9));
        let back: Vec<Ecosystem> = serde_json::from_str(&json).unwrap();
        let names: Vec<&str> = back.iter().map(|e| e.name()).collect();
        assert_eq!(names, ["cargo", "composer"]);
    }
}
