//! C and C++: Conan, vcpkg and CMake (ADR 0065).

use super::ordered::Json;
use super::text::*;
use super::{Beside, Found, SdkPlace, WithSdk};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::Value;
use timonier_core::inventory::{Dependency, Scope};

use super::{CMAKE, CONAN, VCPKG};
use super::{Lockfiles, Packages, Run, Tooling};

pub(crate) struct Cmake;

impl Tooling for Cmake {
    fn name(&self) -> &'static str {
        "cmake"
    }

    fn packages(&self) -> &'static [Packages] {
        &super::PACKAGES[5..8]
    }

    fn dependencies(&self, root: &Path, _run: &Run<'_>) -> Lockfiles {
        cpp(root)
    }

    /// A CMake project at the root, in C (ADR 0068) or C++ (ADR 0065), with vcpkg or Conan when
    /// they are there. One runner either way: the same CTest.
    fn detect(&self, root: &Path) -> Option<Found> {
        self.runs(root).then(|| {
            let manager = if root.join("vcpkg.json").is_file() {
                "vcpkg"
            } else if ["conanfile.txt", "conanfile.py"]
                .iter()
                .any(|f| root.join(f).is_file())
            {
                "conan"
            } else {
                "cmake"
            };
            Found {
                language: if cmake_is_c(root) { "c" } else { "cpp" },
                manager,
                test_script: None,
            }
        })
    }

    fn runs(&self, root: &Path) -> bool {
        root.join("CMakeLists.txt").is_file()
    }

    fn sdk_place(&self, _root: &Path) -> SdkPlace {
        SdkPlace {
            manifest: "CMakeLists.txt".to_owned(),
            under: String::new(),
        }
    }

    fn with_sdk(
        &self,
        _manifest: &str,
        text: &str,
        _sdk: &[Sdk<'_>],
        _feature: Option<&str>,
    ) -> Result<WithSdk, String> {
        Ok(WithSdk::Text(cmake_with_sdk(text)))
    }

    /// The SDK goes into the manifest of vcpkg or of Conan too (ADR 0065), each with the
    /// version its registry has.
    fn sdk_beside(&self, root: &Path, sdk: &[(Option<&str>, Sdk<'_>)]) -> Vec<Beside> {
        let for_manager = |manager: &str| -> Vec<Sdk<'_>> {
            sdk.iter()
                .filter(|(m, _)| m.is_none_or(|m| m == manager))
                .map(|(_, s)| *s)
                .collect()
        };
        let mut out = Vec::new();
        if let Some(vcpkg) = super::read(root, "vcpkg.json") {
            out.push(Beside {
                path: "vcpkg.json".to_owned(),
                text: vcpkg_with_sdk(&vcpkg, &for_manager("vcpkg")),
            });
        }
        if let Some(conan) = super::read(root, "conanfile.txt") {
            out.push(Beside {
                path: "conanfile.txt".to_owned(),
                text: Ok(conanfile_with_sdk(&conan, &for_manager("conan"))),
            });
        }
        out
    }

    fn without_sdk(
        &self,
        path: &str,
        text: &str,
        original: &str,
    ) -> Option<Result<Option<String>, String>> {
        match path {
            "CMakeLists.txt" => Some(Ok(without_block(text, HASH_BEGIN, HASH_END))),
            "conanfile.txt" => Some(Ok(Some(conanfile_without_sdk(text)))),
            "vcpkg.json" => {
                let names = super::sdk_names(self.name(), Some("vcpkg"));
                let names: Vec<&str> = names.iter().map(String::as_str).collect();
                Some(vcpkg_without_sdk(text, original, &names).map(Some))
            }
            _ => None,
        }
    }
}

/// A CMake project in C: its `project()` names C and not CXX, or, naming neither, it has `.c`
/// files and no C++ ones at the root and one folder below.
fn cmake_is_c(root: &Path) -> bool {
    let lists = std::fs::read_to_string(root.join("CMakeLists.txt")).unwrap_or_default();
    let commands = crate::ecosystem::cmake_commands(&lists);
    if let Some((_, args)) = commands.iter().find(|(name, _)| name == "project") {
        let languages: Vec<&str> = args
            .iter()
            .skip_while(|a| *a != "LANGUAGES")
            .skip(1)
            .map(String::as_str)
            .collect();
        let named = |l: &str| languages.contains(&l) || args.iter().skip(1).any(|a| a == l);
        if named("CXX") {
            return false;
        }
        if named("C") {
            return true;
        }
    }
    let mut c = false;
    let mut dirs = vec![root.to_path_buf()];
    if let Ok(entries) = std::fs::read_dir(root) {
        dirs.extend(
            entries
                .flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                .map(|e| e.path()),
        );
    }
    for dir in dirs {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if [".cpp", ".cc", ".cxx", ".hpp"]
                .iter()
                .any(|e| name.ends_with(e))
            {
                return false;
            }
            c |= name.ends_with(".c");
        }
    }
    c
}

