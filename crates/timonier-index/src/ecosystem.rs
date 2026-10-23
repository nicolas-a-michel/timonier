//! Package ecosystems (ADR 0012, 0013): packages found in manifests, and the module path of a
//! file. Cargo, npm and Python (`pyproject.toml`) are compiled in; a pack names its ecosystem.

use std::collections::BTreeMap;
use std::path::Path;

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use timonier_core::NodeId;

use crate::pack::Ecosystem;
use crate::{Error, Result};

/// The manifest file names of the ecosystems.
pub(crate) const MANIFESTS: [(&str, Ecosystem); 15] = [
    ("Cargo.toml", Ecosystem::Cargo),
    ("package.json", Ecosystem::Npm),
    ("pyproject.toml", Ecosystem::Pypi),
    ("go.mod", Ecosystem::Gomod),
    // A Go workspace: no package, its `use` directories are members (ADR 0063).
    ("go.work", Ecosystem::Gomod),
    // Maven coordinates (ADR 0064): Maven's `pom.xml`, Gradle's builds, and Gradle's settings,
    // which make no package but name the members.
    ("pom.xml", Ecosystem::Maven),
    ("build.gradle.kts", Ecosystem::Maven),
    ("build.gradle", Ecosystem::Maven),
    ("settings.gradle.kts", Ecosystem::Maven),
    ("settings.gradle", Ecosystem::Maven),
    // CMake (ADR 0065): `vcpkg.json` and `conanfile.*` beside it are read with it.
    ("CMakeLists.txt", Ecosystem::Cmake),
    ("Package.swift", Ecosystem::Swiftpm),
    // .NET (ADR 0069): a project file is named after its project.
    ("*.csproj", Ecosystem::Nuget),
    ("composer.json", Ecosystem::Composer),
    ("build.sbt", Ecosystem::Sbt),
];

/// Whether a file name is a manifest's: `Cargo.toml` as it is, `*.csproj` by its end.
pub(crate) fn manifest_named(manifest: &str, name: &str) -> bool {
    match manifest.strip_prefix('*') {
        Some(suffix) => name.ends_with(suffix) && name.len() > suffix.len(),
        None => manifest == name,
    }
}

impl PackageInfo {
    /// Its manifest, relative to the repository root.
    pub fn manifest(&self) -> String {
        let file = MANIFESTS
            .iter()
            .find(|(_, ecosystem)| *ecosystem == self.ecosystem)
            .map_or("", |(file, _)| file);
        if self.dir.is_empty() {
            file.to_owned()
        } else {
            format!("{}/{file}", self.dir)
        }
    }
}

/// A package of the repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageInfo {
    pub id: NodeId,
    pub name: String,
    pub ecosystem: Ecosystem,
    /// Its directory relative to the repository root, `""` for the root.
    pub dir: String,
    /// The name the code uses for it: the library crate for Cargo, the package name for npm.
    pub code_name: String,
    /// Cargo: `[lib] path`, relative to the package directory, when it is declared. Python: the
    /// source root (`src`) when the package keeps its modules there (ADR 0062).
    pub lib_path: Option<String>,
    /// Cargo: the package has a library target (its file exists). It decides how `src/main.rs`
    /// is named (ADR 0013).
    pub has_lib: bool,
    /// Cargo: `[[bin]]` targets with an explicit path, as (name, path).
    pub bins: Vec<(String, String)>,
    pub dependencies: Vec<Dependency>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dependency {
    /// The registry name (`serde_json`, `@xyflow/react`).
    pub name: String,
    /// The name the code uses (`serde_json`, the crate ident; the npm name).
    pub code_name: String,
    /// A normal dependency, not a dev, test or build one. Only these are on the map (ADR 0013).
    pub runtime: bool,
}

/// A parsed manifest, before the workspace rules decide whether it is a package.
#[derive(Debug)]
struct Candidate {
    info: Option<PackageInfo>,
    /// Workspace members and exclusions, as globs relative to the repository root.
    members: Vec<String>,
    exclude: Vec<String>,
}

/// Finds the packages among the manifests (paths relative to `root`), with the rules of
/// ADR 0013: workspace members, workspace roots that are packages, and manifests outside
/// any other package of the same ecosystem.
///
/// A manifest that cannot be read or parsed is left out and reported, with the others kept: a
/// broken fixture, or a manifest being edited, must not stop the indexing.
pub(crate) fn discover(root: &Path, manifests: &[String]) -> (Vec<PackageInfo>, Vec<Error>) {
    discover_with(root, manifests, &BTreeMap::new())
}

/// [`discover`], with the files of `overlay` in place of the disk's (`None`: absent): a
/// commit's manifests and Cargo libraries, for its snapshot (Q23).
pub(crate) fn discover_with(
    root: &Path,
    manifests: &[String],
    overlay: &BTreeMap<String, Option<Vec<u8>>>,
) -> (Vec<PackageInfo>, Vec<Error>) {
    let mut problems = Vec::new();
    let mut candidates = Vec::new();
    for manifest in manifests {
        let name = manifest.rsplit('/').next().unwrap_or(manifest);
        let Some((_, ecosystem)) = MANIFESTS
            .iter()
            .find(|(file, _)| manifest_named(file, name))
        else {
            continue;
        };
        let file = &name;
        let dir = manifest[..manifest.len() - file.len()]
            .trim_end_matches('/')
            .to_owned();
        let path = root.join(manifest);
        let read = match overlay.get(manifest) {
            Some(Some(bytes)) => String::from_utf8(bytes.clone())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
            // Absent from the overlay's tree.
            Some(None) => continue,
            None => std::fs::read_to_string(&path),
        };
        let text = match read {
            Ok(text) => text,
            Err(source) => {
                problems.push(Error::Io { path, source });
                continue;
            }
        };
        let candidate = match ecosystem {
            Ecosystem::Cargo => cargo(&text, &dir),
            Ecosystem::Npm => npm(&text, &dir),
            Ecosystem::Pypi => pypi(&text, &dir),
            Ecosystem::Gomod if file == &"go.work" => go_work(&text, &dir),
            Ecosystem::Gomod => go_mod(&text, &dir),
            Ecosystem::Maven if file.starts_with("settings.") => Ok(gradle_settings(&text, &dir)),
            Ecosystem::Maven if file.starts_with("build.") => {
                // The version catalog `gradle init` writes, at the build's root.
                let catalog = std::fs::read_to_string(root.join("gradle/libs.versions.toml"))
                    .unwrap_or_default();
                let fallback = if dir.is_empty() {
                    root.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                } else {
                    dir.rsplit('/').next().unwrap_or(&dir).to_owned()
                };
                gradle_build(&text, &dir, &fallback, &catalog)
            }
            Ecosystem::Maven => maven_pom(&text, &dir),
            Ecosystem::Swiftpm => swift_package(&text, &dir),
            Ecosystem::Nuget => csharp_project(&text, &dir, name.trim_end_matches(".csproj")),
            Ecosystem::Sbt => sbt_build(&text, &dir),
            Ecosystem::Composer => composer(
                &text,
                &dir,
                &std::fs::read_to_string(root.join(&dir).join("composer.lock")).unwrap_or_default(),
            ),
            Ecosystem::Cmake => {
                let beside = |name: &str| {
                    std::fs::read_to_string(root.join(&dir).join(name)).unwrap_or_default()
                };
                Ok(cmake_lists(
                    &text,
                    &dir,
                    &beside("vcpkg.json"),
                    &(beside("conanfile.txt") + &beside("conanfile.py")),
                ))
            }
        };
        match candidate {
            Ok(candidate) => candidates.push(candidate),
            Err(reason) => problems.push(Error::PackageManifest { path, reason }),
        }
    }
    let mut members = GlobSetBuilder::new();
    let mut excluded = GlobSetBuilder::new();
    for candidate in &candidates {
        for glob in &candidate.members {
            match glob_of(glob) {
                Ok(glob) => {
                    members.add(glob);
                }
                Err(problem) => problems.push(problem),
            }
        }
        for glob in &candidate.exclude {
            match glob_of(glob) {
                Ok(glob) => {
                    excluded.add(glob);
                }
                Err(problem) => problems.push(problem),
            }
        }
    }
    let (members, excluded) = match (build(members), build(excluded)) {
        (Ok(members), Ok(excluded)) => (members, excluded),
        (Err(problem), _) | (_, Err(problem)) => {
            problems.push(problem);
            (GlobSet::empty(), GlobSet::empty())
        }
    };
    let mut infos: Vec<(bool, PackageInfo)> = candidates
        .into_iter()
        .filter_map(|c| c.info)
        .map(|info| {
            let member = members.is_match(&info.dir) && !excluded.is_match(&info.dir);
            (member, info)
        })
        .collect();
    // Shallow first, so that a nested manifest sees the packages around it.
    infos.sort_by(|(_, a), (_, b)| depth(&a.dir).cmp(&depth(&b.dir)).then(a.dir.cmp(&b.dir)));
    let mut accepted: Vec<PackageInfo> = Vec::new();
    for (member, info) in infos {
        let nested = accepted
            .iter()
            .any(|p| p.ecosystem == info.ecosystem && contains(&p.dir, &info.dir));
        let duplicate = accepted.iter().any(|p| p.id == info.id);
        if (member || !nested) && !duplicate {
            accepted.push(info);
        }
    }
    for package in &mut accepted {
        // A Python package keeps its modules under `src/` when it has one (the « src layout »).
        if package.ecosystem == Ecosystem::Pypi {
            let src = if package.dir.is_empty() {
                "src".to_owned()
            } else {
                format!("{}/src", package.dir)
            };
            let has_src = overlay
                .keys()
                .any(|path| path.starts_with(&format!("{src}/")))
                || root.join(&src).is_dir();
            package.lib_path = has_src.then(|| "src".to_owned());
        }
        if package.ecosystem == Ecosystem::Cargo {
            let lib = package.lib_path.as_deref().unwrap_or("src/lib.rs");
            let relative = if package.dir.is_empty() {
                lib.to_owned()
            } else {
                format!("{}/{lib}", package.dir)
            };
            package.has_lib = match overlay.get(&relative) {
                Some(content) => content.is_some(),
                None => root.join(&package.dir).join(lib).is_file(),
            };
        }
    }
    accepted.sort_by(|a, b| a.id.cmp(&b.id));
    (accepted, problems)
}

