//! What the tools around an ecosystem need (ADR 0077): the dependencies its lockfiles give, for
//! the technology inventory (ADR 0055). One module per ecosystem of the packs, behind
//! [`Tooling`]; the daemon and the CLI go through it, and name no language.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use timonier_core::inventory::{Dependency, Ecosystem, Scope};

use text::Sdk;

mod cargo;
mod cmake;
mod composer;
mod gomod;
mod maven;
mod npm;
mod nuget;
pub mod ordered;
mod pypi;
mod sbt;
mod swiftpm;
pub mod text;

/// Runs a program in a folder: its standard output when it succeeded. The daemon gives it, so
/// that the index runs nothing itself.
pub type Run<'a> = dyn Fn(&Path, &str, &[&str]) -> Option<Vec<u8>> + 'a;

/// An ecosystem of the packs (`pack.toml`'s `ecosystem`), as the tools around it need it.
pub trait Tooling: Sync {
    /// Its name in `pack.toml`.
    fn name(&self) -> &'static str;
    /// The package ecosystems its dependencies belong to.
    fn packages(&self) -> &'static [Packages];
    /// What its lockfiles under `root` say.
    fn dependencies(&self, root: &Path, run: &Run<'_>) -> Lockfiles;
    /// The steps of the CI workflow `timonier init` writes (ADR 0056) that the project at
    /// `root` needs before the tests run, as GitHub Actions YAML lines.
    fn ci_steps(&self, _root: &Path) -> Option<&'static str> {
        None
    }
    /// What the project at `root` shows of the ecosystem, by its manifests (ADR 0034).
    fn detect(&self, root: &Path) -> Option<Found>;
    /// What a project without any ecosystem's manifest shows of this one, by its files alone.
    fn detect_alone(&self, _root: &Path) -> Option<Found> {
        None
    }
    /// Whether the project at `root` has what the ecosystem's test runner needs: its manifest.
    fn runs(&self, root: &Path) -> bool;
    /// The manifest that takes the OpenTelemetry SDK (ADR 0050), and the folder the SDK's
    /// files go under.
    fn sdk_place(&self, root: &Path) -> SdkPlace;
    /// The manifest at `manifest` with the SDK.
    fn with_sdk(
        &self,
        manifest: &str,
        text: &str,
        sdk: &[Sdk<'_>],
        feature: Option<&str>,
    ) -> Result<WithSdk, String>;
    /// The other files of the project that take the SDK, each with what it gives them: a
    /// package's manager (`manager` in the pack) chooses them.
    fn sdk_beside(&self, _root: &Path, _sdk: &[(Option<&str>, Sdk<'_>)]) -> Vec<Beside> {
        Vec::new()
    }
    /// What `init` says when the project has no manifest to take the SDK, after
    /// `OpenTelemetry (<language>): `.
    fn without_manifest(&self) -> Option<&'static str> {
        None
    }
    /// Timonier's part taken out of `path`, one of the manifests it edited, whose text was
    /// `original` before `init`: `None` when `path` is none of the ecosystem's.
    fn without_sdk(
        &self,
        path: &str,
        text: &str,
        original: &str,
    ) -> Option<Result<Option<String>, String>>;
}

/// What a project shows of an ecosystem: a language, its package manager, and, for a
/// `package.json`, its `test` script, which some runners look for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub language: &'static str,
    pub manager: &'static str,
    pub test_script: Option<String>,
}

/// Where the SDK goes: its manifest, from the root, and the folder (`app/`, or empty) its files
/// go under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdkPlace {
    pub manifest: String,
    pub under: String,
}

/// A manifest with the SDK, or why it takes none here, in words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WithSdk {
    Text(String),
    Elsewhere(&'static str),
}

/// Another file that takes the SDK, and its text with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Beside {
    pub path: String,
    pub text: Result<String, String>,
}

