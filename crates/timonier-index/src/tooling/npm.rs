//! npm: `package-lock.json` (ADR 0055).

use super::ordered::Json;
use super::text::*;
use super::{Found, SdkPlace, WithSdk};
use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;
use timonier_core::inventory::{Dependency, Scope};

use super::NPM;
use super::{Declared, Lockfiles, Packages, Run, Tooling, declare};

pub(crate) struct Npm;

impl Tooling for Npm {
    fn name(&self) -> &'static str {
        "npm"
    }

    fn packages(&self) -> &'static [Packages] {
        &super::PACKAGES[1..2]
    }

    fn dependencies(&self, root: &Path, _run: &Run<'_>) -> Lockfiles {
        let mut out = Lockfiles::default();
        if !root.join("package.json").is_file() {
            return out;
        }
        match std::fs::read_to_string(root.join("package-lock.json")) {
            Ok(text) => match serde_json::from_str::<Value>(&text) {
                Ok(json) => out.merge(npm_lock(&json)),
                Err(error) => out
                    .notes
                    .push(format!("package-lock.json did not read: {error}")),
            },
            Err(_) => out
                .notes
                .push("package.json without package-lock.json: run `npm install`".to_owned()),
        }
        out
    }

    fn ci_steps(&self, root: &Path) -> Option<&'static str> {
        root.join("package-lock.json").is_file().then_some(
            "      - uses: actions/setup-node@v7
        with:
          node-version: 24
      - run: npm ci
",
        )
    }

    /// A `package.json`: TypeScript with a `tsconfig` or a `typescript` dependency, else
    /// JavaScript; the manager by its lockfile; its `test` script.
    fn detect(&self, root: &Path) -> Option<Found> {
        let manifest = std::fs::read_to_string(root.join("package.json")).ok()?;
        let json = serde_json::from_str::<Value>(&manifest).ok();
        let typescript = ["tsconfig.json", "tsconfig.base.json"]
            .iter()
            .any(|f| root.join(f).is_file())
            || json.as_ref().is_some_and(|m| {
                ["dependencies", "devDependencies"]
                    .iter()
                    .any(|d| m[d].get("typescript").is_some())
            });
        let manager = [
            ("pnpm-lock.yaml", "pnpm"),
            ("yarn.lock", "yarn"),
            ("bun.lock", "bun"),
            ("bun.lockb", "bun"),
        ]
        .into_iter()
        .find(|(lock, _)| root.join(lock).is_file())
        .map_or("npm", |(_, manager)| manager);
        Some(Found {
            language: if typescript {
                "typescript"
            } else {
                "javascript"
            },
            manager,
            test_script: json.and_then(|m| m["scripts"]["test"].as_str().map(str::to_owned)),
        })
    }

    fn runs(&self, root: &Path) -> bool {
        root.join("package.json").is_file()
    }

    fn sdk_place(&self, _root: &Path) -> SdkPlace {
        SdkPlace {
            manifest: "package.json".to_owned(),
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
        npm_with_sdk(text, sdk).map(WithSdk::Text)
    }

    /// The SDK's packages that were not there before `init` (ADR 0050).
    fn without_sdk(
        &self,
        path: &str,
        text: &str,
        original: &str,
    ) -> Option<Result<Option<String>, String>> {
        (path == "package.json").then(|| {
            let names = super::sdk_names(self.name(), None);
            let names: Vec<&str> = names.iter().map(String::as_str).collect();
            npm_without_sdk(text, original, &names)
        })
    }
}

/// `package-lock.json`, versions 2 and 3: the workspaces (keys outside `node_modules/`) with
/// what they declare, and each installed package with its version and licence.
pub(crate) fn npm_lock(json: &Value) -> Lockfiles {
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
            NPM,
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
            ecosystem: NPM,
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

// What `timonier init` writes in the manifests, and `uninit` takes back (ADR 0050).

/// The names under `dependencies` and `devDependencies` of a `package.json`.
pub(crate) fn npm_dependencies(manifest: &Json) -> Vec<String> {
    ["dependencies", "devDependencies"]
        .iter()
        .filter_map(|section| match manifest.get(section) {
            Some(Json::Object(entries)) => Some(entries.iter().map(|(name, _)| name.clone())),
            _ => None,
        })
        .flatten()
        .collect()
}

/// `package.json` with the SDK's packages it lacks as development dependencies (ADR 0050).
pub(crate) fn npm_with_sdk(text: &str, sdk: &[Sdk<'_>]) -> Result<String, String> {
    let manifest = object(Some(text))?;
    let present = npm_dependencies(&manifest);
    let mut out = text.to_owned();
    for package in sdk.iter().filter(|p| !present.iter().any(|n| n == p.name)) {
        out = json_with_entry(
            Some(&out),
            if package.production {
                "dependencies"
            } else {
                "devDependencies"
            },
            package.name,
            Json::String(package.version.to_owned()),
        )?;
    }
    Ok(out)
}

/// `package.json` without the SDK's packages that `original`, the file before `init`, did not
/// have.
pub(crate) fn npm_without_sdk(
    text: &str,
    original: &str,
    names: &[&str],
) -> Result<Option<String>, String> {
    let before = npm_dependencies(&object(Some(original))?);
    let mut out = Some(text.to_owned());
    for name in names.iter().filter(|n| !before.iter().any(|b| b == *n)) {
        for section in ["devDependencies", "dependencies"] {
            let Some(current) = &out else { break };
            out = json_without_entry(current, section, name)?;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn the_sdk_goes_into_package_json_and_only_it_comes_out() {
        let original =
            "{\n  \"name\": \"demo\",\n  \"devDependencies\": {\n    \"tracing\": \"1\"\n  }\n}\n";
        let with = npm_with_sdk(original, &crate::tooling::fixtures::sdk()).unwrap();
        let value: serde_json::Value = serde_json::from_str(&with).unwrap();
        assert_eq!(value["devDependencies"]["opentelemetry"], "0.33.1");
        assert_eq!(
            value["dependencies"],
            serde_json::Value::Null,
            "tracing was there already"
        );
        assert_eq!(
            value["devDependencies"]["tracing"], "1",
            "the developer's stays"
        );
        let back = npm_without_sdk(&with, original, &["tracing", "opentelemetry"])
            .unwrap()
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&back).unwrap();
        assert_eq!(
            value,
            serde_json::from_str::<serde_json::Value>(original).unwrap()
        );
        // Q78: the one the shipped code imports goes into `dependencies`, and comes out of it.
        let bare = "{\n  \"name\": \"demo\"\n}\n";
        let with = npm_with_sdk(bare, &crate::tooling::fixtures::sdk()).unwrap();
        let value: serde_json::Value = serde_json::from_str(&with).unwrap();
        assert_eq!(value["dependencies"]["tracing"], "0.1.44");
        assert_eq!(value["devDependencies"]["opentelemetry"], "0.33.1");
        let back = npm_without_sdk(&with, bare, &["tracing", "opentelemetry"])
            .unwrap()
            .unwrap();
        assert_eq!(back, bare);
    }
}
