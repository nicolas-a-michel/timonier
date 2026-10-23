//! Python: `pyproject.toml` and `uv.lock` (ADR 0062).

use super::text::*;
use super::{Found, SdkPlace, WithSdk};
use std::collections::BTreeSet;
use std::path::Path;

use timonier_core::inventory::{Dependency, Scope};

use super::PYPI;
use super::{Declared, Lockfiles, Packages, Run, Tooling, declare};
use crate::ecosystem::{normalise_python, requirement_name};

pub(crate) struct Pypi;

impl Tooling for Pypi {
    fn name(&self) -> &'static str {
        "pypi"
    }

    fn packages(&self) -> &'static [Packages] {
        &super::PACKAGES[2..3]
    }

    fn dependencies(&self, root: &Path, _run: &Run<'_>) -> Lockfiles {
        let mut out = Lockfiles::default();
        let Ok(text) = std::fs::read_to_string(root.join("pyproject.toml")) else {
            return out;
        };
        let declared = python_declared(&text);
        match std::fs::read_to_string(root.join("uv.lock")) {
            Ok(lock) => out.merge(uv_lock(&lock, &declared)),
            Err(_) => {
                out.notes.push(
                    "pyproject.toml without uv.lock: the declared dependencies, versions \
                     unknown (run `uv lock`)"
                        .to_owned(),
                );
                for (name, (scope, _)) in &declared {
                    out.dependencies.push(Dependency {
                        ecosystem: PYPI,
                        name: name.clone(),
                        version: None,
                        license: None,
                        direct: true,
                        scope: *scope,
                        used_by: Vec::new(),
                        function: None,
                    });
                }
            }
        }
        out
    }

    /// A `pyproject.toml`, or a project without one but its requirements (ADR 0062).
    fn detect(&self, root: &Path) -> Option<Found> {
        ["pyproject.toml", "requirements.txt", "setup.py"]
            .iter()
            .any(|f| root.join(f).is_file())
            .then(|| {
                let manager = [
                    ("uv.lock", "uv"),
                    ("poetry.lock", "poetry"),
                    ("pdm.lock", "pdm"),
                ]
                .into_iter()
                .find(|(lock, _)| root.join(lock).is_file())
                .map_or("pip", |(_, manager)| manager);
                Found {
                    language: "python",
                    manager,
                    test_script: None,
                }
            })
    }

    /// A folder of scripts without a manifest: its runner is pytest, as for a project with only
    /// its requirements (no SDK without a `pyproject.toml`).
    fn detect_alone(&self, root: &Path) -> Option<Found> {
        python_files(root).then_some(Found {
            language: "python",
            manager: "pip",
            test_script: None,
        })
    }

    /// Python was found by its manifest or, without one, by its files.
    fn runs(&self, _root: &Path) -> bool {
        true
    }

    fn sdk_place(&self, _root: &Path) -> SdkPlace {
        SdkPlace {
            manifest: "pyproject.toml".to_owned(),
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
        pyproject_with_sdk(text, sdk).map(WithSdk::Text)
    }

    fn without_sdk(
        &self,
        path: &str,
        text: &str,
        _original: &str,
    ) -> Option<Result<Option<String>, String>> {
        (path == "pyproject.toml").then(|| Ok(Some(pyproject_without_sdk(text))))
    }
}

/// A Python file at the root.
fn python_files(root: &Path) -> bool {
    std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .any(|entry| {
            entry.path().extension().is_some_and(|e| e == "py")
                && entry.file_type().is_ok_and(|t| t.is_file())
        })
}

/// What a `pyproject.toml` declares (ADR 0062): `[project]`'s dependencies as normal, its
/// optional ones and PEP 735 groups as dev, names normalised (PEP 503).
pub(crate) fn python_declared(text: &str) -> Declared {
    let mut declared = Declared::new();
    let Ok(manifest) = text.parse::<toml::Table>() else {
        return declared;
    };
    let project = manifest.get("project").and_then(toml::Value::as_table);
    let by = project
        .and_then(|p| p.get("name"))
        .and_then(toml::Value::as_str)
        .map(normalise_python)
        .unwrap_or_default();
    let requirements = |value: Option<&toml::Value>| -> Vec<String> {
        value
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(toml::Value::as_str)
            .filter_map(requirement_name)
            .collect()
    };
    for name in requirements(project.and_then(|p| p.get("dependencies"))) {
        declare(&mut declared, &name, Scope::Normal, &by);
    }
    let groups = project
        .and_then(|p| p.get("optional-dependencies"))
        .and_then(toml::Value::as_table)
        .into_iter()
        .chain(
            manifest
                .get("dependency-groups")
                .and_then(toml::Value::as_table),
        )
        .flat_map(|groups| groups.values());
    for group in groups {
        for name in requirements(Some(group)) {
            declare(&mut declared, &name, Scope::Dev, &by);
        }
    }
    declared
}