fn glob_of(glob: &str) -> Result<globset::Glob> {
    GlobBuilder::new(glob)
        .literal_separator(true)
        .build()
        .map_err(|e| Error::PackageManifest {
            path: glob.into(),
            reason: format!("invalid workspace glob: {e}"),
        })
}

fn build(builder: GlobSetBuilder) -> Result<GlobSet> {
    builder.build().map_err(|e| Error::PackageManifest {
        path: "workspace".into(),
        reason: e.to_string(),
    })
}

fn depth(dir: &str) -> usize {
    if dir.is_empty() {
        0
    } else {
        dir.split('/').count()
    }
}

/// Whether directory `inner` is `outer` or inside it.
pub(crate) fn contains(outer: &str, inner: &str) -> bool {
    outer.is_empty() || inner == outer || inner.starts_with(&format!("{outer}/"))
}

/// `dir/glob`, normalised (`./crates/*` → `crates/*`).
fn under(dir: &str, glob: &str) -> String {
    let glob = glob.trim_start_matches("./").trim_end_matches('/');
    if dir.is_empty() {
        glob.to_owned()
    } else {
        format!("{dir}/{glob}")
    }
}

fn strings(value: Option<&toml::Value>) -> Vec<String> {
    value
        .and_then(toml::Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn cargo(text: &str, dir: &str) -> Result<Candidate, String> {
    let table: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
    let workspace = table.get("workspace").and_then(toml::Value::as_table);
    let members = strings(workspace.and_then(|w| w.get("members")))
        .iter()
        .map(|g| under(dir, g))
        .collect();
    let exclude = strings(workspace.and_then(|w| w.get("exclude")))
        .iter()
        .map(|g| under(dir, g))
        .collect();
    let info = match table.get("package").and_then(toml::Value::as_table) {
        None => None,
        Some(package) => {
            let name = package
                .get("name")
                .and_then(toml::Value::as_str)
                .ok_or("[package] has no name")?
                .to_owned();
            let lib = table.get("lib").and_then(toml::Value::as_table);
            let code_name = lib
                .and_then(|l| l.get("name"))
                .and_then(toml::Value::as_str)
                .map_or_else(|| name.replace('-', "_"), str::to_owned);
            let lib_path = lib
                .and_then(|l| l.get("path"))
                .and_then(toml::Value::as_str)
                .map(|p| p.trim_start_matches("./").to_owned());
            let bins = table
                .get("bin")
                .and_then(toml::Value::as_array)
                .map(|bins| {
                    bins.iter()
                        .filter_map(|b| {
                            let b = b.as_table()?;
                            let name = b.get("name")?.as_str()?;
                            let path = b.get("path")?.as_str()?;
                            Some((name.to_owned(), path.trim_start_matches("./").to_owned()))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let mut dependencies = Vec::new();
            let mut add = |deps: Option<&toml::Value>, runtime: bool| {
                for (key, value) in deps.and_then(toml::Value::as_table).into_iter().flatten() {
                    let name = value
                        .get("package")
                        .and_then(toml::Value::as_str)
                        .unwrap_or(key)
                        .to_owned();
                    dependencies.push(Dependency {
                        name,
                        code_name: key.replace('-', "_"),
                        runtime,
                    });
                }
            };
            add(table.get("dependencies"), true);
            add(table.get("dev-dependencies"), false);
            add(table.get("build-dependencies"), false);
            for target in table
                .get("target")
                .and_then(toml::Value::as_table)
                .into_iter()
                .flatten()
            {
                add(target.1.get("dependencies"), true);
                add(target.1.get("dev-dependencies"), false);
                add(target.1.get("build-dependencies"), false);
            }
            dedup(&mut dependencies);
            Some(PackageInfo {
                id: NodeId::package("cargo", &name).map_err(|e| e.to_string())?,
                name,
                ecosystem: Ecosystem::Cargo,
                dir: dir.to_owned(),
                code_name,
                lib_path,
                has_lib: false,
                bins,
                dependencies,
            })
        }
    };
    Ok(Candidate {
        info,
        members,
        exclude,
    })
}

fn npm(text: &str, dir: &str) -> Result<Candidate, String> {
    let json: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let globs = |v: Option<&serde_json::Value>| -> Vec<String> {
        v.and_then(serde_json::Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|g| g.as_str())
                    .map(|g| under(dir, g))
                    .collect()
            })
            .unwrap_or_default()
    };
    let workspaces = json.get("workspaces");
    let members = match workspaces {
        Some(serde_json::Value::Array(_)) => globs(workspaces),
        Some(object) => globs(object.get("packages")),
        None => Vec::new(),
    };
    let info = match json.get("name").and_then(serde_json::Value::as_str) {
        None => None,
        Some(name) => {
            let mut dependencies = Vec::new();
            for (field, runtime) in [
                ("dependencies", true),
                ("optionalDependencies", true),
                ("peerDependencies", false),
                ("devDependencies", false),
            ] {
                for key in json
                    .get(field)
                    .and_then(serde_json::Value::as_object)
                    .into_iter()
                    .flat_map(|o| o.keys())
                {
                    dependencies.push(Dependency {
                        name: key.clone(),
                        code_name: key.clone(),
                        runtime,
                    });
                }
            }
            dedup(&mut dependencies);
            Some(PackageInfo {
                id: NodeId::package("npm", name).map_err(|e| e.to_string())?,
                name: name.to_owned(),
                ecosystem: Ecosystem::Npm,
                dir: dir.to_owned(),
                code_name: name.to_owned(),
                lib_path: None,
                has_lib: false,
                bins: Vec::new(),
                dependencies,
            })
        }
    };
    Ok(Candidate {
        info,
        members,
        exclude: Vec::new(),
    })
}

/// `pyproject.toml` (ADR 0062): PEP 621's `[project]`, else Poetry's `[tool.poetry]`; uv's
/// workspace members; PEP 508 requirements, their names normalised as PEP 503 says.
fn pypi(text: &str, dir: &str) -> Result<Candidate, String> {
    let manifest: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
    let table = |value: Option<&toml::Value>| value.and_then(toml::Value::as_table).cloned();
    let project = table(manifest.get("project"));
    let tool = table(manifest.get("tool")).unwrap_or_default();
    let poetry = table(tool.get("poetry"));
    let workspace =
        table(table(tool.get("uv")).unwrap_or_default().get("workspace")).unwrap_or_default();
    let members: Vec<String> = strings(workspace.get("members"))
        .iter()
        .map(|g| under(dir, g))
        .collect();
    let exclude: Vec<String> = strings(workspace.get("exclude"))
        .iter()
        .map(|g| under(dir, g))
        .collect();
    let name = project
        .as_ref()
        .and_then(|p| p.get("name"))
        .or_else(|| poetry.as_ref().and_then(|p| p.get("name")))
        .and_then(toml::Value::as_str)
        .map(normalise_python);
    let info = match name {
        None => None,
        Some(name) => {
            let mut dependencies = Vec::new();
            let mut add = |requirement: &str, runtime: bool| {
                if let Some(name) = requirement_name(requirement) {
                    dependencies.push(Dependency {
                        code_name: name.replace('-', "_"),
                        name,
                        runtime,
                    });
                }
            };
            if let Some(project) = &project {
                for requirement in strings(project.get("dependencies")) {
                    add(&requirement, true);
                }
                for (_, group) in table(project.get("optional-dependencies")).unwrap_or_default() {
                    for requirement in strings(Some(&group)) {
                        add(&requirement, false);
                    }
                }
            }
            for (_, group) in table(manifest.get("dependency-groups")).unwrap_or_default() {
                for requirement in strings(Some(&group)) {
                    add(&requirement, false);
                }
            }
            if let Some(poetry) = &poetry {
                for key in table(poetry.get("dependencies")).unwrap_or_default().keys() {
                    if key != "python" {
                        add(key, true);
                    }
                }
            }
            dedup(&mut dependencies);
            Some(PackageInfo {
                id: NodeId::package("pypi", &name).map_err(|e| e.to_string())?,
                code_name: name.replace('-', "_"),
                name,
                ecosystem: Ecosystem::Pypi,
                dir: dir.to_owned(),
                lib_path: None,
                has_lib: false,
                bins: Vec::new(),
                dependencies,
            })
        }
    };
    Ok(Candidate {
        info,
        members,
        exclude,
    })
}

/// `go.mod` (ADR 0063): the module path, its `require`s, a `// indirect` one not on the map.
fn go_mod(text: &str, dir: &str) -> Result<Candidate, String> {
    let mut module = None;
    let mut dependencies = Vec::new();
    let mut in_require = false;
    for line in text.lines() {
        let (code, comment) = line.split_once("//").unwrap_or((line, ""));
        let code = code.trim();
        if in_require {
            if code == ")" {
                in_require = false;
            } else if let Some(path) = code.split_whitespace().next() {
                dependencies.push(go_dependency(path, comment));
            }
            continue;
        }
        if let Some(rest) = code.strip_prefix("module ") {
            module = Some(rest.trim().trim_matches('"').to_owned());
        } else if code == "require (" || code == "require(" {
            in_require = true;
        } else if let Some(rest) = code.strip_prefix("require ")
            && let Some(path) = rest.split_whitespace().next()
        {
            dependencies.push(go_dependency(path, comment));
        }
    }
    let module = module.ok_or("go.mod has no module line")?;
    dedup(&mut dependencies);
    Ok(Candidate {
        info: Some(PackageInfo {
            id: NodeId::package("gomod", &module).map_err(|e| e.to_string())?,
            name: module.clone(),
            ecosystem: Ecosystem::Gomod,
            dir: dir.to_owned(),
            code_name: module,
            lib_path: None,
            has_lib: false,
            bins: Vec::new(),
            dependencies,
        }),
        members: Vec::new(),
        exclude: Vec::new(),
    })
}

fn go_dependency(path: &str, comment: &str) -> Dependency {
    let path = path.trim_matches('"').to_owned();
    Dependency {
        code_name: path.clone(),
        name: path,
        runtime: comment.trim() != "indirect",
    }
}

/// `go.work`: the modules it `use`s are members, each a package of its own.
fn go_work(text: &str, dir: &str) -> Result<Candidate, String> {
    let mut members = Vec::new();
    let mut in_use = false;
    for line in text.lines() {
        let code = line.split_once("//").map_or(line, |(c, _)| c).trim();
        if in_use {
            if code == ")" {
                in_use = false;
            } else if !code.is_empty() {
                members.push(under(dir, code.trim_matches('"')));
            }
        } else if code == "use (" || code == "use(" {
            in_use = true;
        } else if let Some(rest) = code.strip_prefix("use ") {
            members.push(under(dir, rest.trim().trim_matches('"')));
        }
    }
    Ok(Candidate {
        info: None,
        members,
        exclude: Vec::new(),
    })
}

/// `pom.xml` (ADR 0064): `groupId:artifactId`, the group inherited from `<parent>`; its
/// `<modules>` are members; its dependencies, test ones not at run time. A dependency's code
/// name is its group, which prefixes the packages it brings.
fn maven_pom(text: &str, dir: &str) -> Result<Candidate, String> {
    let document = roxmltree::Document::parse(text).map_err(|e| e.to_string())?;
    let project = document.root_element();
    let child = xml_child;
    let text_of = xml_text;
    let artifact = text_of(project, "artifactId").ok_or("pom.xml has no artifactId")?;
    let group = text_of(project, "groupId")
        .or_else(|| child(project, "parent").and_then(|p| text_of(p, "groupId")))
        .unwrap_or_default();
    let members = child(project, "modules")
        .into_iter()
        .flat_map(|m| m.children().filter(|c| c.tag_name().name() == "module"))
        .filter_map(|m| m.text())
        .map(|m| under(dir, m.trim()))
        .collect();
    let mut dependencies: Vec<Dependency> = child(project, "dependencies")
        .into_iter()
        .flat_map(|d| d.children().filter(|c| c.tag_name().name() == "dependency"))
        .filter_map(|d| {
            let group = text_of(d, "groupId")?;
            let artifact = text_of(d, "artifactId")?;
            let scope = text_of(d, "scope").unwrap_or_default();
            Some(Dependency {
                name: format!("{group}:{artifact}"),
                code_name: group,
                runtime: scope != "test",
            })
        })
        .collect();
    dedup(&mut dependencies);
    Ok(Candidate {
        info: Some(maven_package(&group, &artifact, dir, dependencies)?),
        members,
        exclude: Vec::new(),
    })
}

/// The first child element of `node` named `name`.
pub fn xml_child<'a, 'i>(
    node: roxmltree::Node<'a, 'i>,
    name: &str,
) -> Option<roxmltree::Node<'a, 'i>> {
    node.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

/// The trimmed text of the child element `name`, when it has some.
pub fn xml_text(node: roxmltree::Node<'_, '_>, name: &str) -> Option<String> {
    xml_child(node, name)
        .and_then(|c| c.text())
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
}

fn maven_package(
    group: &str,
    artifact: &str,
    dir: &str,
    dependencies: Vec<Dependency>,
) -> Result<PackageInfo, String> {
    let name = if group.is_empty() {
        artifact.to_owned()
    } else {
        format!("{group}:{artifact}")
    };
    Ok(PackageInfo {
        id: NodeId::package("maven", &name).map_err(|e| e.to_string())?,
        name,
        ecosystem: Ecosystem::Maven,
        dir: dir.to_owned(),
        code_name: group.to_owned(),
        lib_path: None,
        has_lib: false,
        bins: Vec::new(),
        dependencies,
    })
}

/// The configurations of a Gradle build whose dependencies are not at run time.
const GRADLE_DEV: [&str; 6] = [
    "testImplementation",
    "testRuntimeOnly",
    "testCompileOnly",
    "testFixturesImplementation",
    "androidTestImplementation",
    "kapt",
];

/// The configurations a Gradle build declares its dependencies in.
const GRADLE_CONFIGURATIONS: [&str; 13] = [
    "implementation",
    "api",
    "compileOnly",
    "runtimeOnly",
    "annotationProcessor",
    "ksp",
    "kapt",
    "testImplementation",
    "testRuntimeOnly",
    "testCompileOnly",
    "testFixturesImplementation",
    "androidTestImplementation",
    "developmentOnly",
];

/// `build.gradle(.kts)` (ADR 0064), read line by line: its `group`, its name (its directory's),
/// and the coordinates its configurations declare, written out (`"g:a:v"`) or through the version
/// catalog (`libs.spring.boot.web`).
fn gradle_build(text: &str, dir: &str, name: &str, catalog: &str) -> Result<Candidate, String> {
    let mut group = String::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("group") {
            let rest = rest.trim_start();
            if let Some(value) = rest.strip_prefix('=') {
                group = value.trim().trim_matches(['"', '\'']).to_owned();
            }
        }
    }
    let mut dependencies: Vec<Dependency> = gradle_coordinates(text, catalog)
        .into_iter()
        .map(|(group, artifact, _, runtime)| Dependency {
            name: format!("{group}:{artifact}"),
            code_name: group,
            runtime,
        })
        .collect();
    dedup(&mut dependencies);
    Ok(Candidate {
        info: Some(maven_package(&group, name, dir, dependencies)?),
        members: Vec::new(),
        exclude: Vec::new(),
    })
}

/// The coordinates a Gradle build declares: group, artifact, version when written, and
/// whether the configuration is at run time. `platform(…)` and project dependencies aside.
pub fn gradle_coordinates(
    text: &str,
    catalog: &str,
) -> Vec<(String, String, Option<String>, bool)> {
    let libraries = gradle_catalog(catalog);
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(configuration) = GRADLE_CONFIGURATIONS.iter().find(|c| {
            line.strip_prefix(**c)
                .is_some_and(|rest| rest.starts_with(['(', ' ', '\t']))
        }) else {
            continue;
        };
        let rest = line[configuration.len()..]
            .trim_start()
            .trim_start_matches('(')
            .trim_start();
        let runtime = !GRADLE_DEV.contains(configuration);
        if let Some(quote) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') {
            let inner = rest[1..].split(quote).next().unwrap_or("");
            let mut parts = inner.split(':');
            if let (Some(group), Some(artifact)) = (parts.next(), parts.next())
                && !group.is_empty()
                && !artifact.is_empty()
            {
                out.push((
                    group.to_owned(),
                    artifact.to_owned(),
                    parts.next().map(str::to_owned),
                    runtime,
                ));
            }
        } else if let Some(alias) = rest.strip_prefix("libs.") {
            let alias: String = alias
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '.' || *c == '_')
                .collect();
            if let Some((group, artifact, version)) = libraries.get(&alias) {
                out.push((group.clone(), artifact.clone(), version.clone(), runtime));
            }
        }
    }
    out
}

/// A version catalog's libraries (`gradle/libs.versions.toml`), by the accessor the builds
/// write: `spring-boot-web` is `libs.spring.boot.web`.
fn gradle_catalog(text: &str) -> BTreeMap<String, (String, String, Option<String>)> {
    let mut out = BTreeMap::new();
    let Ok(catalog) = text.parse::<toml::Table>() else {
        return out;
    };
    let versions = catalog.get("versions").and_then(toml::Value::as_table);
    let Some(libraries) = catalog.get("libraries").and_then(toml::Value::as_table) else {
        return out;
    };
    for (alias, library) in libraries {
        let (coordinates, version) = match library {
            toml::Value::String(s) => (s.clone(), None),
            toml::Value::Table(t) => {
                let module = t
                    .get("module")
                    .and_then(toml::Value::as_str)
                    .map(str::to_owned)
                    .or_else(|| {
                        Some(format!(
                            "{}:{}",
                            t.get("group")?.as_str()?,
                            t.get("name")?.as_str()?
                        ))
                    });
                let version = match t.get("version") {
                    Some(toml::Value::String(v)) => Some(v.clone()),
                    Some(toml::Value::Table(v)) => v
                        .get("ref")
                        .and_then(toml::Value::as_str)
                        .and_then(|r| versions?.get(r)?.as_str())
                        .map(str::to_owned),
                    _ => None,
                };
                let Some(module) = module else { continue };
                (module, version)
            }
            _ => continue,
        };
        let mut parts = coordinates.split(':');
        let (Some(group), Some(artifact)) = (parts.next(), parts.next()) else {
            continue;
        };
        let version = version.or_else(|| parts.next().map(str::to_owned));
        let accessor = alias.replace(['-', '_'], ".");
        out.insert(accessor, (group.to_owned(), artifact.to_owned(), version));
    }
    out
}

/// `settings.gradle(.kts)`: no package; the projects it `include`s are members
/// (`":libs:core"` → `libs/core`).
fn gradle_settings(text: &str, dir: &str) -> Candidate {
    let mut members = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix("include") else {
            continue;
        };
        for (i, part) in rest.split(['"', '\'']).enumerate() {
            if i % 2 == 1 && !part.is_empty() {
                members.push(under(dir, &part.trim_start_matches(':').replace(':', "/")));
            }
        }
    }
    Candidate {
        info: None,
        members,
        exclude: Vec::new(),
    }
}