/// The order in which `detect` lists what the project shows.
static DETECTION: [&dyn Tooling; 10] = [
    &cargo::Cargo,
    &pypi::Pypi,
    &sbt::Sbt,
    &composer::Composer,
    &nuget::Nuget,
    &swiftpm::Swiftpm,
    &cmake::Cmake,
    &maven::Maven,
    &gomod::Gomod,
    &npm::Npm,
];

/// What the project at `root` shows: a language and a package manager per ecosystem, in a
/// stable order; by its files alone when no manifest is there.
pub fn detect(root: &Path) -> Vec<Found> {
    let found: Vec<Found> = DETECTION.iter().filter_map(|t| t.detect(root)).collect();
    if !found.is_empty() {
        return found;
    }
    DETECTION
        .iter()
        .filter_map(|t| t.detect_alone(root))
        .collect()
}

/// The names of the SDK's packages the packs of an ecosystem add, whose manager is `manager`
/// or none: what `uninit` takes out of a manifest that lists packages (ADR 0050).
fn sdk_names(ecosystem: &str, manager: Option<&str>) -> Vec<String> {
    crate::Packs::builtin_manifests()
        .unwrap_or_default()
        .into_iter()
        .filter(|m| m.ecosystem.as_str() == ecosystem)
        .flat_map(|m| m.instrumentation.into_iter())
        .flat_map(|i| i.dependencies.into_iter())
        .filter(|d| manager.is_none_or(|w| d.manager.as_deref().is_none_or(|m| m == w)))
        .map(|d| d.name)
        .collect()
}

/// Reads a file of the project.
fn read(root: &Path, path: &str) -> Option<String> {
    std::fs::read_to_string(root.join(path)).ok()
}

/// A package ecosystem of the inventory: its name in OSV's queries (`None` when OSV does not
/// know it), its type in a package URL, and how a name goes in one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Packages {
    pub ecosystem: Ecosystem,
    pub osv: Option<&'static str>,
    pub purl: &'static str,
    pub purl_name: PurlName,
}

/// How a package's name goes in its purl.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PurlName {
    /// As it is, without a namespace (`pkg:cargo/serde@1.0.0`).
    Plain,
    /// A path whose folders are the namespace (`pkg:golang/github.com/spf13/cobra@1.9.1`).
    Path,
    /// `group:artifact`, the group the namespace (`pkg:maven/io.opentelemetry/opentelemetry-api@1.67.0`).
    GroupArtifact,
}

impl Packages {
    const fn new(
        name: &'static str,
        rank: u16,
        osv: Option<&'static str>,
        purl: &'static str,
        purl_name: PurlName,
    ) -> Self {
        Self {
            ecosystem: Ecosystem::new(name, rank),
            osv,
            purl,
            purl_name,
        }
    }
}

/// Every package ecosystem, in the order that ranks the inventory.
const PACKAGES: [Packages; 11] = [
    Packages::new("cargo", 0, Some("crates.io"), "cargo", PurlName::Plain),
    Packages::new("npm", 1, Some("npm"), "npm", PurlName::Plain),
    Packages::new("pypi", 2, Some("PyPI"), "pypi", PurlName::Plain),
    Packages::new("go", 3, Some("Go"), "golang", PurlName::Path),
    Packages::new("maven", 4, Some("Maven"), "maven", PurlName::GroupArtifact),
    // C++ (ADR 0065, Q89): Conan's packages, vcpkg's ports, what CMake alone finds; OSV knows
    // neither vcpkg nor CMake, and the purl has no type of theirs.
    Packages::new("conan", 5, Some("ConanCenter"), "conan", PurlName::Plain),
    Packages::new("vcpkg", 6, None, "generic", PurlName::Plain),
    Packages::new("cmake", 7, None, "generic", PurlName::Plain),
    // `pkg:swift/github.com/apple/swift-log@1.6.0`, as Go's.
    Packages::new("swift", 8, Some("SwiftURL"), "swift", PurlName::Path),
    Packages::new("nuget", 9, Some("NuGet"), "nuget", PurlName::Plain),
    // `pkg:composer/monolog/monolog@3.9.0`: the vendor is the namespace.
    Packages::new(
        "composer",
        10,
        Some("Packagist"),
        "composer",
        PurlName::Path,
    ),
];