/// `uv.lock`: each package of the resolution with its version; the repository's own (editable
/// or virtual sources) apart. uv does not record licences.
pub(crate) fn uv_lock(text: &str, declared: &Declared) -> Lockfiles {
    let mut out = Lockfiles::default();
    let Ok(lock) = text.parse::<toml::Table>() else {
        out.notes.push("uv.lock did not read".to_owned());
        return out;
    };
    for package in lock
        .get("package")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(name) = package.get("name").and_then(toml::Value::as_str) else {
            continue;
        };
        let name = normalise_python(name);
        let version = package
            .get("version")
            .and_then(toml::Value::as_str)
            .map(str::to_owned);
        let source = package.get("source").and_then(toml::Value::as_table);
        let own = source.is_some_and(|s| s.contains_key("editable") || s.contains_key("virtual"));
        if own {
            out.packages.push((name, version, PYPI));
            continue;
        }
        let (scope, used_by) = declared
            .get(&name)
            .cloned()
            .unwrap_or((Scope::Normal, BTreeSet::new()));
        out.dependencies.push(Dependency {
            ecosystem: PYPI,
            name,
            version,
            license: None,
            direct: !used_by.is_empty(),
            scope,
            used_by: used_by.into_iter().collect(),
            function: None,
        });
    }
    out.notes
        .push("uv.lock records no licences: Python licences unknown".to_owned());
    out
}

// What `timonier init` writes in the manifests, and `uninit` takes back (ADR 0050).

/// What marks the lines `init` adds inside `pyproject.toml`'s own tables (ADR 0062).
pub(crate) const PYPROJECT_MARK: &str = "# timonier";