/// `Package.swift` (ADR 0067), read as text: the package's `name:`, the packages it depends on
/// (`.package(url: "https://github.com/apple/swift-log.git", from: "1.6.0")` →
/// `github.com/apple/swift-log`), each with the modules its targets take from it
/// (`.product(name: "Logging", package: "swift-log")`), its code names, comma-separated. A
/// dependency of test targets alone is not at run time.
fn swift_package(text: &str, dir: &str) -> Result<Candidate, String> {
    let name = swift_argument(text, "name").ok_or("Package.swift names no package")?;
    let packages = swift_packages(text);
    let mut dependencies = Vec::new();
    for (location, _) in &packages {
        let identity = location
            .rsplit('/')
            .next()
            .unwrap_or(location)
            .to_lowercase();
        let mut modules = Vec::new();
        let mut runtime = false;
        for (target, test) in swift_targets(text) {
            for product in swift_products(&target) {
                if product.1.to_lowercase() == identity && !modules.contains(&product.0) {
                    modules.push(product.0.clone());
                }
                if product.1.to_lowercase() == identity && !test {
                    runtime = true;
                }
            }
        }
        if modules.is_empty() {
            modules.push(identity.clone());
            runtime = true;
        }
        dependencies.push(Dependency {
            name: location.clone(),
            code_name: modules.join(","),
            runtime,
        });
    }
    dependencies.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Candidate {
        info: Some(PackageInfo {
            id: NodeId::package("swiftpm", &name).map_err(|e| e.to_string())?,
            code_name: name.clone(),
            name,
            ecosystem: Ecosystem::Swiftpm,
            dir: dir.to_owned(),
            lib_path: None,
            has_lib: false,
            bins: Vec::new(),
            dependencies,
        }),
        members: Vec::new(),
        exclude: Vec::new(),
    })
}

