//! Package ecosystems (ADR 0012, 0013): packages found in manifests, and the module path of a
//! file. Cargo and npm are compiled in; a pack names its ecosystem.

use std::path::Path;

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use timonier_core::NodeId;

use crate::pack::Ecosystem;
use crate::{Error, Result};

/// The manifest file names of the ecosystems.
pub(crate) const MANIFESTS: [(&str, Ecosystem); 2] = [
    ("Cargo.toml", Ecosystem::Cargo),
    ("package.json", Ecosystem::Npm),
];

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
    /// Cargo: `[lib] path`, relative to the package directory, when it is declared.
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
    let mut problems = Vec::new();
    let mut candidates = Vec::new();
    for manifest in manifests {
        let Some((file, ecosystem)) = MANIFESTS
            .iter()
            .find(|(file, _)| manifest == file || manifest.ends_with(&format!("/{file}")))
        else {
            continue;
        };
        let dir = manifest[..manifest.len() - file.len()]
            .trim_end_matches('/')
            .to_owned();
        let path = root.join(manifest);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(source) => {
                problems.push(Error::Io { path, source });
                continue;
            }
        };
        let candidate = match ecosystem {
            Ecosystem::Cargo => cargo(&text, &dir),
            Ecosystem::Npm => npm(&text, &dir),
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
        if package.ecosystem == Ecosystem::Cargo {
            let lib = package.lib_path.as_deref().unwrap_or("src/lib.rs");
            package.has_lib = root.join(&package.dir).join(lib).is_file();
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

/// `src/App.tsx` → `src/App`; `types/vscode.d.ts` → `types/vscode`.
pub(crate) fn without_extension(path: &str) -> &str {
    for extension in [
        ".d.ts", ".d.mts", ".d.cts", ".tsx", ".ts", ".mts", ".cts", ".rs",
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
