//! Swift packages: `Package.resolved` (ADR 0067).

use super::text::*;
use super::{Found, SdkPlace, WithSdk};
use std::path::Path;

use serde_json::Value;
use timonier_core::inventory::{Dependency, Scope};

use super::SWIFT;
use super::{Lockfiles, Packages, Run, Tooling};

pub(crate) struct Swiftpm;

impl Tooling for Swiftpm {
    fn name(&self) -> &'static str {
        "swiftpm"
    }

    fn packages(&self) -> &'static [Packages] {
        &super::PACKAGES[8..9]
    }

    fn dependencies(&self, root: &Path, _run: &Run<'_>) -> Lockfiles {
        swift(root)
    }

    /// A package at the root (ADR 0067).
    fn detect(&self, root: &Path) -> Option<Found> {
        self.runs(root).then_some(Found {
            language: "swift",
            manager: "swiftpm",
            test_script: None,
        })
    }

    fn runs(&self, root: &Path) -> bool {
        root.join("Package.swift").is_file()
    }

    fn sdk_place(&self, _root: &Path) -> SdkPlace {
        SdkPlace {
            manifest: "Package.swift".to_owned(),
            under: String::new(),
        }
    }

    fn with_sdk(
        &self,
        _manifest: &str,
        text: &str,
        sdk: &[Sdk<'_>],
        _feature: Option<&str>,
    ) -> Result<WithSdk, String> {
        swift_package_with_sdk(text, sdk).map(WithSdk::Text)
    }

    fn without_sdk(
        &self,
        path: &str,
        text: &str,
        _original: &str,
    ) -> Option<Result<Option<String>, String>> {
        (path == "Package.swift").then(|| Ok(Some(swift_package_without_sdk(text))))
    }
}

