//! .NET: the projects and `packages.lock.json` (ADR 0069).

use super::maven::*;
use super::text::*;
use super::{Found, SdkPlace, WithSdk};
use std::collections::BTreeMap;
use std::path::Path;

use timonier_core::inventory::{Dependency, Scope};

use super::NUGET;
use super::{Lockfiles, Packages, Run, Tooling};
use crate::ecosystem::xml_text;

pub(crate) struct Nuget;

impl Tooling for Nuget {
    fn name(&self) -> &'static str {
        "nuget"
    }

    fn packages(&self) -> &'static [Packages] {
        &super::PACKAGES[9..10]
    }

    fn dependencies(&self, root: &Path, _run: &Run<'_>) -> Lockfiles {
        dotnet(root)
    }

    /// A solution or a project at the root or one folder below (ADR 0069).
    fn detect(&self, root: &Path) -> Option<Found> {
        self.runs(root).then_some(Found {
            language: "csharp",
            manager: "dotnet",
            test_script: None,
        })
    }

    fn runs(&self, root: &Path) -> bool {
        dotnet_projects(root)
    }

    /// The test project takes the SDK, and the files go beside it (ADR 0069).
    fn sdk_place(&self, root: &Path) -> SdkPlace {
        match dotnet_test_project(root) {
            Some(project) => SdkPlace {
                under: project
                    .rsplit_once('/')
                    .map_or(String::new(), |(dir, _)| format!("{dir}/")),
                manifest: project,
            },
            None => SdkPlace {
                manifest: "Tests.csproj".to_owned(),
                under: String::new(),
            },
        }
    }

    fn with_sdk(
        &self,
        _manifest: &str,
        text: &str,
        sdk: &[Sdk<'_>],
        _feature: Option<&str>,
    ) -> Result<WithSdk, String> {
        csproj_with_sdk(text, sdk).map(WithSdk::Text)
    }

    fn without_manifest(&self) -> Option<&'static str> {
        Some(
            "no test project (one that references Microsoft.NET.Test.Sdk); the project that runs \
             the tests takes the SDK and the files as the conventions say",
        )
    }

    fn without_sdk(
        &self,
        path: &str,
        text: &str,
        _original: &str,
    ) -> Option<Result<Option<String>, String>> {
        path.ends_with(".csproj")
            .then(|| Ok(Some(pom_without_sdk(text))))
    }
}

/// A .NET solution (`.sln`, `.slnx`) or project (`.csproj`) at the root, or a project one
/// folder below (`src/Shop/Shop.csproj` is two: also looked for).
fn dotnet_projects(root: &Path) -> bool {
    let has = |dir: &Path, extensions: &[&str]| {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .any(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                extensions.iter().any(|x| name.ends_with(x))
            })
    };
    if has(root, &[".sln", ".slnx", ".csproj"]) {
        return true;
    }
    let below = |dir: &Path| -> Vec<std::path::PathBuf> {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
            .map(|e| e.path())
            .collect()
    };
    below(root)
        .iter()
        .flat_map(|d| std::iter::once(d.clone()).chain(below(d)))
        .any(|d| has(&d, &[".csproj"]))
}

/// The first .NET test project, the one that references Microsoft.NET.Test.Sdk, at the root and
/// three folders below: its path from the root.
fn dotnet_test_project(root: &Path) -> Option<String> {
    let mut found = Vec::new();
    let mut pending = vec![(root.to_path_buf(), 0)];
    while let Some((dir, depth)) = pending.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                if depth < 3 && !name.starts_with('.') && !matches!(name.as_str(), "bin" | "obj") {
                    pending.push((path, depth + 1));
                }
            } else if name.ends_with(".csproj")
                && std::fs::read_to_string(&path)
                    .is_ok_and(|t| t.contains("Microsoft.NET.Test.Sdk"))
                && let Ok(relative) = path.strip_prefix(root)
            {
                found.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    found.sort();
    found.into_iter().next()
}