/// The first string argument `label: "…"` of a Swift text.
fn swift_argument(text: &str, label: &str) -> Option<String> {
    let at = text.find(&format!("{label}:"))?;
    let rest = &text[at + label.len() + 1..];
    let open = rest.find('"')?;
    let close = rest[open + 1..].find('"')?;
    Some(rest[open + 1..open + 1 + close].to_owned())
}

/// The remote packages of a `Package.swift`: their location without scheme nor `.git`
/// (`github.com/apple/swift-log`), or a registry identity, and the version they start from.
pub fn swift_packages(text: &str) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    for call in text.split(".package(").skip(1) {
        let call = call.split(')').next().unwrap_or("");
        let location = swift_argument(call, "url")
            .map(|url| {
                url.trim_start_matches("https://")
                    .trim_start_matches("http://")
                    .trim_start_matches("git@")
                    .replace(':', "/")
                    .trim_end_matches(".git")
                    .trim_end_matches('/')
                    .to_owned()
            })
            .or_else(|| swift_argument(call, "id"));
        let Some(location) = location else { continue };
        let version = ["from", "exact", "branch"]
            .iter()
            .find_map(|label| swift_argument(call, label))
            .or_else(|| {
                // `"1.0.0"..<"2.0.0"`, `.upToNextMajor(from: "1.0.0")`.
                let open = call.find('"')?;
                let rest = &call[open + 1..];
                let close = rest.find('"')?;
                Some(rest[..close].to_owned())
                    .filter(|v| v.chars().next().is_some_and(|c| c.is_ascii_digit()))
            });
        out.push((location, version));
    }
    out
}

/// The targets of a `Package.swift`, each with its text, and whether it is a test target.
fn swift_targets(text: &str) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    for (marker, test) in [
        (".target(", false),
        (".executableTarget(", false),
        (".testTarget(", true),
    ] {
        let mut rest = text;
        while let Some(at) = rest.find(marker) {
            let body = &rest[at + marker.len()..];
            let mut depth = 1;
            let mut end = body.len();
            for (i, c) in body.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            out.push((body[..end].to_owned(), test));
            rest = &body[end..];
        }
    }
    out
}

/// The `.product(name:, package:)` a target takes: (module, package identity).
fn swift_products(target: &str) -> Vec<(String, String)> {
    target
        .split(".product(")
        .skip(1)
        .filter_map(|call| {
            let call = call.split(')').next()?;
            Some((
                swift_argument(call, "name")?,
                swift_argument(call, "package")?,
            ))
        })
        .collect()
}

/// Swift module paths (ADR 0067): the target, its folder under `Sources/` or `Tests/`, then
/// the folders and the file's stem. `Sources/Shop/Model/Item.swift` → `Shop/Model/Item`.
pub(crate) fn swift_module(relative: &str) -> ModulePath {
    let inside = ["Sources/", "Tests/"]
        .iter()
        .find_map(|root| relative.strip_prefix(root))
        .unwrap_or(relative);
    let segments: Vec<String> = without_extension(inside)
        .split('/')
        .map(str::to_owned)
        .collect();
    ModulePath {
        root_len: segments.len().min(1),
        segments,
    }
}

/// `build.sbt` (ADR 0071), read as text: `organization` and `name` (`com.shop:shop`), and the
/// dependencies its settings declare, by group; a `% Test` one is not at run time.
fn sbt_build(text: &str, dir: &str) -> Result<Candidate, String> {
    let setting = |key: &str| sbt_setting(text, key);
    let name = setting("name").unwrap_or_else(|| {
        dir.rsplit('/')
            .next()
            .filter(|d| !d.is_empty())
            .unwrap_or("root")
            .to_owned()
    });
    let organization = setting("organization").unwrap_or_default();
    let mut dependencies: Vec<Dependency> = sbt_dependencies(text)
        .into_iter()
        .map(|d| Dependency {
            name: format!("{}:{}", d.group, d.artifact),
            code_name: d.group,
            runtime: !d.test,
        })
        .collect();
    dedup(&mut dependencies);
    let id = if organization.is_empty() {
        name.clone()
    } else {
        format!("{organization}:{name}")
    };
    Ok(Candidate {
        info: Some(PackageInfo {
            id: NodeId::package("sbt", &id).map_err(|e| e.to_string())?,
            name: id,
            ecosystem: Ecosystem::Sbt,
            dir: dir.to_owned(),
            code_name: organization,
            lib_path: None,
            has_lib: false,
            bins: Vec::new(),
            dependencies,
        }),
        members: Vec::new(),
        exclude: Vec::new(),
    })
}

/// The value of `key := "…"` in a `build.sbt`, `ThisBuild / key` included.
pub fn sbt_setting(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let line = line.trim();
        let rest = line
            .strip_prefix(&format!("ThisBuild / {key}"))
            .or_else(|| line.strip_prefix(key))?;
        let rest = rest.trim_start().strip_prefix(":=")?.trim();
        let value = rest.strip_prefix('"')?.split('"').next()?;
        Some(value.to_owned())
    })
}

/// A dependency of a `build.sbt`.
#[derive(Debug, PartialEq, Eq)]
pub struct SbtDependency {
    pub group: String,
    /// With the Scala suffix of `%%` (`upickle_3`), from `scalaVersion`.
    pub artifact: String,
    pub version: Option<String>,
    pub test: bool,
}

/// The `"group" %% "artifact" % "version"` of a `build.sbt`, `% Test` marking the tests'.
pub fn sbt_dependencies(text: &str) -> Vec<SbtDependency> {
    let scala = sbt_setting(text, "scalaVersion").unwrap_or_default();
    let suffix = if scala.starts_with('3') {
        "_3".to_owned()
    } else {
        let mut parts = scala.split('.');
        match (parts.next(), parts.next()) {
            (Some(major), Some(minor)) => format!("_{major}.{minor}"),
            _ => String::new(),
        }
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let code = line.split("//").next().unwrap_or("");
        // Each `"a" %% "b" % "c"` of the line, in order.
        let mut rest = code;
        while let Some(start) = rest.find('"') {
            let tokens: Vec<&str> = rest[start..].splitn(8, '"').collect();
            // ["", group, op, artifact, op, version, after…]
            if tokens.len() < 6 {
                break;
            }
            let (group, op, artifact, op2) =
                (tokens[1], tokens[2].trim(), tokens[3], tokens[4].trim());
            if matches!(op, "%" | "%%" | "%%%") && op2 == "%" && !group.is_empty() {
                let after = tokens.get(6).copied().unwrap_or("");
                let test = after.trim_start().strip_prefix('%').is_some_and(|t| {
                    t.trim_start().starts_with("Test") || t.trim_start().starts_with("\"test")
                });
                out.push(SbtDependency {
                    group: group.to_owned(),
                    artifact: if op == "%" {
                        artifact.to_owned()
                    } else {
                        format!("{artifact}{suffix}")
                    },
                    version: Some(tokens[5].to_owned()),
                    test,
                });
                let consumed: usize = tokens[..6].iter().map(|t| t.len() + 1).sum();
                rest = &rest[start + consumed.min(rest.len() - start)..];
            } else {
                rest = &rest[start + 1..];
            }
        }
    }
    out
}

