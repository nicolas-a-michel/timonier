//! Cargo: `cargo metadata`, else `Cargo.lock` (ADR 0055).

use std::fmt::Write;

use super::text::*;
use super::{Found, SdkPlace, WithSdk};
use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;
use timonier_core::inventory::{Dependency, Scope};

use super::CARGO;
use super::{Declared, Lockfiles, Packages, Run, Tooling, declare};

pub(crate) struct Cargo;

impl Tooling for Cargo {
    fn name(&self) -> &'static str {
        "cargo"
    }

    fn packages(&self) -> &'static [Packages] {
        &super::PACKAGES[0..1]
    }

    fn dependencies(&self, root: &Path, run: &Run<'_>) -> Lockfiles {
        let mut out = Lockfiles::default();
        if !root.join("Cargo.toml").is_file() {
            return out;
        }
        match run(
            root,
            "cargo",
            &["metadata", "--format-version", "1", "--offline"],
        ) {
            Some(stdout) => match serde_json::from_slice::<Value>(&stdout) {
                Ok(json) => out.merge(cargo_metadata(&json)),
                Err(error) => out
                    .notes
                    .push(format!("cargo metadata did not read: {error}")),
            },
            None => match std::fs::read_to_string(root.join("Cargo.lock")) {
                Ok(text) => {
                    out.notes.push(
                        "cargo metadata failed (no cargo, or a dependency not downloaded): \
                         versions from Cargo.lock, licences unknown"
                            .to_owned(),
                    );
                    out.merge(cargo_lock(&text, &cargo_members(root)));
                }
                Err(_) => out.notes.push(
                    "Cargo.toml without Cargo.lock: run `cargo generate-lockfile`".to_owned(),
                ),
            },
        }
        out
    }

    fn detect(&self, root: &Path) -> Option<Found> {
        self.runs(root).then_some(Found {
            language: "rust",
            manager: "cargo",
            test_script: None,
        })
    }

    fn runs(&self, root: &Path) -> bool {
        root.join("Cargo.toml").is_file()
    }

    fn sdk_place(&self, _root: &Path) -> SdkPlace {
        SdkPlace {
            manifest: "Cargo.toml".to_owned(),
            under: String::new(),
        }
    }

    fn with_sdk(
        &self,
        _manifest: &str,
        text: &str,
        sdk: &[Sdk<'_>],
        feature: Option<&str>,
    ) -> Result<WithSdk, String> {
        let Some(feature) = feature else {
            return Err("the pack names no feature".to_owned());
        };
        Ok(match cargo_with_sdk(text, feature, sdk)? {
            Some(text) => WithSdk::Text(text),
            None => WithSdk::Elsewhere(
                "a workspace without a root package; each crate that runs tests takes the SDK as \
                 the conventions say",
            ),
        })
    }

    fn without_sdk(
        &self,
        path: &str,
        text: &str,
        _original: &str,
    ) -> Option<Result<Option<String>, String>> {
        (path == "Cargo.toml").then(|| Ok(cargo_without_sdk(text)))
    }
}

