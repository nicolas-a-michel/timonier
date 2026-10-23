//! Maven and Gradle (ADR 0064).

use std::fmt::Write;

use super::text::*;
use super::{Found, SdkPlace, WithSdk};
use std::collections::BTreeMap;
use std::path::Path;

use timonier_core::inventory::{Dependency, Scope};

use super::MAVEN;
use super::{Lockfiles, Packages, Run, Tooling};
use crate::ecosystem::{xml_child, xml_text};

pub(crate) struct Maven;

impl Tooling for Maven {
    fn name(&self) -> &'static str {
        "maven"
    }

    fn packages(&self) -> &'static [Packages] {
        &super::PACKAGES[4..5]
    }

    fn dependencies(&self, root: &Path, _run: &Run<'_>) -> Lockfiles {
        maven(root)
    }

    /// A Maven or a Gradle build at the root (ADR 0064): Kotlin's when the build applies the
    /// Kotlin plugin, at the root, in a project it includes, or in its version catalog; a build
    /// of both is Kotlin's (ADR 0066).
    fn detect(&self, root: &Path) -> Option<Found> {
        let gradle = GRADLE_FILES.iter().any(|f| root.join(f).is_file());
        if gradle && gradle_texts(root).iter().any(|t| t.contains("kotlin")) {
            return Some(Found {
                language: "kotlin",
                manager: "gradle",
                test_script: None,
            });
        }
        (root.join("pom.xml").is_file() || gradle).then_some(Found {
            language: "java",
            manager: if gradle { "gradle" } else { "maven" },
            test_script: None,
        })
    }

    fn runs(&self, root: &Path) -> bool {
        root.join("pom.xml").is_file() || GRADLE_FILES.iter().any(|f| root.join(f).is_file())
    }

    /// The root build; else the build of a lone subproject (`gradle init`'s `app/`), its files
    /// under it (ADR 0066); else none.
    fn sdk_place(&self, root: &Path) -> SdkPlace {
        if let Some(m) = ["pom.xml", "build.gradle.kts", "build.gradle"]
            .into_iter()
            .find(|m| root.join(m).is_file())
        {
            return SdkPlace {
                manifest: m.to_owned(),
                under: String::new(),
            };
        }
        match lone_gradle_project(root) {
            Some((dir, build)) => SdkPlace {
                manifest: format!("{dir}/{build}"),
                under: format!("{dir}/"),
            },
            None => SdkPlace {
                manifest: "build.gradle.kts".to_owned(),
                under: String::new(),
            },
        }
    }

    fn with_sdk(
        &self,
        manifest: &str,
        text: &str,
        sdk: &[Sdk<'_>],
        _feature: Option<&str>,
    ) -> Result<WithSdk, String> {
        if manifest == "pom.xml" {
            pom_with_sdk(text, sdk).map(WithSdk::Text)
        } else {
            Ok(WithSdk::Text(gradle_with_sdk(text, sdk)))
        }
    }

    fn without_manifest(&self) -> Option<&'static str> {
        Some(
            "a Gradle build of subprojects alone; the project that runs the tests takes the SDK \
             and the files as the conventions say",
        )
    }

    /// A lone subproject's build too (ADR 0066).
    fn without_sdk(
        &self,
        path: &str,
        text: &str,
        _original: &str,
    ) -> Option<Result<Option<String>, String>> {
        if path == "pom.xml" {
            return Some(Ok(Some(pom_without_sdk(text))));
        }
        let gradle = ["build.gradle.kts", "build.gradle"]
            .iter()
            .any(|b| path == *b || path.ends_with(&format!("/{b}")));
        gradle.then(|| Ok(Some(gradle_without_sdk(text))))
    }
}

/// The files of a Gradle build at the root.
const GRADLE_FILES: [&str; 4] = [
    "build.gradle.kts",
    "build.gradle",
    "settings.gradle.kts",
    "settings.gradle",
];

