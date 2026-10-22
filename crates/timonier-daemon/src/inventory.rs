//! The technology inventory (ADR 0055): the dependencies read in the lockfiles (`cargo
//! metadata`, else `Cargo.lock`; `package-lock.json`), the languages of the graph, the
//! frameworks and functions of the packs, what was added since the review, and the known
//! vulnerabilities (`osv`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;
use timonier_index::ecosystem::{xml_child, xml_text};

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
    if let Ok(text) = std::fs::read_to_string(root.join("pyproject.toml")) {
        let declared = python_declared(&text);
        match std::fs::read_to_string(root.join("uv.lock")) {
            Ok(lock) => merge(&mut out, uv_lock(&lock, &declared)),
            Err(_) => {
                out.notes.push(
                    "pyproject.toml without uv.lock: the declared dependencies, versions \
                     unknown (run `uv lock`)"
                        .to_owned(),
                );
                for (name, (scope, _)) in &declared {
                    out.dependencies.push(Dependency {
                        ecosystem: Ecosystem::Pypi,
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
    }
    if let Ok(text) = std::fs::read_to_string(root.join("go.mod")) {
        merge(&mut out, go_mod(&text));
    }
    merge(&mut out, maven(root));
    merge(&mut out, cpp(root));
    merge(&mut out, swift(root));
    merge(&mut out, dotnet(root));
    merge(&mut out, composer(root));
    merge(&mut out, sbt(root));
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

/// What a `pyproject.toml` declares (ADR 0062): `[project]`'s dependencies as normal, its
/// optional ones and PEP 735 groups as dev, names normalised (PEP 503).
pub fn python_declared(text: &str) -> Declared {
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
pub fn uv_lock(text: &str, declared: &Declared) -> Lockfiles {
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
            out.packages.push((name, version, Ecosystem::Pypi));
            continue;
        }
        let (scope, used_by) = declared
            .get(&name)
            .cloned()
            .unwrap_or((Scope::Normal, BTreeSet::new()));
        out.dependencies.push(Dependency {
            ecosystem: Ecosystem::Pypi,
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

/// `go.mod` (ADR 0063): the module, and every requirement with its version, which Go's
/// minimal version selection builds; `// indirect` ones are not direct. Go records no
/// licences.
pub fn go_mod(text: &str) -> Lockfiles {
    let mut out = Lockfiles::default();
    let mut module = String::new();
    let mut in_block = false;
    for line in text.lines() {
        let (code, comment) = line.split_once("//").unwrap_or((line, ""));
        let mut words = code.split_whitespace();
        let first = words.next();
        let spec: Vec<&str> = match first {
            Some("module") => {
                module = words.next().unwrap_or("").trim_matches('"').to_owned();
                out.packages.push((module.clone(), None, Ecosystem::Go));
                continue;
            }
            Some("require") => match words.next() {
                Some("(") => {
                    in_block = true;
                    continue;
                }
                Some(name) => [name].into_iter().chain(words).collect(),
                None => continue,
            },
            Some(")") => {
                in_block = false;
                continue;
            }
            Some(name) if in_block => [name].into_iter().chain(words).collect(),
            _ => continue,
        };
        let [name, version, ..] = spec[..] else {
            continue;
        };
        let direct = comment.trim() != "indirect";
        out.dependencies.push(Dependency {
            ecosystem: Ecosystem::Go,
            name: name.trim_matches('"').to_owned(),
            version: Some(version.to_owned()),
            license: None,
            direct,
            scope: Scope::Normal,
            used_by: if direct && !module.is_empty() {
                vec![module.clone()]
            } else {
                Vec::new()
            },
            function: None,
        });
    }
    out.notes
        .push("go.mod records no licences: Go licences unknown".to_owned());
    out
}

/// Maven and Gradle (ADR 0064): the dependencies the root `pom.xml` and its `<modules>`
/// declare, or the root Gradle build and the projects its settings include, with the versions
/// written in them (Maven's `${properties}` read, Gradle's version catalog). A version that a
/// parent or a BOM manages stays unknown; no licences.
pub fn maven(root: &Path) -> Lockfiles {
    let mut out = Lockfiles::default();
    let read = |path: &str| std::fs::read_to_string(root.join(path)).ok();
    if let Some(text) = read("pom.xml") {
        let mut poms = vec![(String::new(), text)];
        let mut at = 0;
        while at < poms.len() && at < 64 {
            let (dir, text) = poms[at].clone();
            at += 1;
            let Ok(document) = roxmltree::Document::parse(&text) else {
                out.notes.push(format!("{dir}/pom.xml did not read"));
                continue;
            };
            let project = document.root_element();
            maven_pom(project, &mut out);
            for m in xml_child(project, "modules")
                .into_iter()
                .flat_map(|m| m.children().filter(|c| c.tag_name().name() == "module"))
                .filter_map(|m| m.text())
            {
                let sub = if dir.is_empty() {
                    m.trim().to_owned()
                } else {
                    format!("{dir}/{}", m.trim())
                };
                if let Some(text) = read(&format!("{sub}/pom.xml")) {
                    poms.push((sub, text));
                }
            }
        }
        out.notes
            .push("pom.xml records no licences: Java licences unknown".to_owned());
        return out;
    }
    let build = |dir: &str| {
        ["build.gradle.kts", "build.gradle"].iter().find_map(|f| {
            let path = if dir.is_empty() {
                (*f).to_owned()
            } else {
                format!("{dir}/{f}")
            };
            read(&path)
        })
    };
    let settings = read("settings.gradle.kts").or_else(|| read("settings.gradle"));
    if settings.is_none() && build("").is_none() {
        return out;
    }
    let catalog = read("gradle/libs.versions.toml").unwrap_or_default();
    let mut projects = vec![String::new()];
    for line in settings.as_deref().unwrap_or_default().lines() {
        if let Some(rest) = line.trim().strip_prefix("include") {
            for (i, part) in rest.split(['"', '\'']).enumerate() {
                if i % 2 == 1 && !part.is_empty() {
                    projects.push(part.trim_start_matches(':').replace(':', "/"));
                }
            }
        }
    }
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for project in &projects {
        let Some(text) = build(project) else {
            continue;
        };
        let name = if project.is_empty() {
            root.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        } else {
            project.rsplit('/').next().unwrap_or(project).to_owned()
        };
        out.packages.push((name.clone(), None, Ecosystem::Maven));
        for (group, artifact, version, runtime) in
            timonier_index::ecosystem::gradle_coordinates(&text, &catalog)
        {
            let id = format!("{group}:{artifact}");
            let scope = if runtime { Scope::Normal } else { Scope::Dev };
            if let Some(&i) = seen.get(&id) {
                let known: &mut Dependency = &mut out.dependencies[i];
                known.used_by.push(name.clone());
                known.scope = known.scope.min(scope);
                continue;
            }
            seen.insert(id.clone(), out.dependencies.len());
            out.dependencies.push(Dependency {
                ecosystem: Ecosystem::Maven,
                name: id,
                version,
                license: None,
                direct: true,
                scope,
                used_by: vec![name.clone()],
                function: None,
            });
        }
    }
    out.notes.push(
        "Gradle builds record no licences, and a version from a plugin or a BOM stays unknown"
            .to_owned(),
    );
    out
}

/// Scala (ADR 0071): the dependencies `build.sbt` declares, as Maven artifacts with their Scala
/// suffix (`upickle_3`) and the versions written there; no licences.
pub fn sbt(root: &Path) -> Lockfiles {
    let mut out = Lockfiles::default();
    let Ok(text) = std::fs::read_to_string(root.join("build.sbt")) else {
        return out;
    };
    let setting = |key: &str| timonier_index::ecosystem::sbt_setting(&text, key);
    let name = match (setting("organization"), setting("name")) {
        (Some(organization), Some(name)) => format!("{organization}:{name}"),
        (_, Some(name)) => name,
        _ => "root".to_owned(),
    };
    out.packages
        .push((name.clone(), setting("version"), Ecosystem::Maven));
    for dependency in timonier_index::ecosystem::sbt_dependencies(&text) {
        out.dependencies.push(Dependency {
            ecosystem: Ecosystem::Maven,
            name: format!("{}:{}", dependency.group, dependency.artifact),
            version: dependency.version,
            license: None,
            direct: true,
            scope: if dependency.test {
                Scope::Dev
            } else {
                Scope::Normal
            },
            used_by: vec![name.clone()],
            function: None,
        });
    }
    out.notes.push(
        "build.sbt records no licences, and the dependencies of plugins and of other projects of \
         the build are not read"
            .to_owned(),
    );
    out
}

/// PHP (ADR 0070): the packages `composer.json` declares, and every package of
/// `composer.lock` with its version and its licences (Composer records them); `require-dev`
/// and `packages-dev` are for the tests.
pub fn composer(root: &Path) -> Lockfiles {
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
        Ecosystem::Composer,
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
                        ecosystem: Ecosystem::Composer,
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
                ecosystem: Ecosystem::Composer,
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

/// .NET (ADR 0069): the `<PackageReference>`s of the `.csproj` files at the root and three
/// folders below, with their versions (`Version`, or central `Directory.Packages.props`); a
/// test project's are for the tests. No licences.
pub fn dotnet(root: &Path) -> Lockfiles {
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
        out.packages.push((name.clone(), None, Ecosystem::Nuget));
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
            .any(|(p, _)| p == timonier_index::ecosystem::DOTNET_TEST_SDK);
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
                ecosystem: Ecosystem::Nuget,
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

/// Swift (ADR 0067): the packages of `Package.swift`, with the versions `Package.resolved` pins,
/// and the ones they pull in; no licences.
pub fn swift(root: &Path) -> Lockfiles {
    let mut out = Lockfiles::default();
    let Ok(manifest) = std::fs::read_to_string(root.join("Package.swift")) else {
        return out;
    };
    let name = manifest
        .find("name:")
        .and_then(|at| manifest[at..].split('"').nth(1))
        .unwrap_or_default()
        .to_owned();
    out.packages.push((name.clone(), None, Ecosystem::Swift));
    let declared: Vec<(String, Option<String>)> =
        timonier_index::ecosystem::swift_packages(&manifest);
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
                ecosystem: Ecosystem::Swift,
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
            ecosystem: Ecosystem::Swift,
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

/// C++ (ADR 0065, Q89): the root `CMakeLists.txt`'s project; the ports of `vcpkg.json` (the
/// version it asks for, if any), Conan's requirements (`conan.lock` pins them), and what CMake
/// finds besides (`find_package`, `FetchContent`, its `GIT_TAG`). No licences.
pub fn cpp(root: &Path) -> Lockfiles {
    use timonier_index::ecosystem::{
        cmake_commands, conan_requires, cpp_code_name, vcpkg_dependencies,
    };
    let mut out = Lockfiles::default();
    let read = |path: &str| std::fs::read_to_string(root.join(path)).ok();
    let Some(lists) = read("CMakeLists.txt") else {
        return out;
    };
    let commands = cmake_commands(&lists);
    let Some(project) = commands
        .iter()
        .find(|(name, _)| name == "project")
        .and_then(|(_, args)| args.first().cloned())
    else {
        return out;
    };
    let version = commands
        .iter()
        .find(|(name, _)| name == "project")
        .and_then(|(_, args)| {
            args.iter()
                .position(|a| a == "VERSION")
                .and_then(|i| args.get(i + 1))
        })
        .cloned();
    out.packages
        .push((project.clone(), version, Ecosystem::Cmake));
    let mut seen = BTreeSet::new();
    let mut add = |out: &mut Lockfiles, ecosystem, name: &str, version: Option<String>| {
        if !seen.insert(cpp_code_name(name)) {
            return;
        }
        let test = matches!(
            cpp_code_name(name).as_str(),
            "gtest" | "catch2" | "doctest" | "benchmark"
        );
        out.dependencies.push(Dependency {
            ecosystem,
            name: name.to_owned(),
            version,
            license: None,
            direct: true,
            scope: if test { Scope::Dev } else { Scope::Normal },
            used_by: vec![project.clone()],
            function: None,
        });
    };
    if let Some(text) = read("vcpkg.json") {
        for (name, version) in vcpkg_dependencies(&text) {
            add(&mut out, Ecosystem::Vcpkg, &name, version);
        }
    }
    let conan = read("conanfile.txt").or_else(|| read("conanfile.py"));
    if let Some(text) = conan {
        // `conan.lock` pins each requirement: `fmt/11.2.0#<revision>%<time>`.
        let pinned: BTreeMap<String, String> = read("conan.lock")
            .and_then(|lock| serde_json::from_str::<Value>(&lock).ok())
            .map(|lock| {
                ["requires", "build_requires", "python_requires"]
                    .iter()
                    .flat_map(|k| lock[k].as_array().cloned().unwrap_or_default())
                    .filter_map(|r| {
                        let r = r.as_str()?.split('#').next()?;
                        let (name, version) = r.split_once('/')?;
                        Some((name.to_owned(), version.to_owned()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        for (name, version) in conan_requires(&text) {
            let version = pinned.get(&name).cloned().or(version);
            add(&mut out, Ecosystem::Conan, &name, version);
        }
    }
    for (command, args) in &commands {
        if !matches!(command.as_str(), "find_package" | "fetchcontent_declare") {
            continue;
        }
        let Some(name) = args.first() else { continue };
        let tag = args
            .iter()
            .position(|a| a == "GIT_TAG")
            .and_then(|i| args.get(i + 1))
            .cloned();
        add(&mut out, Ecosystem::Cmake, name, tag);
    }
    out.notes.push(
        "C++ records no licences; vcpkg and CMake dependencies are not checked against OSV"
            .to_owned(),
    );
    out
}

/// One `pom.xml`: the module, and its dependencies with their versions.
fn maven_pom(project: roxmltree::Node<'_, '_>, out: &mut Lockfiles) {
    let properties: BTreeMap<String, String> = xml_child(project, "properties")
        .into_iter()
        .flat_map(|p| p.children().filter(roxmltree::Node::is_element))
        .filter_map(|p| Some((p.tag_name().name().to_owned(), p.text()?.trim().to_owned())))
        .collect();
    let version = xml_text(project, "version")
        .or_else(|| xml_child(project, "parent").and_then(|p| xml_text(p, "version")));
    let resolve = |value: String| -> Option<String> {
        match value.strip_prefix("${").and_then(|v| v.strip_suffix('}')) {
            Some("project.version") => version.clone(),
            Some(key) => properties.get(key).cloned(),
            None => Some(value),
        }
    };
    let artifact = xml_text(project, "artifactId").unwrap_or_default();
    let group = xml_text(project, "groupId")
        .or_else(|| xml_child(project, "parent").and_then(|p| xml_text(p, "groupId")))
        .unwrap_or_default();
    let module = format!("{group}:{artifact}");
    out.packages
        .push((module.clone(), version.clone(), Ecosystem::Maven));
    for d in xml_child(project, "dependencies")
        .into_iter()
        .flat_map(|d| d.children().filter(|c| c.tag_name().name() == "dependency"))
    {
        let (Some(g), Some(a)) = (xml_text(d, "groupId"), xml_text(d, "artifactId")) else {
            continue;
        };
        let scope = match xml_text(d, "scope").as_deref() {
            Some("test") => Scope::Dev,
            _ => Scope::Normal,
        };
        out.dependencies.push(Dependency {
            ecosystem: Ecosystem::Maven,
            name: format!("{g}:{a}"),
            version: xml_text(d, "version").and_then(resolve),
            license: None,
            direct: true,
            scope,
            used_by: vec![module.clone()],
            function: None,
        });
    }
}

/// The name of a PEP 508 requirement, normalised.
fn requirement_name(requirement: &str) -> Option<String> {
    let name: String = requirement
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect();
    (!name.is_empty()).then(|| normalise_python(&name))
}

/// PEP 503: lower case, every run of `-`, `_` and `.` one `-`.
fn normalise_python(name: &str) -> String {
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
            let ecosystems: &[Ecosystem] = match pack.manifest.ecosystem {
                timonier_index::pack::Ecosystem::Cargo => &[Ecosystem::Cargo],
                timonier_index::pack::Ecosystem::Npm => &[Ecosystem::Npm],
                timonier_index::pack::Ecosystem::Pypi => &[Ecosystem::Pypi],
                timonier_index::pack::Ecosystem::Gomod => &[Ecosystem::Go],
                timonier_index::pack::Ecosystem::Maven => &[Ecosystem::Maven],
                timonier_index::pack::Ecosystem::Swiftpm => &[Ecosystem::Swift],
                timonier_index::pack::Ecosystem::Nuget => &[Ecosystem::Nuget],
                timonier_index::pack::Ecosystem::Composer => &[Ecosystem::Composer],
                // sbt's dependencies are Maven's artifacts (ADR 0071).
                timonier_index::pack::Ecosystem::Sbt => &[Ecosystem::Maven],
                timonier_index::pack::Ecosystem::Cmake => {
                    &[Ecosystem::Conan, Ecosystem::Vcpkg, Ecosystem::Cmake]
                }
            };
            // C and C++ share their ecosystems: their rules add up.
            for ecosystem in ecosystems {
                let rules: &mut InventoryRules = out.entry(*ecosystem).or_default();
                let more = &pack.manifest.inventory;
                rules.frameworks.extend(more.frameworks.iter().cloned());
                for (function, packages) in &more.functions {
                    rules
                        .functions
                        .entry(function.clone())
                        .or_default()
                        .extend(packages.iter().cloned());
                }
            }
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

    /// T-122: `go.mod` gives the module and each requirement with its version; an indirect one
    /// is not direct.
    #[test]
    fn go_mod_gives_the_requirements_and_their_versions() {
        let read = go_mod(
            "module example.com/shop\n\ngo 1.27\n\nrequire github.com/gin-gonic/gin v1.11.0\n\nrequire (\n\tgolang.org/x/text v0.30.0 // indirect\n\t\"gotest.tools/v3\" v3.5.2\n)\n",
        );
        assert_eq!(
            read.packages,
            [("example.com/shop".to_owned(), None, Ecosystem::Go)]
        );
        let all: Vec<(&str, Option<&str>, bool, Vec<String>)> = read
            .dependencies
            .iter()
            .map(|d| {
                (
                    d.name.as_str(),
                    d.version.as_deref(),
                    d.direct,
                    d.used_by.clone(),
                )
            })
            .collect();
        let shop = vec!["example.com/shop".to_owned()];
        assert_eq!(
            all,
            [
                (
                    "github.com/gin-gonic/gin",
                    Some("v1.11.0"),
                    true,
                    shop.clone()
                ),
                ("golang.org/x/text", Some("v0.30.0"), false, Vec::new()),
                ("gotest.tools/v3", Some("v3.5.2"), true, shop)
            ]
        );
    }
}
