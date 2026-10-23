//! Composer: `composer.lock` (ADR 0070).

use super::ordered::Json;
use super::text::*;
use super::{Found, SdkPlace, WithSdk};
use std::path::Path;

use serde_json::Value;
use timonier_core::inventory::{Dependency, Scope};

use super::COMPOSER;
use super::{Lockfiles, Packages, Run, Tooling};

pub(crate) struct Composer;

impl Tooling for Composer {
    fn name(&self) -> &'static str {
        "composer"
    }

    fn packages(&self) -> &'static [Packages] {
        &super::PACKAGES[10..11]
    }

    fn dependencies(&self, root: &Path, _run: &Run<'_>) -> Lockfiles {
        composer(root)
    }

    /// A Composer project at the root (ADR 0070).
    fn detect(&self, root: &Path) -> Option<Found> {
        self.runs(root).then_some(Found {
            language: "php",
            manager: "composer",
            test_script: None,
        })
    }

    fn runs(&self, root: &Path) -> bool {
        root.join("composer.json").is_file()
    }

    fn sdk_place(&self, _root: &Path) -> SdkPlace {
        SdkPlace {
            manifest: "composer.json".to_owned(),
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
        composer_with_sdk(text, sdk).map(WithSdk::Text)
    }

    /// The SDK's packages that were not there before `init` (ADR 0070).
    fn without_sdk(
        &self,
        path: &str,
        text: &str,
        original: &str,
    ) -> Option<Result<Option<String>, String>> {
        (path == "composer.json").then(|| {
            let names = super::sdk_names(self.name(), None);
            let names: Vec<&str> = names.iter().map(String::as_str).collect();
            composer_without_sdk(text, original, &names)
        })
    }
}

/// PHP (ADR 0070): the packages `composer.json` declares, and every package of
/// `composer.lock` with its version and its licences (Composer records them); `require-dev`
/// and `packages-dev` are for the tests.
pub(crate) fn composer(root: &Path) -> Lockfiles {
    let mut out = Lockfiles::default();
    let Some(manifest) = std::fs::read_to_string(root.join("composer.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
    else {
        return out;
    };
    let name = manifest["name"].as_str().unwrap_or("project").to_owned();
    out.packages.push((
        name.clone(),
        manifest["version"].as_str().map(str::to_owned),
        COMPOSER,
    ));
    let declared = |section: &str, package: &str| manifest[section].get(package).is_some();
    let lock = std::fs::read_to_string(root.join("composer.lock"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok());
    let Some(lock) = lock else {
        out.notes.push(
            "composer.json without composer.lock: the declared packages, versions unknown \
             (run `composer update`)"
                .to_owned(),
        );
        for (section, scope) in [("require", Scope::Normal), ("require-dev", Scope::Dev)] {
            for package in manifest[section]
                .as_object()
                .into_iter()
                .flat_map(|m| m.keys())
            {
                if package.contains('/') {
                    out.dependencies.push(Dependency {
                        ecosystem: COMPOSER,
                        name: package.clone(),
                        version: None,
                        license: None,
                        direct: true,
                        scope,
                        used_by: vec![name.clone()],
                        function: None,
                    });
                }
            }
        }
        return out;
    };
    for (section, scope) in [("packages", Scope::Normal), ("packages-dev", Scope::Dev)] {
        for package in lock[section].as_array().into_iter().flatten() {
            let Some(id) = package["name"].as_str() else {
                continue;
            };
            let direct = declared("require", id) || declared("require-dev", id);
            let license = package["license"].as_array().map(|l| {
                l.iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(" OR ")
            });
            out.dependencies.push(Dependency {
                ecosystem: COMPOSER,
                name: id.to_owned(),
                version: package["version"]
                    .as_str()
                    .map(|v| v.trim_start_matches('v').to_owned()),
                license: license.filter(|l| !l.is_empty()),
                direct,
                scope,
                used_by: if direct {
                    vec![name.clone()]
                } else {
                    Vec::new()
                },
                function: None,
            });
        }
    }
    out
}

// What `timonier init` writes in the manifests, and `uninit` takes back (ADR 0050).

/// `composer.json` with the SDK (ADR 0070): the production packages in `require`, the others in
/// `require-dev`, each with its constraint; a package declared already is left.
pub(crate) fn composer_with_sdk(text: &str, sdk: &[Sdk<'_>]) -> Result<String, String> {
    let manifest = object(Some(text))?;
    let declared = |name: &str| {
        ["require", "require-dev"]
            .iter()
            .any(|s| manifest.get(s).and_then(|m| m.get(name)).is_some())
    };
    let mut out = text.to_owned();
    for package in sdk.iter().filter(|p| !declared(p.name)) {
        out = json_with_entry(
            Some(&out),
            if package.production {
                "require"
            } else {
                "require-dev"
            },
            package.name,
            Json::String(package.version.to_owned()),
        )?;
    }
    Ok(out)
}

/// `composer.json` without the SDK's packages that `original`, the file before `init`, did
/// not have.
pub(crate) fn composer_without_sdk(
    text: &str,
    original: &str,
    names: &[&str],
) -> Result<Option<String>, String> {
    let before = object(Some(original))?;
    let had = |name: &str| {
        ["require", "require-dev"]
            .iter()
            .any(|s| before.get(s).and_then(|m| m.get(name)).is_some())
    };
    let mut out = Some(text.to_owned());
    for name in names.iter().filter(|n| !had(n)) {
        for section in ["require-dev", "require"] {
            let Some(current) = &out else { break };
            out = json_without_entry(current, section, name)?;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// T-131: `composer.json` takes the API in `require`, the SDK in `require-dev`, and gives
    /// them back as the file was.
    #[test]
    fn the_php_sdk_goes_in_and_out_of_composer_json() {
        let sdk = [
            Sdk {
                name: "open-telemetry/api",
                version: "^1.10",
                features: &[],
                default_features: true,
                production: true,
            },
            Sdk {
                name: "open-telemetry/sdk",
                version: "^1.15",
                features: &[],
                default_features: true,
                production: false,
            },
        ];
        let original =
            "{\n  \"name\": \"shop/shop\",\n  \"require\": {\n    \"php\": \">=8.4\"\n  }\n}\n";
        let with = composer_with_sdk(original, &sdk).unwrap();
        let parsed: Value = serde_json::from_str(&with).unwrap();
        assert_eq!(parsed["require"]["open-telemetry/api"], "^1.10");
        assert_eq!(parsed["require-dev"]["open-telemetry/sdk"], "^1.15");
        assert_eq!(composer_with_sdk(&with, &sdk).unwrap(), with, "twice");
        let back = composer_without_sdk(
            &with,
            original,
            &["open-telemetry/api", "open-telemetry/sdk"],
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&back).unwrap(),
            serde_json::from_str::<Value>(original).unwrap()
        );
    }
}