/// The Gradle files that say which plugins a build applies: settings, root build, version
/// catalog, and the builds of the projects the settings include.
fn gradle_texts(root: &Path) -> Vec<String> {
    let read = |p: &str| std::fs::read_to_string(root.join(p)).ok();
    let mut texts: Vec<String> = [
        "settings.gradle.kts",
        "settings.gradle",
        "build.gradle.kts",
        "build.gradle",
        "gradle/libs.versions.toml",
    ]
    .iter()
    .filter_map(|p| read(p))
    .collect();
    let included: Vec<String> = texts
        .iter()
        .take(2)
        .flat_map(|s| s.lines())
        .filter_map(|l| l.trim().strip_prefix("include").map(str::to_owned))
        .flat_map(|rest| {
            rest.split(['"', '\''])
                .enumerate()
                .filter(|(i, p)| i % 2 == 1 && !p.is_empty())
                .map(|(_, p)| p.trim_start_matches(':').replace(':', "/"))
                .collect::<Vec<_>>()
        })
        .collect();
    for project in included {
        for build in ["build.gradle.kts", "build.gradle"] {
            if let Some(text) = read(&format!("{project}/{build}")) {
                texts.push(text);
            }
        }
    }
    texts
}

/// The one project a Gradle build without a root build includes, and its build file:
/// `include("app")` and `app/build.gradle.kts` → `("app", "build.gradle.kts")`.
fn lone_gradle_project(root: &Path) -> Option<(String, &'static str)> {
    let settings = super::read(root, "settings.gradle.kts")
        .or_else(|| super::read(root, "settings.gradle"))?;
    let mut projects = Vec::new();
    for line in settings.lines() {
        if let Some(rest) = line.trim().strip_prefix("include") {
            for (i, part) in rest.split(['"', '\'']).enumerate() {
                if i % 2 == 1 && !part.is_empty() {
                    projects.push(part.trim_start_matches(':').replace(':', "/"));
                }
            }
        }
    }
    let [dir] = projects.as_slice() else {
        return None;
    };
    let build = ["build.gradle.kts", "build.gradle"]
        .into_iter()
        .find(|b| root.join(dir).join(b).is_file())?;
    Some((dir.clone(), build))
}