/// `composer.json` (ADR 0070): `vendor/package`, its PSR-4 root (`"Shop\\": "src/"`: the
/// namespace as code name, the folder as `lib_path`), its `require` at run time and its
/// `require-dev` apart; `php` and extensions (`ext-json`) are not packages. A dependency's
/// code names are the namespaces `composer.lock` says it autoloads, comma-separated; without the
/// lock, its vendor's name.
fn composer(text: &str, dir: &str, lock: &str) -> Result<Candidate, String> {
    let manifest: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let name = manifest["name"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| dir.rsplit('/').next().unwrap_or("project").to_owned());
    let (namespace, folder) = manifest["autoload"]["psr-4"]
        .as_object()
        .and_then(|map| map.iter().next())
        .map(|(ns, folder)| {
            let folder = match folder {
                serde_json::Value::Array(list) => list.first().and_then(|f| f.as_str()),
                other => other.as_str(),
            };
            (
                ns.trim_end_matches('\\').to_owned(),
                folder.map(|f| f.trim_end_matches('/').to_owned()),
            )
        })
        .unwrap_or_default();
    let lock: serde_json::Value = serde_json::from_str(lock).unwrap_or_default();
    let namespaces = |package: &str| -> String {
        let locked: Vec<String> = ["packages", "packages-dev"]
            .iter()
            .flat_map(|k| lock[k].as_array().cloned().unwrap_or_default())
            .filter(|p| p["name"].as_str() == Some(package))
            .flat_map(|p| {
                p["autoload"]["psr-4"]
                    .as_object()
                    .map(|m| {
                        m.keys()
                            .map(|k| k.trim_end_matches('\\').to_owned())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .collect();
        if locked.is_empty() {
            package.split('/').next().unwrap_or(package).to_owned()
        } else {
            locked.join(",")
        }
    };
    let mut dependencies = Vec::new();
    for (section, runtime) in [("require", true), ("require-dev", false)] {
        for package in manifest[section]
            .as_object()
            .into_iter()
            .flat_map(|m| m.keys())
        {
            if !package.contains('/') {
                continue;
            }
            dependencies.push(Dependency {
                name: package.clone(),
                code_name: namespaces(package),
                runtime,
            });
        }
    }
    dedup(&mut dependencies);
    Ok(Candidate {
        info: Some(PackageInfo {
            id: NodeId::package("composer", &name).map_err(|e| e.to_string())?,
            name,
            ecosystem: Ecosystem::Composer,
            dir: dir.to_owned(),
            code_name: namespace,
            lib_path: folder.filter(|f| !f.is_empty()),
            has_lib: false,
            bins: Vec::new(),
            dependencies,
        }),
        members: Vec::new(),
        exclude: Vec::new(),
    })
}

/// PHP module paths (ADR 0070): under the PSR-4 folder, the namespace and the path
/// (`src/Model/Item.php` with `"Shop\\": "src/"` → `Shop\Model\Item`); elsewhere, the path.
pub(crate) fn php_module(package: Option<&PackageInfo>, relative: &str) -> ModulePath {
    let stem = without_extension(relative);
    let mapped = package.and_then(|p| {
        let folder = p.lib_path.as_deref()?;
        let rest = stem.strip_prefix(&format!("{folder}/"))?;
        let mut segments: Vec<String> = p
            .code_name
            .split('\\')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        segments.extend(rest.split('/').map(str::to_owned));
        Some(segments)
    });
    let segments = mapped.unwrap_or_else(|| stem.split('/').map(str::to_owned).collect());
    ModulePath {
        root_len: 0,
        segments,
    }
}

/// The packages a .NET test project takes to run its tests: a project that references one is
/// a test project, whose dependencies are not at run time (ADR 0069).
pub const DOTNET_TEST_SDK: &str = "Microsoft.NET.Test.Sdk";

/// A `.csproj` (ADR 0069): its assembly (`<AssemblyName>`, else the file's name), its root
/// namespace as code name, its `<PackageReference>`s, and its `<ProjectReference>`s, which are
/// packages of the repository. A test project's dependencies are not at run time.
fn csharp_project(text: &str, dir: &str, stem: &str) -> Result<Candidate, String> {
    // `dotnet new` writes a byte order mark.
    let document = roxmltree::Document::parse(text.trim_start_matches('\u{feff}'))
        .map_err(|e| e.to_string())?;
    let property = |name: &str| {
        document
            .descendants()
            .find(|n| n.has_tag_name(name))
            .and_then(|n| n.text())
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
    };
    let name = property("AssemblyName").unwrap_or_else(|| stem.to_owned());
    let code_name = property("RootNamespace").unwrap_or_else(|| name.clone());
    let packages: Vec<String> = document
        .descendants()
        .filter(|n| n.has_tag_name("PackageReference"))
        .filter_map(|n| n.attribute("Include").map(str::to_owned))
        .collect();
    let test = packages.iter().any(|p| p == DOTNET_TEST_SDK);
    let mut dependencies: Vec<Dependency> = packages
        .into_iter()
        .map(|package| Dependency {
            code_name: package.clone(),
            name: package,
            runtime: !test,
        })
        .collect();
    dedup(&mut dependencies);
    Ok(Candidate {
        info: Some(PackageInfo {
            id: NodeId::package("nuget", &name).map_err(|e| e.to_string())?,
            name,
            ecosystem: Ecosystem::Nuget,
            dir: dir.to_owned(),
            code_name,
            lib_path: None,
            has_lib: false,
            bins: Vec::new(),
            dependencies,
        }),
        members: Vec::new(),
        exclude: Vec::new(),
    })
}

/// The test frameworks of C++: dependencies for the tests, not at run time.
const CPP_TESTS: [&str; 5] = ["gtest", "catch2", "doctest", "benchmark", "boost-test"];

/// `CMakeLists.txt` (ADR 0065): a package when it declares a `project(…)`; its
/// `add_subdirectory` are members; its dependencies are its `find_package` and
/// `FetchContent_Declare`, then those of `vcpkg.json` and Conan beside it. A dependency's code
/// name is the first folder its headers are included from (`fmt`, `boost`, `nlohmann`).
fn cmake_lists(text: &str, dir: &str, vcpkg: &str, conan: &str) -> Candidate {
    let commands = cmake_commands(text);
    let project = commands
        .iter()
        .find(|(name, _)| name == "project")
        .and_then(|(_, args)| args.first().cloned());
    let members = commands
        .iter()
        .filter(|(name, _)| name == "add_subdirectory")
        .filter_map(|(_, args)| args.first())
        .map(|sub| under(dir, sub))
        .collect();
    let mut dependencies = Vec::new();
    let mut add = |name: &str| {
        let code_name = cpp_code_name(name);
        if !dependencies
            .iter()
            .any(|d: &Dependency| d.code_name == code_name)
        {
            dependencies.push(Dependency {
                name: name.to_owned(),
                runtime: !CPP_TESTS.contains(&code_name.as_str()),
                code_name,
            });
        }
    };
    for (name, _) in vcpkg_dependencies(vcpkg) {
        add(&name);
    }
    for (name, _) in conan_requires(conan) {
        add(&name);
    }
    for (command, args) in &commands {
        if matches!(command.as_str(), "find_package" | "fetchcontent_declare")
            && let Some(name) = args.first()
        {
            add(name);
        }
    }
    dependencies.sort_by(|a, b| a.name.cmp(&b.name));
    let info = project.and_then(|name| {
        Some(PackageInfo {
            id: NodeId::package("cmake", &name).ok()?,
            code_name: name.clone(),
            name,
            ecosystem: Ecosystem::Cmake,
            dir: dir.to_owned(),
            lib_path: None,
            has_lib: false,
            bins: Vec::new(),
            dependencies,
        })
    });
    Candidate {
        info,
        members,
        exclude: Vec::new(),
    }
}

/// The commands of a CMake file, lower-cased, with their arguments, comments and quotes
/// removed (`project("shop" VERSION 1.0)` → `("project", ["shop", "VERSION", "1.0"])`).
pub fn cmake_commands(text: &str) -> Vec<(String, Vec<String>)> {
    let code: String = text
        .lines()
        .map(|l| l.split('#').next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    let mut out = Vec::new();
    let mut rest = code.as_str();
    while let Some(open) = rest.find('(') {
        let name: String = rest[..open]
            .chars()
            .rev()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let mut depth = 0;
        let mut close = None;
        for (i, c) in rest[open..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(open + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(close) = close else { break };
        let args = rest[open + 1..close]
            .split_whitespace()
            .map(|a| a.trim_matches('"').to_owned())
            .filter(|a| !a.is_empty())
            .collect();
        if !name.is_empty() {
            out.push((name.to_lowercase(), args));
        }
        rest = &rest[close + 1..];
    }
    out
}

/// `vcpkg.json`'s dependencies, each with the version it asks for (`version>=`), if any.
pub fn vcpkg_dependencies(text: &str) -> Vec<(String, Option<String>)> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let overrides: BTreeMap<String, String> = json["overrides"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|o| {
            Some((
                o["name"].as_str()?.to_owned(),
                o["version"].as_str()?.to_owned(),
            ))
        })
        .collect();
    json["dependencies"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|d| {
            let name = d.as_str().or_else(|| d["name"].as_str())?.to_owned();
            let version = overrides
                .get(&name)
                .cloned()
                .or_else(|| d["version>="].as_str().map(str::to_owned));
            Some((name, version))
        })
        .collect()
}

/// Conan's requirements, from `conanfile.txt` (`[requires]`, `[test_requires]`) or
/// `conanfile.py` (`requires = …`, `self.requires(…)`): `fmt/10.2.1` → `("fmt", "10.2.1")`.
pub fn conan_requires(text: &str) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    let mut section = "";
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            section = line;
            continue;
        }
        let candidates: Vec<&str> = if matches!(
            section,
            "[requires]" | "[test_requires]" | "[tool_requires]"
        ) && !line.contains('"')
        {
            vec![line]
        } else if line.contains("requires") {
            line.split(['"', '\'']).skip(1).step_by(2).collect()
        } else {
            Vec::new()
        };
        for reference in candidates {
            let reference = reference.split('@').next().unwrap_or("").trim();
            if let Some((name, version)) = reference.split_once('/')
                && !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            {
                out.push((
                    name.to_owned(),
                    Some(version.trim_matches(['[', ']']).to_owned()),
                ));
            }
        }
    }
    out
}

/// The folder a C++ dependency's headers are included from: lower case, cut at its first `-`
/// or `_` (`nlohmann-json`, `nlohmann_json` → `nlohmann`; `boost-asio` → `boost`; `GTest` →
/// `gtest`; `SQLite3` and vcpkg's `unofficial-sqlite3` → `sqlite3`).
pub fn cpp_code_name(name: &str) -> String {
    let name = name.strip_prefix("unofficial-").unwrap_or(name);
    name.split(['-', '_', ':'])
        .next()
        .unwrap_or(name)
        .to_lowercase()
}

/// The name of a PEP 508 requirement (`requests[socks]>=2; python_version<"4"` → `requests`),
/// normalised.
pub(crate) fn requirement_name(requirement: &str) -> Option<String> {
    let name: String = requirement
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect();
    (!name.is_empty()).then(|| normalise_python(&name))
}

/// PEP 503: lower case, and every run of `-`, `_` and `.` one `-`.
pub(crate) fn normalise_python(name: &str) -> String {
    let mut normal = String::with_capacity(name.len());
    for c in name.chars() {
        if matches!(c, '-' | '_' | '.') {
            if !normal.ends_with('-') {
                normal.push('-');
            }
        } else {
            normal.push(c.to_ascii_lowercase());
        }
    }
    normal
}

/// One entry per name; a runtime declaration wins over a dev one.
fn dedup(dependencies: &mut Vec<Dependency>) {
    dependencies.sort_by(|a, b| a.name.cmp(&b.name).then(b.runtime.cmp(&a.runtime)));
    dependencies.dedup_by(|later, kept| later.name == kept.name);
}

/// The module path of a file and how many of its first segments name its crate root
/// (`crate::` in Rust). `relative` is the path inside the package, or inside the repository
/// for a file outside any package.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModulePath {
    pub segments: Vec<String>,
    pub root_len: usize,
}

/// Rust module paths (ADR 0013).
pub(crate) fn rust_module(
    package: Option<&PackageInfo>,
    has_lib: bool,
    relative: &str,
) -> ModulePath {
    let stem = |path: &str| -> Vec<String> {
        let path = path.strip_suffix(".rs").unwrap_or(path);
        let mut segments: Vec<String> = path.split('/').map(str::to_owned).collect();
        if segments.len() > 1 && segments.last().is_some_and(|s| s == "mod") {
            segments.pop();
        }
        segments
    };
    let Some(package) = package else {
        let segments = stem(relative);
        return ModulePath {
            root_len: segments.len().min(1),
            segments,
        };
    };
    let krate = package.code_name.clone();
    let with = |prefix: &[&str], rest: Vec<String>, root_len: usize| {
        let mut segments = vec![krate.clone()];
        segments.extend(prefix.iter().map(|s| (*s).to_owned()));
        segments.extend(rest);
        ModulePath { segments, root_len }
    };
    let lib_path = package.lib_path.as_deref().unwrap_or("src/lib.rs");
    if relative == lib_path {
        return with(&[], Vec::new(), 1);
    }
    if let Some((name, _)) = package.bins.iter().find(|(_, path)| path == relative) {
        return if has_lib || package.bins.len() > 1 {
            with(&["@bin", name], Vec::new(), 3)
        } else {
            with(&[], Vec::new(), 1)
        };
    }
    if relative == "src/main.rs" {
        return if has_lib {
            with(&["@bin", &package.name.replace('-', "_")], Vec::new(), 3)
        } else {
            with(&[], Vec::new(), 1)
        };
    }
    if let Some(rest) = relative.strip_prefix("src/bin/") {
        let mut segments = stem(rest);
        if segments.len() > 1 && segments.last().is_some_and(|s| s == "main") {
            segments.pop();
        }
        let name = segments.remove(0);
        return with(&["@bin", &name], segments, 3);
    }
    for target in ["tests", "examples", "benches"] {
        if let Some(rest) = relative.strip_prefix(&format!("{target}/")) {
            let mut segments = stem(rest);
            if segments.len() > 1 && segments.last().is_some_and(|s| s == "main") {
                segments.pop();
            }
            let target = format!("@{target}");
            return with(&[target.as_str()], segments, 3);
        }
    }
    if relative == "build.rs" {
        return with(&["@build"], Vec::new(), 2);
    }
    let lib_dir = lib_path.rsplit_once('/').map_or("", |(dir, _)| dir);
    let inside = if lib_dir.is_empty() {
        Some(relative)
    } else {
        relative.strip_prefix(&format!("{lib_dir}/"))
    };
    with(&[], stem(inside.unwrap_or(relative)), 1)
}

/// TypeScript module paths: the package name, then the path inside it without extension.
pub(crate) fn npm_module(package: Option<&PackageInfo>, relative: &str) -> ModulePath {
    let file = without_extension(relative);
    let mut segments: Vec<String> = match package {
        Some(p) => p.code_name.split('/').map(str::to_owned).collect(),
        None => Vec::new(),
    };
    let root_len = segments.len();
    segments.extend(file.split('/').map(str::to_owned));
    ModulePath { segments, root_len }
}

/// Python module paths (ADR 0062): the dotted path from the source root (`src/` when the package
/// has one, else its directory), `__init__` being its package. `src/shop/cart.py` → `shop.cart`.
pub(crate) fn python_module(package: Option<&PackageInfo>, relative: &str) -> ModulePath {
    let inside = package
        .and_then(|p| p.lib_path.as_deref())
        .and_then(|src| relative.strip_prefix(&format!("{src}/")))
        .unwrap_or(relative);
    let mut segments: Vec<String> = without_extension(inside)
        .split('/')
        .map(str::to_owned)
        .collect();
    if segments.len() > 1 && segments.last().is_some_and(|s| s == "__init__") {
        segments.pop();
    }
    ModulePath {
        root_len: segments.len().min(1),
        segments,
    }
}

/// Go module paths (ADR 0063): the module path, the package's directory inside it, then the
/// file's stem, which keeps two `init` functions of one package apart.
/// `example.com/shop` + `cart/cart.go` → `example.com/shop/cart/cart`.
pub(crate) fn go_module(package: Option<&PackageInfo>, relative: &str) -> ModulePath {
    let mut segments: Vec<String> = match package {
        Some(p) => p.code_name.split('/').map(str::to_owned).collect(),
        None => Vec::new(),
    };
    let root_len = segments.len();
    segments.extend(without_extension(relative).split('/').map(str::to_owned));
    ModulePath { segments, root_len }
}

/// Java module paths (ADR 0064): the dotted path from the source root (`src/<set>/java`, or
/// `src`), with the file's stem, the class it holds. `src/main/java/com/shop/Cart.java` →
/// `com.shop.Cart`.
pub(crate) fn java_module(relative: &str) -> ModulePath {
    let parts: Vec<&str> = relative.split('/').collect();
    let start = parts
        .windows(3)
        .position(|w| w[0] == "src" && matches!(w[2], "java" | "kotlin" | "scala"))
        .map(|i| i + 3)
        .or_else(|| parts.iter().position(|p| *p == "src").map(|i| i + 1))
        .unwrap_or(0);
    let inside = parts[start.min(parts.len())..].join("/");
    let segments: Vec<String> = without_extension(&inside)
        .split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    ModulePath {
        root_len: 0,
        segments,
    }
}

/// C++ module paths (ADR 0065): the file's path from the repository root, extension included,
/// since a header and its source share a stem.
pub(crate) fn cpp_module(package: Option<&PackageInfo>, relative: &str) -> ModulePath {
    let path = match package {
        Some(p) if !p.dir.is_empty() => format!("{}/{relative}", p.dir),
        _ => relative.to_owned(),
    };
    ModulePath {
        root_len: 0,
        segments: path.split('/').map(str::to_owned).collect(),
    }
}

/// `src/App.tsx` → `src/App`; `types/vscode.d.ts` → `types/vscode`; `src/cart.mjs` → `src/cart`.
pub(crate) fn without_extension(path: &str) -> &str {
    for extension in [
        ".d.ts", ".d.mts", ".d.cts", ".tsx", ".ts", ".mts", ".cts", ".jsx", ".js", ".mjs", ".cjs",
        ".rs", ".pyi", ".py", ".go", ".java", ".kts", ".kt", ".swift", ".cs", ".php", ".scala",
        ".sc",
    ] {
        if let Some(stem) = path.strip_suffix(extension) {
            return stem;
        }
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, files: &[(&str, &str)]) -> Vec<String> {
        for (path, text) in files {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        files.iter().map(|(p, _)| (*p).to_owned()).collect()
    }

    fn ids(packages: &[PackageInfo]) -> Vec<String> {
        packages
            .iter()
            .map(|p| format!("{} @{}", p.id, p.dir))
            .collect()
    }

    #[test]
    fn workspaces_members_roots_and_strays() {
        let dir = tempfile::tempdir().unwrap();
        let manifests = write(
            dir.path(),
            &[
                (
                    "Cargo.toml",
                    "[workspace]\nmembers = [\"crates/*\"]\nexclude = [\"crates/old\"]\n",
                ),
                (
                    "crates/core/Cargo.toml",
                    "[package]\nname = \"my-core\"\n[dependencies]\nserde = \"1\"\nrenamed = { package = \"real-name\", version = \"1\" }\n[dev-dependencies]\nproptest = \"1\"\n[target.'cfg(unix)'.dependencies]\nlibc = \"0.2\"\n",
                ),
                (
                    "crates/cli/Cargo.toml",
                    "[package]\nname = \"my-cli\"\n[lib]\nname = \"cli\"\n[[bin]]\nname = \"tool\"\npath = \"src/main.rs\"\n[dependencies]\nmy-core = { path = \"../core\" }\n",
                ),
                ("crates/old/Cargo.toml", "[package]\nname = \"old\"\n"),
                // Nested in a package without being a member: its files belong to the package.
                (
                    "crates/core/tests/fixture/Cargo.toml",
                    "[package]\nname = \"fixture\"\n",
                ),
                // Outside any package: standalone.
                ("tools/gen/Cargo.toml", "[package]\nname = \"gen\"\n"),
                (
                    "package.json",
                    r#"{"name": "root-ws", "private": true, "workspaces": ["web", "editors/*"], "devDependencies": {"eslint": "1"}}"#,
                ),
                (
                    "web/package.json",
                    r#"{"name": "@me/web", "dependencies": {"react": "19"}, "devDependencies": {"vite": "8", "react": "19"}}"#,
                ),
                ("editors/vscode/package.json", r#"{"name": "ext"}"#),
                // Not a member, inside the npm root: its files belong to the root.
                ("crates/core/js/package.json", r#"{"name": "core-js"}"#),
            ],
        );
        let (packages, problems) = discover(dir.path(), &manifests);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            ids(&packages),
            [
                "pkg:cargo/gen @tools/gen",
                "pkg:cargo/my-cli @crates/cli",
                "pkg:cargo/my-core @crates/core",
                // Excluded from the workspace, but inside no other package: standalone.
                "pkg:cargo/old @crates/old",
                "pkg:npm/@me/web @web",
                "pkg:npm/ext @editors/vscode",
                "pkg:npm/root-ws @",
            ]
        );
        let core = packages.iter().find(|p| p.name == "my-core").unwrap();
        let deps: Vec<_> = core
            .dependencies
            .iter()
            .map(|d| format!("{}={} {}", d.name, d.code_name, d.runtime))
            .collect();
        assert_eq!(
            deps,
            [
                "libc=libc true",
                "proptest=proptest false",
                "real-name=renamed true",
                "serde=serde true"
            ]
        );
        let cli = packages.iter().find(|p| p.name == "my-cli").unwrap();
        assert_eq!(cli.code_name, "cli");
        assert_eq!(cli.bins, [("tool".to_owned(), "src/main.rs".to_owned())]);
        let web = packages.iter().find(|p| p.name == "@me/web").unwrap();
        assert_eq!(web.dependencies.len(), 2);
        assert!(
            web.dependencies
                .iter()
                .find(|d| d.name == "react")
                .unwrap()
                .runtime
        );
    }

    #[test]
    fn nesting_only_counts_within_an_ecosystem() {
        let dir = tempfile::tempdir().unwrap();
        let manifests = write(
            dir.path(),
            &[
                ("Cargo.toml", "[package]\nname = \"app\"\n"),
                ("web/package.json", r#"{"name": "app-web"}"#),
                ("web/inner/Cargo.toml", "[package]\nname = \"inner\"\n"),
            ],
        );
        assert_eq!(
            ids(&discover(dir.path(), &manifests).0),
            ["pkg:cargo/app @", "pkg:npm/app-web @web"]
        );
    }

    #[test]
    fn broken_manifests_are_reported_and_left_out() {
        let dir = tempfile::tempdir().unwrap();
        let manifests = write(
            dir.path(),
            &[
                ("a/Cargo.toml", "[package\n"),
                ("b/Cargo.toml", "[package]\nversion = \"1\"\n"),
                ("c/package.json", r#"{"name": "c",}"#),
                ("d/Cargo.toml", "[package]\nname = \"d\"\n"),
            ],
        );
        let (packages, problems) = discover(dir.path(), &manifests);
        assert_eq!(ids(&packages), ["pkg:cargo/d @d"]);
        let problems: Vec<String> = problems.iter().map(ToString::to_string).collect();
        assert_eq!(problems.len(), 3, "{problems:?}");
        assert!(problems[0].contains("a/Cargo.toml"), "{problems:?}");
        assert!(problems[1].contains("no name"), "{problems:?}");
        assert!(problems[2].contains("c/package.json"), "{problems:?}");
    }

    /// T-122: a `go.mod` gives its module path, its direct requirements at run time, and the
    /// indirect ones apart; a `go.work` makes no package of its own.
    #[test]
    fn go_modules_and_their_requirements() {
        let dir = tempfile::tempdir().unwrap();
        let manifests = write(
            dir.path(),
            &[
                ("go.work", "go 1.27\n\nuse (\n\t./api\n\t./tools\n)\n"),
                (
                    "api/go.mod",
                    "module example.com/shop/api // the API\n\ngo 1.27\n\nrequire github.com/gin-gonic/gin v1.11.0\n\nrequire (\n\tgolang.org/x/text v0.30.0 // indirect\n\tgithub.com/stretchr/testify v1.11.1\n)\n",
                ),
                ("tools/go.mod", "module \"example.com/shop/tools\"\n"),
            ],
        );
        let (packages, problems) = discover(dir.path(), &manifests);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            ids(&packages),
            [
                "pkg:gomod/example.com/shop/api @api",
                "pkg:gomod/example.com/shop/tools @tools"
            ]
        );
        let api = &packages[0];
        assert_eq!(api.code_name, "example.com/shop/api");
        let dependencies: Vec<(String, bool)> = api
            .dependencies
            .iter()
            .map(|d| (d.name.clone(), d.runtime))
            .collect();
        assert_eq!(
            dependencies,
            [
                ("github.com/gin-gonic/gin".to_owned(), true),
                ("github.com/stretchr/testify".to_owned(), true),
                ("golang.org/x/text".to_owned(), false)
            ]
        );
        let m = go_module(Some(api), "cart/cart.go");
        assert_eq!(m.segments.join("/"), "example.com/shop/api/cart/cart");
        assert_eq!(m.root_len, 3);
    }

    /// T-123: a parent `pom.xml` and its modules, each a package named `group:artifact`, the
    /// group inherited; test dependencies not at run time.
    #[test]
    fn maven_modules_and_their_dependencies() {
        let dir = tempfile::tempdir().unwrap();
        let manifests = write(
            dir.path(),
            &[
                (
                    "pom.xml",
                    "<project><groupId>com.shop</groupId><artifactId>parent</artifactId><packaging>pom</packaging><modules><module>api</module></modules></project>\n",
                ),
                (
                    "api/pom.xml",
                    "<project><parent><groupId>com.shop</groupId><artifactId>parent</artifactId></parent><artifactId>api</artifactId><dependencies><dependency><groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter-web</artifactId></dependency><dependency><groupId>org.junit.jupiter</groupId><artifactId>junit-jupiter</artifactId><scope>test</scope></dependency></dependencies></project>\n",
                ),
            ],
        );
        let (packages, problems) = discover(dir.path(), &manifests);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            ids(&packages),
            ["pkg:maven/com.shop:api @api", "pkg:maven/com.shop:parent @"]
        );
        let dependencies: Vec<(String, String, bool)> = packages[0]
            .dependencies
            .iter()
            .map(|d| (d.name.clone(), d.code_name.clone(), d.runtime))
            .collect();
        assert_eq!(
            dependencies,
            [
                (
                    "org.junit.jupiter:junit-jupiter".to_owned(),
                    "org.junit.jupiter".to_owned(),
                    false
                ),
                (
                    "org.springframework.boot:spring-boot-starter-web".to_owned(),
                    "org.springframework.boot".to_owned(),
                    true
                )
            ]
        );
    }

    /// T-123: Gradle's settings name the projects; a build's coordinates, written out or through
    /// the version catalog, with their configurations.
    #[test]
    fn gradle_projects_and_their_dependencies() {
        let dir = tempfile::tempdir().unwrap();
        let manifests = write(
            dir.path(),
            &[
                (
                    "settings.gradle.kts",
                    "rootProject.name = \"shop\"\ninclude(\"app\", \":libs:core\")\n",
                ),
                (
                    "gradle/libs.versions.toml",
                    "[versions]\nguava = \"33.5.0-jre\"\n\n[libraries]\nguava = { module = \"com.google.guava:guava\", version.ref = \"guava\" }\njunit-jupiter = \"org.junit.jupiter:junit-jupiter:6.1.3\"\n",
                ),
                (
                    "app/build.gradle.kts",
                    "plugins { application }\n\ngroup = \"com.shop\"\n\ndependencies {\n    implementation(libs.guava)\n    implementation(project(\":libs:core\"))\n    testImplementation(libs.junit.jupiter)\n    runtimeOnly(\"org.postgresql:postgresql:42.7.8\")\n}\n",
                ),
                (
                    "libs/core/build.gradle",
                    "dependencies {\n    api 'com.google.code.gson:gson:2.13.2'\n}\n",
                ),
            ],
        );
        let (packages, problems) = discover(dir.path(), &manifests);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            ids(&packages),
            ["pkg:maven/com.shop:app @app", "pkg:maven/core @libs/core"]
        );
        let names = |p: &PackageInfo| -> Vec<(String, bool)> {
            p.dependencies
                .iter()
                .map(|d| (d.name.clone(), d.runtime))
                .collect()
        };
        assert_eq!(
            names(&packages[0]),
            [
                ("com.google.guava:guava".to_owned(), true),
                ("org.junit.jupiter:junit-jupiter".to_owned(), false),
                ("org.postgresql:postgresql".to_owned(), true)
            ]
        );
        assert_eq!(
            names(&packages[1]),
            [("com.google.code.gson:gson".to_owned(), true)]
        );
        let coordinates = gradle_coordinates(
            "dependencies {\n    implementation(libs.guava)\n}\n",
            "[versions]\nguava = \"33.5.0-jre\"\n[libraries]\nguava = { module = \"com.google.guava:guava\", version.ref = \"guava\" }\n",
        );
        assert_eq!(
            coordinates,
            [(
                "com.google.guava".to_owned(),
                "guava".to_owned(),
                Some("33.5.0-jre".to_owned()),
                true
            )]
        );
        for (rel, expected) in [
            ("src/main/java/com/shop/Cart.java", "com.shop.Cart"),
            ("src/test/java/com/shop/CartTest.java", "com.shop.CartTest"),
            ("src/integrationTest/java/com/shop/It.java", "com.shop.It"),
            ("src/com/shop/Old.java", "com.shop.Old"),
            ("Main.java", "Main"),
        ] {
            assert_eq!(java_module(rel).segments.join("."), expected, "{rel}");
        }
    }

    /// T-124: a CMake project and its dependencies: `find_package`, `FetchContent`, vcpkg and
    /// Conan, one per header folder; a subdirectory without `project()` is no package.
    #[test]
    fn cmake_projects_and_their_dependencies() {
        let dir = tempfile::tempdir().unwrap();
        let manifests = write(
            dir.path(),
            &[
                (
                    "CMakeLists.txt",
                    "cmake_minimum_required(VERSION 3.24)\nproject(\"shop\" VERSION 1.0)\n# find_package(Ignored)\nfind_package(Boost REQUIRED COMPONENTS asio)\ninclude(FetchContent)\nFetchContent_Declare(\n  spdlog\n  GIT_REPOSITORY https://github.com/gabime/spdlog.git\n  GIT_TAG v1.15.3)\nadd_subdirectory(lib)\n",
                ),
                (
                    "vcpkg.json",
                    r#"{"dependencies": ["boost-asio", {"name": "nlohmann-json", "version>=": "3.12.0"}]}"#,
                ),
                (
                    "conanfile.txt",
                    "[requires]\nfmt/11.2.0\n\n[test_requires]\ngtest/1.17.0\n\n[generators]\nCMakeToolchain\n",
                ),
                ("lib/CMakeLists.txt", "add_library(core core.cpp)\n"),
            ],
        );
        let (packages, problems) = discover(dir.path(), &manifests);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(ids(&packages), ["pkg:cmake/shop @"]);
        let dependencies: Vec<(String, String, bool)> = packages[0]
            .dependencies
            .iter()
            .map(|d| (d.name.clone(), d.code_name.clone(), d.runtime))
            .collect();
        let want = |n: &str, c: &str, r: bool| (n.to_owned(), c.to_owned(), r);
        assert_eq!(
            dependencies,
            [
                want("boost-asio", "boost", true),
                want("fmt", "fmt", true),
                want("gtest", "gtest", false),
                want("nlohmann-json", "nlohmann", true),
                want("spdlog", "spdlog", true)
            ]
        );
        assert_eq!(
            conan_requires(
                "requires = \"zlib/1.3.1\", \"openssl/[>=3 <4]\"\nself.requires(\"fmt/11.2.0@user/channel\")\n"
            ),
            [
                ("zlib".to_owned(), Some("1.3.1".to_owned())),
                ("openssl".to_owned(), Some(">=3 <4".to_owned())),
                ("fmt".to_owned(), Some("11.2.0".to_owned()))
            ]
        );
        assert_eq!(
            vcpkg_dependencies(
                r#"{"dependencies": ["fmt"], "overrides": [{"name": "fmt", "version": "11.2.0"}]}"#
            ),
            [("fmt".to_owned(), Some("11.2.0".to_owned()))]
        );
    }

    /// T-128: a `Package.swift`: its name, its remote packages by location, with the modules its
    /// targets take from them; a package of test targets alone is not at run time.
    #[test]
    fn swift_packages_and_their_products() {
        let dir = tempfile::tempdir().unwrap();
        let manifests = write(
            dir.path(),
            &[(
                "Package.swift",
                "// swift-tools-version: 6.0\nimport PackageDescription\n\nlet package = Package(\n    name: \"Shop\",\n    dependencies: [\n        .package(url: \"https://github.com/apple/swift-log.git\", from: \"1.6.0\"),\n        .package(url: \"https://github.com/pointfreeco/swift-snapshot-testing\", exact: \"1.18.4\"),\n        .package(path: \"../Local\"),\n    ],\n    targets: [\n        .target(name: \"Shop\", dependencies: [.product(name: \"Logging\", package: \"swift-log\")]),\n        .testTarget(name: \"ShopTests\", dependencies: [\"Shop\", .product(name: \"SnapshotTesting\", package: \"swift-snapshot-testing\")]),\n    ]\n)\n",
            )],
        );
        let (packages, problems) = discover(dir.path(), &manifests);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(ids(&packages), ["pkg:swiftpm/Shop @"]);
        let dependencies: Vec<(String, String, bool)> = packages[0]
            .dependencies
            .iter()
            .map(|d| (d.name.clone(), d.code_name.clone(), d.runtime))
            .collect();
        assert_eq!(
            dependencies,
            [
                (
                    "github.com/apple/swift-log".to_owned(),
                    "Logging".to_owned(),
                    true
                ),
                (
                    "github.com/pointfreeco/swift-snapshot-testing".to_owned(),
                    "SnapshotTesting".to_owned(),
                    false
                )
            ]
        );
        assert_eq!(
            swift_module("Sources/Shop/Model/Item.swift").segments,
            ["Shop", "Model", "Item"]
        );
        assert_eq!(
            swift_module("Tests/ShopTests/CartTests.swift").segments,
            ["ShopTests", "CartTests"]
        );
    }

    /// T-132: a `build.sbt`: its organisation and name, its dependencies with the Scala suffix
    /// of `%%`, the `% Test` ones apart.
    #[test]
    fn sbt_builds_and_their_dependencies() {
        let text = "ThisBuild / scalaVersion := \"3.10.0\"\nThisBuild / organization := \"com.shop\"\nlazy val root = (project in file(\".\")).settings(\n  name := \"shop\",\n  libraryDependencies ++= Seq(\n    \"com.lihaoyi\" %% \"upickle\" % \"4.4.3\",\n    \"org.postgresql\" % \"postgresql\" % \"42.7.8\", // the driver\n    \"org.scalameta\" %% \"munit\" % \"1.3.6\" % Test,\n  ),\n)\n";
        let found: Vec<(String, String, bool)> = sbt_dependencies(text)
            .into_iter()
            .map(|d| (d.group, d.artifact, d.test))
            .collect();
        assert_eq!(
            found,
            [
                ("com.lihaoyi".to_owned(), "upickle_3".to_owned(), false),
                ("org.postgresql".to_owned(), "postgresql".to_owned(), false),
                ("org.scalameta".to_owned(), "munit_3".to_owned(), true)
            ]
        );
        let two = sbt_dependencies(
            "scalaVersion := \"2.13.16\"\nlibraryDependencies += \"org.typelevel\" %% \"cats-core\" % \"2.13.0\"\n",
        );
        assert_eq!(two[0].artifact, "cats-core_2.13");
        let dir = tempfile::tempdir().unwrap();
        let manifests = write(dir.path(), &[("build.sbt", text)]);
        let (packages, problems) = discover(dir.path(), &manifests);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(ids(&packages), ["pkg:sbt/com.shop:shop @"]);
    }

    fn package(name: &str, bins: &[(&str, &str)]) -> PackageInfo {
        PackageInfo {
            id: NodeId::package("cargo", name).unwrap(),
            name: name.into(),
            ecosystem: Ecosystem::Cargo,
            dir: "crates/x".into(),
            code_name: name.replace('-', "_"),
            lib_path: None,
            has_lib: true,
            bins: bins
                .iter()
                .map(|(n, p)| ((*n).into(), (*p).into()))
                .collect(),
            dependencies: Vec::new(),
        }
    }

    #[test]
    fn rust_module_paths_follow_the_targets() {
        let lib = package("my-store", &[]);
        let module = |p: Option<&PackageInfo>, has_lib, rel| {
            let m = rust_module(p, has_lib, rel);
            format!("{} /{}", m.segments.join("::"), m.root_len)
        };
        for (rel, expected) in [
            ("src/lib.rs", "my_store /1"),
            ("src/node.rs", "my_store::node /1"),
            ("src/a/mod.rs", "my_store::a /1"),
            ("src/a/b.rs", "my_store::a::b /1"),
            ("src/main.rs", "my_store::@bin::my_store /3"),
            ("src/bin/tool.rs", "my_store::@bin::tool /3"),
            ("src/bin/tool/main.rs", "my_store::@bin::tool /3"),
            ("src/bin/tool/args.rs", "my_store::@bin::tool::args /3"),
            ("tests/graph.rs", "my_store::@tests::graph /3"),
            ("tests/common/mod.rs", "my_store::@tests::common /3"),
            ("examples/bench.rs", "my_store::@examples::bench /3"),
            ("build.rs", "my_store::@build /2"),
            // A module named like a target does not collide with it.
            ("src/build.rs", "my_store::build /1"),
            ("src/tests/x.rs", "my_store::tests::x /1"),
        ] {
            assert_eq!(module(Some(&lib), true, rel), expected, "{rel}");
        }
        // Without a library, `src/main.rs` is the crate root.
        assert_eq!(module(Some(&lib), false, "src/main.rs"), "my_store /1");
        assert_eq!(module(Some(&lib), false, "src/cli.rs"), "my_store::cli /1");
        // Explicit bins, as in timonier-cli.
        let cli = package(
            "timonier-cli",
            &[("timonier", "src/main.rs"), ("tim", "src/bin/tim.rs")],
        );
        assert_eq!(
            module(Some(&cli), true, "src/main.rs"),
            "timonier_cli::@bin::timonier /3"
        );
        assert_eq!(
            module(Some(&cli), true, "src/bin/tim.rs"),
            "timonier_cli::@bin::tim /3"
        );
        // Outside any package (Q14).
        assert_eq!(module(None, false, "scripts/gen.rs"), "scripts::gen /1");
    }

    #[test]
    fn npm_module_paths_start_with_the_package_name() {
        let mut web = package("x", &[]);
        web.code_name = "@me/web".into();
        let m = npm_module(Some(&web), "src/App.tsx");
        assert_eq!(m.segments.join("/"), "@me/web/src/App");
        assert_eq!(m.root_len, 2);
        assert_eq!(
            npm_module(None, "types/vscode.d.ts").segments.join("/"),
            "types/vscode"
        );
    }
}