const PYPI: Ecosystem = PACKAGES[2].ecosystem;
const NPM: Ecosystem = PACKAGES[1].ecosystem;
const CARGO: Ecosystem = PACKAGES[0].ecosystem;
const GO: Ecosystem = PACKAGES[3].ecosystem;
const MAVEN: Ecosystem = PACKAGES[4].ecosystem;
const CONAN: Ecosystem = PACKAGES[5].ecosystem;
const VCPKG: Ecosystem = PACKAGES[6].ecosystem;
const CMAKE: Ecosystem = PACKAGES[7].ecosystem;
const SWIFT: Ecosystem = PACKAGES[8].ecosystem;
const NUGET: Ecosystem = PACKAGES[9].ecosystem;
const COMPOSER: Ecosystem = PACKAGES[10].ecosystem;

/// Every ecosystem of the packs, in the order the inventory reads their lockfiles.
static ALL: [&dyn Tooling; 10] = [
    &cargo::Cargo,
    &npm::Npm,
    &pypi::Pypi,
    &gomod::Gomod,
    &maven::Maven,
    &cmake::Cmake,
    &swiftpm::Swiftpm,
    &nuget::Nuget,
    &composer::Composer,
    &sbt::Sbt,
];

/// Every ecosystem of the packs.
pub fn all() -> &'static [&'static dyn Tooling] {
    &ALL
}

/// The ecosystem `pack.toml` names.
pub fn get(name: &str) -> Option<&'static dyn Tooling> {
    ALL.iter().copied().find(|tooling| tooling.name() == name)
}

/// A package ecosystem of the inventory, by its name.
pub fn packages(ecosystem: Ecosystem) -> Option<&'static Packages> {
    PACKAGES.iter().find(|p| p.ecosystem == ecosystem)
}

/// What the lockfiles of the repository say: its own packages and their dependencies.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lockfiles {
    /// The repository's packages: name, version, ecosystem.
    pub packages: Vec<(String, Option<String>, Ecosystem)>,
    pub dependencies: Vec<Dependency>,
    pub notes: Vec<String>,
}

impl Lockfiles {
    pub fn merge(&mut self, from: Lockfiles) {
        self.packages.extend(from.packages);
        self.dependencies.extend(from.dependencies);
        self.notes.extend(from.notes);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pack_ecosystem_has_its_tooling_and_the_ranks_follow_the_order() {
        for (rank, packages) in PACKAGES.iter().enumerate() {
            let next = PACKAGES.get(rank + 1);
            assert!(next.is_none_or(|n| packages.ecosystem < n.ecosystem));
        }
        for manifest in crate::Packs::builtin_manifests().expect("packs") {
            assert!(
                get(manifest.ecosystem.as_str()).is_some(),
                "{} has no tooling",
                manifest.ecosystem.as_str()
            );
        }
    }
}

/// The SDKs the tests of the edits give.
#[cfg(test)]
pub(crate) mod fixtures {
    use super::text::Sdk;

    pub(crate) fn sdk() -> Vec<Sdk<'static>> {
        static FEATURES: [String; 0] = [];
        vec![
            Sdk {
                name: "tracing",
                version: "0.1.44",
                features: &FEATURES,
                default_features: true,
                production: true,
            },
            Sdk {
                name: "opentelemetry",
                version: "0.33.1",
                features: &FEATURES,
                default_features: false,
                production: false,
            },
        ]
    }

    pub(crate) fn java_sdk() -> [Sdk<'static>; 2] {
        [
            Sdk {
                name: "io.opentelemetry:opentelemetry-api",
                version: "1.67.0",
                features: &[],
                default_features: true,
                production: true,
            },
            Sdk {
                name: "io.opentelemetry:opentelemetry-sdk",
                version: "1.67.0",
                features: &[],
                default_features: true,
                production: false,
            },
        ]
    }
}