/// `pyproject.toml` with the SDK (ADR 0062): the production packages in `[project]`'s
/// `dependencies` (the code that ships imports them, Q78), the others in a PEP 735 group
/// `timonier`. Each line it adds ends with `# timonier`; `[dependency-groups]`, when it adds the
/// table, is a block between markers. Packages already declared are left. `Err` without a
/// `[project]` table (Poetry's alone): the API goes in by hand.
pub(crate) fn pyproject_with_sdk(text: &str, sdk: &[Sdk<'_>]) -> Result<String, String> {
    let bare = pyproject_without_sdk(text);
    let manifest: toml::Table = bare
        .parse()
        .map_err(|e: toml::de::Error| format!("it does not read as TOML ({})", e.message()))?;
    let project = manifest
        .get("project")
        .and_then(toml::Value::as_table)
        .ok_or("it has no [project] table: add opentelemetry-api to the dependencies by hand")?;
    let declared: Vec<String> = project
        .get("dependencies")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .chain(
            manifest
                .get("dependency-groups")
                .and_then(toml::Value::as_table)
                .into_iter()
                .flat_map(|groups| groups.values())
                .filter_map(toml::Value::as_array)
                .flatten(),
        )
        .filter_map(toml::Value::as_str)
        .map(|r| {
            r.chars()
                .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
                .collect::<String>()
                .to_lowercase()
                .replace('_', "-")
        })
        .collect();
    let requirement = |p: &Sdk<'_>| quoted(&format!("{}=={}", p.name, p.version));
    let missing = |p: &&Sdk<'_>| !declared.contains(&p.name.to_lowercase());
    let production: Vec<String> = sdk
        .iter()
        .filter(|p| p.production)
        .filter(missing)
        .map(requirement)
        .collect();
    let group: Vec<String> = sdk
        .iter()
        .filter(|p| !p.production)
        .filter(missing)
        .map(requirement)
        .collect();
    let mut lines: Vec<String> = bare.lines().map(str::to_owned).collect();
    let header = |lines: &[String], name: &str| lines.iter().position(|l| l.trim() == name);
    if !production.is_empty() {
        let at =
            header(&lines, "[project]").ok_or("its [project] table is not on a line of its own")?;
        let end = lines[at + 1..]
            .iter()
            .position(|l| l.trim_start().starts_with('['))
            .map_or(lines.len(), |i| at + 1 + i);
        let key = (at + 1..end).find(|&i| {
            let line = lines[i].trim_start();
            line.starts_with("dependencies")
                && line["dependencies".len()..].trim_start().starts_with('=')
        });
        match key {
            Some(i) => {
                let Some(open) = lines[i].find('[') else {
                    return Err("its dependencies are not written as an array".to_owned());
                };
                let (head, tail) = lines[i].split_at(open + 1);
                let added: Vec<String> = production
                    .iter()
                    .map(|r| format!("    {r},  {PYPROJECT_MARK}"))
                    .collect();
                let rest = tail.trim_start();
                let mut replacement = vec![head.to_owned()];
                replacement.extend(added);
                if !rest.is_empty() {
                    replacement.push(format!("    {rest}"));
                }
                lines.splice(i..=i, replacement);
            }
            None => lines.insert(
                at + 1,
                format!(
                    "dependencies = [{}]  {PYPROJECT_MARK}",
                    production.join(", ")
                ),
            ),
        }
    }
    if !group.is_empty() {
        let entry = format!("timonier = [{}]  {PYPROJECT_MARK}", group.join(", "));
        match header(&lines, "[dependency-groups]") {
            Some(at) => lines.insert(at + 1, entry),
            None => {
                if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                    lines.push(String::new());
                }
                lines.extend([
                    HASH_BEGIN.to_owned(),
                    "[dependency-groups]".to_owned(),
                    entry,
                    HASH_END.to_owned(),
                ]);
            }
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    Ok(out)
}

/// `pyproject.toml` without what [`pyproject_with_sdk`] added: its marked lines, its block, and
/// the dependencies array it opened, put back on one line.
pub(crate) fn pyproject_without_sdk(text: &str) -> String {
    let rest = without_block(text, HASH_BEGIN, HASH_END).unwrap_or_default();
    let mut lines: Vec<&str> = Vec::new();
    let mut opened_by_us = false;
    for line in rest.lines() {
        if line.trim_end().ends_with(PYPROJECT_MARK) {
            // An element `init` put in an array the developer wrote: that array was split.
            opened_by_us |= line.trim_start().starts_with('"');
            continue;
        }
        lines.push(line);
    }
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        // `dependencies = [` followed by one moved-down line, as `init` split it: one line again.
        if opened_by_us
            && lines[i].trim_end().ends_with('[')
            && lines[i].trim_start().starts_with("dependencies")
            && i + 1 < lines.len()
            && lines[i + 1].starts_with("    ")
            && lines[i + 1].trim_end().ends_with(']')
        {
            out.push(format!("{}{}", lines[i], lines[i + 1].trim_start()));
            i += 2;
            continue;
        }
        out.push(lines[i].to_owned());
        i += 1;
    }
    let mut text = out.join("\n");
    if rest.ends_with('\n') {
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-121: the SDK goes into `pyproject.toml` and comes out, the file as it was; a second
    /// `init` changes nothing; a package declared already is left.
    #[test]
    fn the_python_sdk_goes_in_and_out_of_pyproject() {
        let sdk = [
            Sdk {
                name: "opentelemetry-api",
                version: "1.45.1",
                features: &[],
                default_features: true,
                production: true,
            },
            Sdk {
                name: "opentelemetry-sdk",
                version: "1.45.1",
                features: &[],
                default_features: true,
                production: false,
            },
        ];
        let one_line = "[project]\nname = \"shop\"\ndependencies = [\"fastapi>=0.115\"]\n";
        let many = "[project]\nname = \"shop\"\ndependencies = [\n    \"fastapi>=0.115\",\n]\n\n[dependency-groups]\ndev = [\"pytest\"]\n";
        let none = "[project]\nname = \"shop\"\nversion = \"1.0\"\n";
        for original in [one_line, many, none] {
            let with = pyproject_with_sdk(original, &sdk).unwrap();
            let parsed: toml::Table = with.parse().unwrap_or_else(|e| panic!("{e}\n{with}"));
            let dependencies = parsed["project"]["dependencies"].as_array().unwrap();
            assert!(
                dependencies
                    .iter()
                    .any(|d| d.as_str() == Some("opentelemetry-api==1.45.1")),
                "{with}"
            );
            assert_eq!(
                parsed["dependency-groups"]["timonier"].as_array().unwrap()[0].as_str(),
                Some("opentelemetry-sdk==1.45.1"),
                "{with}"
            );
            assert_eq!(pyproject_with_sdk(&with, &sdk).unwrap(), with, "twice");
            assert_eq!(pyproject_without_sdk(&with), original, "{with}");
        }
        let declared = "[project]\nname = \"x\"\ndependencies = [\"opentelemetry-api>=1\"]\n";
        let with = pyproject_with_sdk(declared, &sdk).unwrap();
        assert!(!with.contains("opentelemetry-api==1.45.1"), "{with}");
        assert!(
            pyproject_with_sdk("[tool.poetry]\nname = \"x\"\n", &sdk)
                .unwrap_err()
                .contains("[project]")
        );
    }
}