/// `cargo metadata --format-version 1`: the workspace members, and every package of the
/// resolution with its licence.
pub(crate) fn cargo_metadata(json: &Value) -> Lockfiles {
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
            CARGO,
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
            ecosystem: CARGO,
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
pub(crate) fn cargo_lock(text: &str, declared: &Declared) -> Lockfiles {
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
            out.packages.push((name.to_owned(), version, CARGO));
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
            ecosystem: CARGO,
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

// What `timonier init` writes in the manifests, and `uninit` takes back (ADR 0050).

/// The markers of the two blocks of the OpenTelemetry SDK in a `Cargo.toml` (ADR 0050).
pub(crate) const FEATURE_BEGIN: &str = "# >>> timonier: feature >>>";
pub(crate) const FEATURE_END: &str = "# <<< timonier: feature <<<";
pub(crate) const DEPENDENCIES_BEGIN: &str = "# >>> timonier: dependencies >>>";
pub(crate) const DEPENDENCIES_END: &str = "# <<< timonier: dependencies <<<";

/// `Cargo.toml` with the SDK (ADR 0050): the packages the root package lacks as optional
/// dependencies, `[dependencies.<name>]` tables at the end, and `feature` turning them on, right
/// after `[features]` (or under a `[features]` of its own). `None` for a workspace without a
/// root package, whose crates take it as the conventions say; a feature of that name already
/// there is refused, with what to do.
pub(crate) fn cargo_with_sdk(
    text: &str,
    feature: &str,
    sdk: &[Sdk<'_>],
) -> Result<Option<String>, String> {
    let bare = cargo_without_sdk(text).unwrap_or_default();
    let manifest: toml::Table = bare
        .parse()
        .map_err(|e: toml::de::Error| format!("it does not read as TOML ({})", e.message()))?;
    if !manifest.contains_key("package") {
        return Ok(None);
    }
    let has = |table: &str, key: &str| {
        manifest
            .get(table)
            .and_then(toml::Value::as_table)
            .is_some_and(|t| t.contains_key(key))
    };
    if has("features", feature) {
        return Err(format!("a feature `{feature}` is already there"));
    }
    let added: Vec<&Sdk<'_>> = sdk
        .iter()
        .filter(|p| !has("dependencies", p.name))
        .collect();
    let mut dependencies = String::new();
    for package in &added {
        let _ = writeln!(dependencies, "[dependencies.{}]", package.name);
        let _ = writeln!(dependencies, "version = {}", quoted(package.version));
        dependencies.push_str("optional = true\n");
        if !package.default_features {
            dependencies.push_str("default-features = false\n");
        }
        if !package.features.is_empty() {
            let features: Vec<String> = package.features.iter().map(|f| quoted(f)).collect();
            let _ = writeln!(dependencies, "features = [{}]", features.join(", "));
        }
        dependencies.push('\n');
    }
    let turned_on: Vec<String> = added
        .iter()
        .map(|p| quoted(&format!("dep:{}", p.name)))
        .collect();
    let line = format!("{feature} = [{}]", turned_on.join(", "));
    let header = bare.lines().any(|l| l.trim() == "[features]");
    let mut out = if header {
        // Right after the developer's `[features]`.
        let mut out = String::new();
        let mut placed = false;
        for piece in bare.split_inclusive('\n') {
            out.push_str(piece);
            if !placed && piece.trim() == "[features]" {
                if !piece.ends_with('\n') {
                    out.push('\n');
                }
                let _ = write!(out, "{FEATURE_BEGIN}\n{line}\n{FEATURE_END}\n");
                placed = true;
            }
        }
        out
    } else {
        with_block(
            Some(&bare),
            FEATURE_BEGIN,
            FEATURE_END,
            &format!("[features]\n{line}"),
        )
    };
    if !added.is_empty() {
        out = with_block(
            Some(&out),
            DEPENDENCIES_BEGIN,
            DEPENDENCIES_END,
            &dependencies,
        );
    }
    out.parse::<toml::Table>()
        .map_err(|e| format!("the result would not read as TOML ({})", e.message()))?;
    Ok(Some(out))
}

/// `Cargo.toml` without the SDK's two blocks; `None` when nothing else is left.
pub(crate) fn cargo_without_sdk(text: &str) -> Option<String> {
    // The last added first: each block takes back the blank line it came with.
    let rest = without_block(text, DEPENDENCIES_BEGIN, DEPENDENCIES_END)?;
    without_block(&rest, FEATURE_BEGIN, FEATURE_END)
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
            [("app".to_owned(), Some("0.1.0".to_owned()), CARGO)]
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
    fn the_sdk_goes_into_cargo_toml_and_comes_out_as_it_was() {
        let plain = "[package]\nname = \"demo\"\n\n[dependencies]\ntracing = \"0.1\"\n";
        let with = cargo_with_sdk(plain, "otel", &crate::tooling::fixtures::sdk())
            .unwrap()
            .unwrap();
        let manifest: toml::Table = with.parse().unwrap();
        assert_eq!(
            manifest["features"]["otel"].as_array().unwrap(),
            &vec![toml::Value::from("dep:opentelemetry")],
            "the developer's tracing stays theirs: {with}"
        );
        let otel = &manifest["dependencies"]["opentelemetry"];
        assert_eq!(otel["optional"].as_bool(), Some(true));
        assert_eq!(otel["default-features"].as_bool(), Some(false));
        assert_eq!(manifest["dependencies"]["tracing"].as_str(), Some("0.1"));
        // Again: the same; then taken back, as it was.
        assert_eq!(
            cargo_with_sdk(&with, "otel", &crate::tooling::fixtures::sdk()).unwrap(),
            Some(with.clone())
        );
        assert_eq!(cargo_without_sdk(&with).as_deref(), Some(plain));

        // A `[features]` of the developer's: the feature goes right under it.
        let featured = "[package]\nname = \"demo\"\n\n[features]\nfast = []\n";
        let with = cargo_with_sdk(featured, "otel", &crate::tooling::fixtures::sdk())
            .unwrap()
            .unwrap();
        assert!(
            with.contains("[features]\n# >>> timonier: feature >>>\notel = ["),
            "{with}"
        );
        let manifest: toml::Table = with.parse().unwrap();
        assert!(manifest["features"].get("fast").is_some());
        assert_eq!(cargo_without_sdk(&with).as_deref(), Some(featured));
    }

    #[test]
    fn a_cargo_toml_the_sdk_cannot_go_into_says_why() {
        let workspace = "[workspace]\nmembers = [\"a\"]\n";
        assert_eq!(
            cargo_with_sdk(workspace, "otel", &crate::tooling::fixtures::sdk()),
            Ok(None)
        );
        let taken = "[package]\nname = \"d\"\n[features]\notel = []\n";
        assert!(
            cargo_with_sdk(taken, "otel", &crate::tooling::fixtures::sdk())
                .unwrap_err()
                .contains("already")
        );
        assert!(
            cargo_with_sdk("[package", "otel", &crate::tooling::fixtures::sdk())
                .unwrap_err()
                .contains("TOML")
        );
    }
}