/// C++ (ADR 0065, Q89): the root `CMakeLists.txt`'s project; the ports of `vcpkg.json` (the
/// version it asks for, if any), Conan's requirements (`conan.lock` pins them), and what CMake
/// finds besides (`find_package`, `FetchContent`, its `GIT_TAG`). No licences.
pub(crate) fn cpp(root: &Path) -> Lockfiles {
    use crate::ecosystem::{cmake_commands, conan_requires, cpp_code_name, vcpkg_dependencies};
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
    out.packages.push((project.clone(), version, CMAKE));
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
            add(&mut out, VCPKG, &name, version);
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
            add(&mut out, CONAN, &name, version);
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
        add(&mut out, CMAKE, name, tag);
    }
    out.notes.push(
        "C++ records no licences; vcpkg and CMake dependencies are not checked against OSV"
            .to_owned(),
    );
    out
}

// What `timonier init` writes in the manifests, and `uninit` takes back (ADR 0050).

/// `conanfile.txt` with the SDK (ADR 0065): the references it does not require yet
/// (`opentelemetry-cpp/1.26.0`), between markers at the top of `[requires]` (a `[requires]` of
/// their own at the end when it has none). Taken back by [`conanfile_without_sdk`].
pub(crate) fn conanfile_with_sdk(text: &str, sdk: &[Sdk<'_>]) -> String {
    let bare = conanfile_without_sdk(text);
    let missing: Vec<String> = sdk
        .iter()
        .filter(|p| {
            !bare
                .lines()
                .any(|l| l.trim().starts_with(&format!("{}/", p.name)))
        })
        .map(|p| format!("{}/{}", p.name, p.version))
        .collect();
    if missing.is_empty() {
        return bare;
    }
    let mut lines: Vec<String> = bare.lines().map(str::to_owned).collect();
    match lines.iter().position(|l| l.trim() == "[requires]") {
        Some(at) => {
            let mut block = vec![HASH_BEGIN.to_owned()];
            block.extend(missing);
            block.push(HASH_END.to_owned());
            lines.splice(at + 1..at + 1, block);
        }
        None => {
            if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push(HASH_BEGIN.to_owned());
            lines.push("[requires]".to_owned());
            lines.extend(missing);
            lines.push(HASH_END.to_owned());
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// `conanfile.txt` without the block [`conanfile_with_sdk`] added.
pub(crate) fn conanfile_without_sdk(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let begin = lines.iter().position(|l| l.trim() == HASH_BEGIN);
    let end = lines.iter().position(|l| l.trim() == HASH_END);
    let (Some(begin), Some(end)) = (begin, end) else {
        return text.to_owned();
    };
    let mut kept: Vec<&str> = lines[..begin].to_vec();
    kept.extend(&lines[end + 1..]);
    // The `[requires]` block came after a blank line it adds.
    if begin == kept.len() && kept.last().is_some_and(|l| l.trim().is_empty()) {
        kept.pop();
    }
    let mut out = kept.join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// The block `init` adds to the root `CMakeLists.txt` (ADR 0065): the folder `timonier/`, and
/// CTest, which its smoke test needs.
pub(crate) fn cmake_with_sdk(text: &str) -> String {
    with_block(
        Some(text),
        HASH_BEGIN,
        HASH_END,
        "# Timonier's OpenTelemetry and its smoke test (timonier init), when opentelemetry-cpp is found.\nenable_testing()\nadd_subdirectory(timonier)",
    )
}

/// `vcpkg.json` with the SDK (ADR 0065): each port it does not depend on yet, with its
/// features (`opentelemetry-cpp[otlp-http]`).
pub(crate) fn vcpkg_with_sdk(text: &str, sdk: &[Sdk<'_>]) -> Result<String, String> {
    let mut manifest = object(Some(text))?;
    let named = |entry: &Json, name: &str| match entry {
        Json::String(s) => s == name,
        _ => matches!(entry.get("name"), Some(Json::String(s)) if s == name),
    };
    let Some(Json::Array(dependencies)) =
        manifest.entry("dependencies", || Json::Array(Vec::new()))
    else {
        return Err("its dependencies are not a list".to_owned());
    };
    let before = dependencies.len();
    for port in sdk {
        if dependencies.iter().any(|d| named(d, port.name)) {
            continue;
        }
        dependencies.push(if port.features.is_empty() {
            Json::String(port.name.to_owned())
        } else {
            Json::Object(vec![
                ("name".to_owned(), Json::String(port.name.to_owned())),
                (
                    "features".to_owned(),
                    Json::Array(
                        port.features
                            .iter()
                            .map(|f| Json::String(f.clone()))
                            .collect(),
                    ),
                ),
            ])
        });
    }
    if dependencies.len() == before {
        return Ok(text.to_owned());
    }
    Ok(manifest.pretty())
}

/// `vcpkg.json` without the SDK's ports that `original`, the file before `init`, did not have.
pub(crate) fn vcpkg_without_sdk(
    text: &str,
    original: &str,
    names: &[&str],
) -> Result<String, String> {
    let named = |entry: &Json, name: &str| match entry {
        Json::String(s) => s == name,
        _ => matches!(entry.get("name"), Some(Json::String(s)) if s == name),
    };
    let had = |name: &str| {
        matches!(object(Some(original)).ok().and_then(|o| o.get("dependencies").cloned()),
            Some(Json::Array(list)) if list.iter().any(|d| named(d, name)))
    };
    let mut manifest = object(Some(text))?;
    let mut emptied = false;
    if let Some(Json::Array(dependencies)) =
        manifest.entry("dependencies", || Json::Array(Vec::new()))
    {
        dependencies.retain(|d| !names.iter().any(|n| named(d, n) && !had(n)));
        emptied = dependencies.is_empty();
    }
    // A list `init` made for its port alone goes with it.
    let made = object(Some(original)).is_ok_and(|o| o.get("dependencies").is_none());
    if emptied && made {
        manifest.remove("dependencies");
    }
    Ok(manifest.pretty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    /// T-124: the root `CMakeLists.txt` takes the folder `timonier/` in a block, and gives it
    /// back; `vcpkg.json` takes opentelemetry-cpp with its feature, once, and gives it back.
    #[test]
    fn the_cpp_sdk_goes_in_and_out_of_cmake_and_vcpkg() {
        let lists = "cmake_minimum_required(VERSION 3.20)\nproject(shop CXX)\nadd_executable(shop main.cpp)\n";
        let with = cmake_with_sdk(lists);
        assert!(
            with.ends_with("enable_testing()\nadd_subdirectory(timonier)\n# <<< timonier <<<\n"),
            "{with}"
        );
        assert_eq!(cmake_with_sdk(&with), with);
        assert_eq!(
            without_block(&with, HASH_BEGIN, HASH_END).as_deref(),
            Some(lists)
        );
        let features = ["otlp-http".to_owned()];
        let sdk = [Sdk {
            name: "opentelemetry-cpp",
            version: "1.29.0",
            features: &features,
            default_features: true,
            production: false,
        }];
        let vcpkg = "{\n  \"name\": \"shop\",\n  \"dependencies\": [\"fmt\"]\n}\n";
        let with = vcpkg_with_sdk(vcpkg, &sdk).unwrap();
        let parsed: Value = serde_json::from_str(&with).unwrap();
        assert_eq!(
            parsed["dependencies"],
            json!(["fmt", {"name": "opentelemetry-cpp", "features": ["otlp-http"]}])
        );
        assert_eq!(vcpkg_with_sdk(&with, &sdk).unwrap(), with, "twice");
        let without = vcpkg_without_sdk(&with, vcpkg, &["opentelemetry-cpp"]).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&without).unwrap(),
            serde_json::from_str::<Value>(vcpkg).unwrap()
        );
        // Declared before `init`: it stays.
        assert_eq!(
            vcpkg_without_sdk(&with, &with, &["opentelemetry-cpp"]).unwrap(),
            Json::parse(&with).unwrap().pretty()
        );
    }

    /// A `vcpkg.json` without dependencies, indented as its author did, gets its list for
    /// opentelemetry-cpp, and loses the list again with the port.
    #[test]
    fn a_dependency_list_vcpkg_had_not_goes_with_the_port() {
        let features = ["otlp-http".to_owned()];
        let sdk = [Sdk {
            name: "opentelemetry-cpp",
            version: "1.29.0",
            features: &features,
            default_features: true,
            production: false,
        }];
        let original = "{\n    \"name\": \"shop\",\n    \"version\": \"1.0.0\"\n}\n";
        let with = vcpkg_with_sdk(original, &sdk).unwrap();
        let back = vcpkg_without_sdk(&with, original, &["opentelemetry-cpp"]).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&back).unwrap(),
            serde_json::from_str::<Value>(original).unwrap()
        );
    }

    /// `conanfile.txt` takes opentelemetry-cpp at the top of `[requires]`, or in a `[requires]`
    /// of its own, and gives it back as it was.
    #[test]
    fn the_cpp_sdk_goes_in_and_out_of_conanfile_txt() {
        let sdk = [Sdk {
            name: "opentelemetry-cpp",
            version: "1.26.0",
            features: &[],
            default_features: true,
            production: false,
        }];
        let with_requires = "[requires]\nfmt/12.2.0\n\n[generators]\nCMakeDeps\nCMakeToolchain\n";
        let without = "[generators]\nCMakeDeps\n";
        for original in [with_requires, without] {
            let with = conanfile_with_sdk(original, &sdk);
            assert!(with.contains("opentelemetry-cpp/1.26.0\n"), "{with}");
            assert_eq!(conanfile_with_sdk(&with, &sdk), with, "twice");
            assert_eq!(conanfile_without_sdk(&with), original, "{with}");
        }
        assert!(conanfile_with_sdk(with_requires, &sdk).starts_with("[requires]\n# >>> timonier >>>\nopentelemetry-cpp/1.26.0\n# <<< timonier <<<\nfmt/12.2.0\n"));
    }
}