/// .NET (ADR 0069): the `<PackageReference>`s of the `.csproj` files at the root and three
/// folders below, with their versions (`Version`, or central `Directory.Packages.props`); a
/// test project's are for the tests. No licences.
pub(crate) fn dotnet(root: &Path) -> Lockfiles {
    let mut out = Lockfiles::default();
    let mut projects = Vec::new();
    let mut pending = vec![(root.to_path_buf(), 0)];
    while let Some((dir, depth)) = pending.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                if depth < 3
                    && !name.starts_with('.')
                    && !matches!(name.as_str(), "bin" | "obj" | "node_modules")
                {
                    pending.push((path, depth + 1));
                }
            } else if name.ends_with(".csproj") {
                projects.push(path);
            }
        }
    }
    if projects.is_empty() {
        return out;
    }
    projects.sort();
    let central: BTreeMap<String, String> =
        std::fs::read_to_string(root.join("Directory.Packages.props"))
            .ok()
            .and_then(|text| {
                let document =
                    roxmltree::Document::parse(text.trim_start_matches('\u{feff}')).ok()?;
                Some(
                    document
                        .descendants()
                        .filter(|n| n.has_tag_name("PackageVersion"))
                        .filter_map(|n| {
                            Some((
                                n.attribute("Include")?.to_owned(),
                                n.attribute("Version")?.to_owned(),
                            ))
                        })
                        .collect(),
                )
            })
            .unwrap_or_default();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for project in projects {
        let Ok(text) = std::fs::read_to_string(&project) else {
            continue;
        };
        let Ok(document) = roxmltree::Document::parse(text.trim_start_matches('\u{feff}')) else {
            out.notes
                .push(format!("{} did not read", project.display()));
            continue;
        };
        let name = project
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        out.packages.push((name.clone(), None, NUGET));
        let references: Vec<(String, Option<String>)> = document
            .descendants()
            .filter(|n| n.has_tag_name("PackageReference"))
            .filter_map(|n| {
                let package = n.attribute("Include")?.to_owned();
                let version = n
                    .attribute("Version")
                    .map(str::to_owned)
                    .or_else(|| xml_text(n, "Version"))
                    .or_else(|| central.get(&package).cloned());
                Some((package, version))
            })
            .collect();
        let test = references
            .iter()
            .any(|(p, _)| p == crate::ecosystem::DOTNET_TEST_SDK);
        let scope = if test { Scope::Dev } else { Scope::Normal };
        for (package, version) in references {
            if let Some(&i) = seen.get(&package) {
                let known: &mut Dependency = &mut out.dependencies[i];
                known.used_by.push(name.clone());
                known.scope = known.scope.min(scope);
                continue;
            }
            seen.insert(package.clone(), out.dependencies.len());
            out.dependencies.push(Dependency {
                ecosystem: NUGET,
                name: package,
                version,
                license: None,
                direct: true,
                scope,
                used_by: vec![name.clone()],
                function: None,
            });
        }
    }
    out.notes
        .push(".csproj records no licences: .NET licences unknown".to_owned());
    out
}

// What `timonier init` writes in the manifests, and `uninit` takes back (ADR 0050).

/// A `.csproj` with the SDK (ADR 0069): an `<ItemGroup>` of its own between markers before
/// `</Project>`, with the packages the project does not reference yet. Taken back by
/// [`pom_without_sdk`], whose markers it shares.
pub(crate) fn csproj_with_sdk(text: &str, sdk: &[Sdk<'_>]) -> Result<String, String> {
    let bare = pom_without_sdk(text);
    let missing: Vec<&Sdk<'_>> = sdk
        .iter()
        .filter(|p| !bare.contains(&format!("Include=\"{}\"", p.name)))
        .collect();
    if missing.is_empty() {
        return Ok(bare);
    }
    let mut lines: Vec<String> = bare.lines().map(str::to_owned).collect();
    let at = lines
        .iter()
        .rposition(|l| l.trim() == "</Project>")
        .ok_or("its </Project> is not on a line of its own")?;
    let mut block = vec![format!("  {XML_BEGIN}"), "  <ItemGroup>".to_owned()];
    for p in missing {
        block.push(format!(
            "    <PackageReference Include=\"{}\" Version=\"{}\" />",
            p.name, p.version
        ));
    }
    block.push("  </ItemGroup>".to_owned());
    block.push(format!("  {XML_END}"));
    lines.splice(at..at, block);
    let mut out = lines.join("\n");
    if bare.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-130: a test project takes the SDK and the JUnit logger in an `<ItemGroup>` of its own,
    /// and gives them back; a package referenced already is left.
    #[test]
    fn the_csharp_sdk_goes_in_and_out_of_a_csproj() {
        let sdk = [
            Sdk {
                name: "OpenTelemetry",
                version: "1.20.1",
                features: &[],
                default_features: true,
                production: false,
            },
            Sdk {
                name: "JunitXml.TestLogger",
                version: "8.0.0",
                features: &[],
                default_features: true,
                production: false,
            },
        ];
        let project = "<Project Sdk=\"Microsoft.NET.Sdk\">\n  <ItemGroup>\n    <PackageReference Include=\"xunit\" Version=\"2.9.3\" />\n    <PackageReference Include=\"JunitXml.TestLogger\" Version=\"7.0.0\" />\n  </ItemGroup>\n</Project>\n";
        let with = csproj_with_sdk(project, &sdk).unwrap();
        assert!(with.contains("    <PackageReference Include=\"OpenTelemetry\" Version=\"1.20.1\" />\n  </ItemGroup>\n  <!-- <<< timonier <<< -->\n</Project>\n"), "{with}");
        assert_eq!(with.matches("JunitXml.TestLogger").count(), 1, "{with}");
        assert_eq!(csproj_with_sdk(&with, &sdk).unwrap(), with, "twice");
        assert_eq!(pom_without_sdk(&with), project);
    }
}