/// Swift (ADR 0067): the packages of `Package.swift`, with the versions `Package.resolved` pins,
/// and the ones they pull in; no licences.
pub(crate) fn swift(root: &Path) -> Lockfiles {
    let mut out = Lockfiles::default();
    let Ok(manifest) = std::fs::read_to_string(root.join("Package.swift")) else {
        return out;
    };
    let name = manifest
        .find("name:")
        .and_then(|at| manifest[at..].split('"').nth(1))
        .unwrap_or_default()
        .to_owned();
    out.packages.push((name.clone(), None, SWIFT));
    let declared: Vec<(String, Option<String>)> = crate::ecosystem::swift_packages(&manifest);
    let location = |url: &str| {
        url.trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches(".git")
            .trim_end_matches('/')
            .to_owned()
    };
    let resolved = std::fs::read_to_string(root.join("Package.resolved"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok());
    let pins: Vec<Value> = resolved
        .as_ref()
        .and_then(|r| {
            r["pins"]
                .as_array()
                .or_else(|| r["object"]["pins"].as_array())
                .cloned()
        })
        .unwrap_or_default();
    if pins.is_empty() {
        out.notes.push(
            "Package.swift without Package.resolved: the declared packages, versions unknown              (run `swift package resolve`)"
                .to_owned(),
        );
        for (package, _) in &declared {
            out.dependencies.push(Dependency {
                ecosystem: SWIFT,
                name: package.clone(),
                version: None,
                license: None,
                direct: true,
                scope: Scope::Normal,
                used_by: vec![name.clone()],
                function: None,
            });
        }
        return out;
    }
    for pin in pins {
        let url = pin["location"]
            .as_str()
            .or_else(|| pin["repositoryURL"].as_str());
        let Some(url) = url else { continue };
        let package = location(url);
        let direct = declared
            .iter()
            .any(|(d, _)| d.eq_ignore_ascii_case(&package));
        out.dependencies.push(Dependency {
            ecosystem: SWIFT,
            name: package,
            version: pin["state"]["version"].as_str().map(str::to_owned),
            license: None,
            direct,
            scope: Scope::Normal,
            used_by: if direct {
                vec![name.clone()]
            } else {
                Vec::new()
            },
            function: None,
        });
    }
    out.notes
        .push("Package.resolved records no licences: Swift licences unknown".to_owned());
    out
}

// What `timonier init` writes in the manifests, and `uninit` takes back (ADR 0050).

/// What marks the lines `init` adds to `Package.swift` (ADR 0067).
pub(crate) const SWIFT_MARK: &str = "// timonier";

/// `Package.swift` with the SDK (ADR 0067): each package it does not depend on yet, at the top
/// of the package's `dependencies` (an array of their own before `targets` when it has none),
/// and a test target `TimonierTests` that takes their products (`features`), at the top of
/// `targets`. Every line it adds ends with `// timonier`. `Err` when `targets` is not an array
/// opened on its own line.
pub(crate) fn swift_package_with_sdk(text: &str, sdk: &[Sdk<'_>]) -> Result<String, String> {
    let bare = swift_package_without_sdk(text);
    let mut lines: Vec<String> = bare.lines().map(str::to_owned).collect();
    let indent_of = |line: &str| line[..line.len() - line.trim_start().len()].to_owned();
    // The arguments of `Package(`: the indentation of its `name:`.
    let name = lines
        .iter()
        .position(|l| l.trim_start().starts_with("name:"))
        .ok_or("it has no `name:` line")?;
    let outer = indent_of(&lines[name]);
    let unit = if outer.is_empty() {
        "    ".to_owned()
    } else {
        outer.clone()
    };
    let inner = format!("{outer}{unit}");
    let at = |lines: &[String], key: &str| {
        lines
            .iter()
            .position(|l| indent_of(l) == outer && l.trim_start().starts_with(&format!("{key}:")))
    };
    let identity = |url: &str| {
        url.trim_end_matches(".git")
            .rsplit('/')
            .next()
            .unwrap_or(url)
            .to_owned()
    };
    let missing: Vec<&Sdk<'_>> = sdk
        .iter()
        .filter(|p| !bare.contains(p.name.trim_end_matches(".git")))
        .collect();
    let packages: Vec<String> = missing
        .iter()
        .map(|p| {
            format!(
                "{inner}.package(url: \"{}\", from: \"{}\"),  {SWIFT_MARK}",
                p.name, p.version
            )
        })
        .collect();
    if !packages.is_empty() {
        match at(&lines, "dependencies") {
            Some(i) if lines[i].trim_end().ends_with('[') => {
                lines.splice(i + 1..i + 1, packages);
            }
            Some(_) => {
                return Err("its dependencies are not an array opened on its own line".to_owned());
            }
            None => {
                let targets = at(&lines, "targets").ok_or("it has no `targets:` line")?;
                let mut block = vec![format!("{outer}dependencies: [  {SWIFT_MARK}")];
                block.extend(packages);
                block.push(format!("{outer}],  {SWIFT_MARK}"));
                lines.splice(targets..targets, block);
            }
        }
    }
    if !bare.contains("\"TimonierTests\"") {
        let targets = at(&lines, "targets")
            .filter(|&i| lines[i].trim_end().ends_with('['))
            .ok_or("its targets are not an array opened on its own line")?;
        let products: Vec<String> = sdk
            .iter()
            .flat_map(|p| {
                p.features.iter().map(move |product| {
                    format!(
                        ".product(name: \"{product}\", package: \"{}\")",
                        identity(p.name)
                    )
                })
            })
            .collect();
        lines.insert(
            targets + 1,
            format!(
                "{inner}.testTarget(name: \"TimonierTests\", dependencies: [{}]),  {SWIFT_MARK}",
                products.join(", ")
            ),
        );
    }
    let mut out = lines.join("\n");
    if bare.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

/// `Package.swift` without the lines [`swift_package_with_sdk`] added.
pub(crate) fn swift_package_without_sdk(text: &str) -> String {
    let mut out: String = text
        .lines()
        .filter(|l| !l.trim_end().ends_with(SWIFT_MARK))
        .collect::<Vec<_>>()
        .join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-128: `Package.swift` takes the OpenTelemetry packages and the test target, and gives
    /// them back; without a `dependencies` array, one of its own; twice changes nothing.
    #[test]
    fn the_swift_sdk_goes_in_and_out_of_package_swift() {
        let core = ["OpenTelemetryApi".to_owned(), "OpenTelemetrySdk".to_owned()];
        let sdk = [Sdk {
            name: "https://github.com/open-telemetry/opentelemetry-swift-core.git",
            version: "2.6.0",
            features: &core,
            default_features: true,
            production: false,
        }];
        let with_dependencies = "// swift-tools-version: 6.0\nimport PackageDescription\n\nlet package = Package(\n    name: \"Shop\",\n    dependencies: [\n        .package(url: \"https://github.com/apple/swift-log.git\", from: \"1.6.0\"),\n    ],\n    targets: [\n        .target(name: \"Shop\"),\n    ]\n)\n";
        let without = "// swift-tools-version: 6.0\nimport PackageDescription\n\nlet package = Package(\n    name: \"Shop\",\n    targets: [\n        .target(name: \"Shop\"),\n    ]\n)\n";
        for original in [with_dependencies, without] {
            let with = swift_package_with_sdk(original, &sdk).unwrap();
            assert!(
                with.contains("        .package(url: \"https://github.com/open-telemetry/opentelemetry-swift-core.git\", from: \"2.6.0\"),  // timonier\n"),
                "{with}"
            );
            assert!(
                with.contains("        .testTarget(name: \"TimonierTests\", dependencies: [.product(name: \"OpenTelemetryApi\", package: \"opentelemetry-swift-core\"), .product(name: \"OpenTelemetrySdk\", package: \"opentelemetry-swift-core\")]),  // timonier\n"),
                "{with}"
            );
            assert_eq!(swift_package_with_sdk(&with, &sdk).unwrap(), with, "twice");
            assert_eq!(swift_package_without_sdk(&with), original, "{with}");
        }
        assert!(
            swift_package_with_sdk(without, &sdk)
                .unwrap()
                .contains("    dependencies: [  // timonier\n")
        );
    }
}