/// Maven and Gradle (ADR 0064): the dependencies the root `pom.xml` and its `<modules>`
/// declare, or the root Gradle build and the projects its settings include, with the versions
/// written in them (Maven's `${properties}` read, Gradle's version catalog). A version that a
/// parent or a BOM manages stays unknown; no licences.
pub(crate) fn maven(root: &Path) -> Lockfiles {
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
        out.packages.push((name.clone(), None, MAVEN));
        for (group, artifact, version, runtime) in
            crate::ecosystem::gradle_coordinates(&text, &catalog)
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
                ecosystem: MAVEN,
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
    out.packages.push((module.clone(), version.clone(), MAVEN));
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
            ecosystem: MAVEN,
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

// What `timonier init` writes in the manifests, and `uninit` takes back (ADR 0050).

/// The markers of the block `init` adds inside `pom.xml` (ADR 0064).
pub(crate) const XML_BEGIN: &str = "<!-- >>> timonier >>> -->";
pub(crate) const XML_END: &str = "<!-- <<< timonier <<< -->";

/// `pom.xml` with the SDK (ADR 0064): the artifacts it does not declare yet, between markers,
/// at the top of the project's `<dependencies>` (a `<dependencies>` of their own before
/// `</project>` when it has none); the API at compile scope, the others at test scope.
pub(crate) fn pom_with_sdk(text: &str, sdk: &[Sdk<'_>]) -> Result<String, String> {
    let bare = pom_without_sdk(text);
    let document =
        roxmltree::Document::parse(&bare).map_err(|e| format!("it does not read as XML ({e})"))?;
    let project = document.root_element();
    let dependencies = project
        .children()
        .find(|c| c.is_element() && c.tag_name().name() == "dependencies");
    let text_of = |node: roxmltree::Node<'_, '_>, name: &str| {
        node.children()
            .find(|c| c.is_element() && c.tag_name().name() == name)
            .and_then(|c| c.text())
            .map(str::trim)
            .unwrap_or_default()
            .to_owned()
    };
    let declared: Vec<String> = dependencies
        .into_iter()
        .flat_map(|d| d.children().filter(|c| c.tag_name().name() == "dependency"))
        .map(|d| format!("{}:{}", text_of(d, "groupId"), text_of(d, "artifactId")))
        .collect();
    let missing: Vec<&Sdk<'_>> = sdk
        .iter()
        .filter(|p| !declared.iter().any(|d| d == p.name))
        .collect();
    if missing.is_empty() {
        return Ok(bare);
    }
    let mut lines: Vec<String> = bare.lines().map(str::to_owned).collect();
    let indent_of = |line: &str| line[..line.len() - line.trim_start().len()].to_owned();
    let entries = |inner: &str, unit: &str| -> Vec<String> {
        let mut out = Vec::new();
        for p in &missing {
            let (group, artifact) = p.name.split_once(':').unwrap_or((p.name, p.name));
            out.push(format!("{inner}<dependency>"));
            out.push(format!("{inner}{unit}<groupId>{group}</groupId>"));
            out.push(format!("{inner}{unit}<artifactId>{artifact}</artifactId>"));
            out.push(format!("{inner}{unit}<version>{}</version>", p.version));
            if !p.production {
                out.push(format!("{inner}{unit}<scope>test</scope>"));
            }
            out.push(format!("{inner}</dependency>"));
        }
        out
    };
    let line_of = |offset: usize| bare[..offset].matches('\n').count();
    match dependencies {
        Some(node) => {
            let at = line_of(node.range().start);
            if lines[at].trim() != "<dependencies>" {
                return Err("its <dependencies> is not on a line of its own".to_owned());
            }
            let outer = indent_of(&lines[at]);
            let unit = if outer.is_empty() {
                "  ".to_owned()
            } else {
                outer.clone()
            };
            let inner = format!("{outer}{unit}");
            let mut block = vec![format!("{inner}{XML_BEGIN}")];
            block.extend(entries(&inner, &unit));
            block.push(format!("{inner}{XML_END}"));
            lines.splice(at + 1..at + 1, block);
        }
        None => {
            let at = lines
                .iter()
                .rposition(|l| l.trim() == "</project>")
                .ok_or("its </project> is not on a line of its own")?;
            let unit = lines
                .iter()
                .find(|l| l.starts_with([' ', '\t']))
                .map_or("  ".to_owned(), |l| indent_of(l));
            let inner = format!("{unit}{unit}");
            let mut block = vec![
                format!("{unit}{XML_BEGIN}"),
                format!("{unit}<dependencies>"),
            ];
            block.extend(entries(&inner, &unit));
            block.push(format!("{unit}</dependencies>"));
            block.push(format!("{unit}{XML_END}"));
            lines.splice(at..at, block);
        }
    }
    let mut out = lines.join("\n");
    if bare.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

/// `pom.xml` without the block [`pom_with_sdk`] added: the lines from its first marker to its
/// last.
pub(crate) fn pom_without_sdk(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let begin = lines.iter().position(|l| l.trim() == XML_BEGIN);
    let end = lines.iter().position(|l| l.trim() == XML_END);
    let (Some(begin), Some(end)) = (begin, end) else {
        return text.to_owned();
    };
    if end < begin {
        return text.to_owned();
    }
    let mut kept: Vec<&str> = lines[..begin].to_vec();
    kept.extend(&lines[end + 1..]);
    let mut out = kept.join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// The markers of the block `init` adds to a Gradle build (ADR 0064).
pub(crate) const GRADLE_BEGIN: &str = "// >>> timonier >>>";
pub(crate) const GRADLE_END: &str = "// <<< timonier <<<";

/// A Gradle build (`build.gradle.kts` or `build.gradle`) with the SDK (ADR 0064): a
/// `dependencies` block of its own at the end, between markers, which Gradle adds to the
/// others; the API in `implementation`, the others in `testImplementation`. A module the build
/// names already is left.
pub(crate) fn gradle_with_sdk(text: &str, sdk: &[Sdk<'_>]) -> String {
    let bare = without_block(text, GRADLE_BEGIN, GRADLE_END).unwrap_or_default();
    let missing: Vec<&Sdk<'_>> = sdk
        .iter()
        .filter(|p| {
            !(bare.contains(&format!("\"{}:", p.name)) || bare.contains(&format!("'{}:", p.name)))
        })
        .collect();
    if missing.is_empty() {
        return bare;
    }
    let mut body = String::from("dependencies {\n");
    for p in missing {
        let configuration = if p.production {
            "implementation"
        } else {
            "testImplementation"
        };
        let _ = writeln!(body, "    {configuration}(\"{}:{}\")", p.name, p.version);
    }
    body.push('}');
    with_block(Some(&bare), GRADLE_BEGIN, GRADLE_END, &body)
}

/// A Gradle build without the block [`gradle_with_sdk`] added.
pub(crate) fn gradle_without_sdk(text: &str) -> String {
    without_block(text, GRADLE_BEGIN, GRADLE_END).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-123: the SDK goes into `pom.xml`, inside its `<dependencies>` or in one of its own,
    /// and comes out, the file as it was; a second `init` changes nothing; an artifact declared
    /// already is left.
    #[test]
    fn the_java_sdk_goes_in_and_out_of_pom_xml() {
        let sdk = crate::tooling::fixtures::java_sdk();
        let with_dependencies = "<project>\n    <artifactId>shop</artifactId>\n    <dependencies>\n        <dependency>\n            <groupId>io.opentelemetry</groupId>\n            <artifactId>opentelemetry-api</artifactId>\n        </dependency>\n    </dependencies>\n</project>\n";
        let without = "<project>\n  <artifactId>shop</artifactId>\n</project>\n";
        for original in [with_dependencies, without] {
            let with = pom_with_sdk(original, &sdk).unwrap();
            roxmltree::Document::parse(&with).unwrap_or_else(|e| panic!("{e}\n{with}"));
            assert!(
                with.contains("<artifactId>opentelemetry-sdk</artifactId>"),
                "{with}"
            );
            assert!(with.contains("<scope>test</scope>"), "{with}");
            assert_eq!(
                with.matches("<artifactId>opentelemetry-api</artifactId>")
                    .count(),
                1,
                "{with}"
            );
            assert_eq!(pom_with_sdk(&with, &sdk).unwrap(), with, "twice");
            assert_eq!(pom_without_sdk(&with), original, "{with}");
        }
        assert!(
            with_dependencies.contains("    <dependencies>\n")
                && pom_with_sdk(with_dependencies, &sdk)
                    .unwrap()
                    .contains("    <dependencies>\n        <!-- >>> timonier >>> -->\n        <dependency>\n            <groupId>io.opentelemetry</groupId>")
        );
    }

    /// T-123: the SDK goes into a Gradle build as a `dependencies` block of its own and comes
    /// out; a module named already is left.
    #[test]
    fn the_java_sdk_goes_in_and_out_of_a_gradle_build() {
        let sdk = crate::tooling::fixtures::java_sdk();
        let original = "plugins {\n    java\n}\n\ndependencies {\n    implementation(\"io.opentelemetry:opentelemetry-api:1.60.0\")\n}\n";
        let with = gradle_with_sdk(original, &sdk);
        assert!(
            with.ends_with("// >>> timonier >>>\ndependencies {\n    testImplementation(\"io.opentelemetry:opentelemetry-sdk:1.67.0\")\n}\n// <<< timonier <<<\n"),
            "{with}"
        );
        assert_eq!(gradle_with_sdk(&with, &sdk), with, "twice");
        assert_eq!(gradle_without_sdk(&with), original);
    }

    /// T-127: `gradle init`'s layout, a lone `app/`, takes the SDK; two projects do not.
    #[test]
    fn a_lone_gradle_subproject_is_found() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("settings.gradle.kts"),
            "rootProject.name = \"shop\"\ninclude(\"app\")\n",
        )
        .unwrap();
        assert_eq!(lone_gradle_project(root), None, "no build in app/");
        std::fs::create_dir(root.join("app")).unwrap();
        std::fs::write(
            root.join("app/build.gradle.kts"),
            "plugins { application }\n",
        )
        .unwrap();
        assert_eq!(
            lone_gradle_project(root),
            Some(("app".to_owned(), "build.gradle.kts"))
        );
        std::fs::write(
            root.join("settings.gradle.kts"),
            "include(\"app\", \"lib\")\n",
        )
        .unwrap();
        assert_eq!(lone_gradle_project(root), None, "two projects");
    }
}
