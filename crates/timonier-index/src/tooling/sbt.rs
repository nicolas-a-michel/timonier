//! sbt, whose dependencies are Maven's artifacts (ADR 0071).

use super::maven::*;
use super::text::*;
use super::{Found, SdkPlace, WithSdk};
use std::path::Path;

use timonier_core::inventory::{Dependency, Scope};

use super::MAVEN;
use super::{Lockfiles, Packages, Run, Tooling};

pub(crate) struct Sbt;

impl Tooling for Sbt {
    fn name(&self) -> &'static str {
        "sbt"
    }

    fn packages(&self) -> &'static [Packages] {
        &super::PACKAGES[4..5]
    }

    fn dependencies(&self, root: &Path, _run: &Run<'_>) -> Lockfiles {
        sbt(root)
    }

    /// An sbt build at the root (ADR 0071).
    fn detect(&self, root: &Path) -> Option<Found> {
        self.runs(root).then_some(Found {
            language: "scala",
            manager: "sbt",
            test_script: None,
        })
    }

    fn runs(&self, root: &Path) -> bool {
        root.join("build.sbt").is_file()
    }

    fn sdk_place(&self, _root: &Path) -> SdkPlace {
        SdkPlace {
            manifest: "build.sbt".to_owned(),
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
        Ok(WithSdk::Text(sbt_with_sdk(text, sdk)))
    }

    /// The same `//` markers as Gradle's.
    fn without_sdk(
        &self,
        path: &str,
        text: &str,
        _original: &str,
    ) -> Option<Result<Option<String>, String>> {
        (path == "build.sbt").then(|| Ok(Some(gradle_without_sdk(text))))
    }
}

/// Scala (ADR 0071): the dependencies `build.sbt` declares, as Maven artifacts with their Scala
/// suffix (`upickle_3`) and the versions written there; no licences.
pub(crate) fn sbt(root: &Path) -> Lockfiles {
    let mut out = Lockfiles::default();
    let Ok(text) = std::fs::read_to_string(root.join("build.sbt")) else {
        return out;
    };
    let setting = |key: &str| crate::ecosystem::sbt_setting(&text, key);
    let name = match (setting("organization"), setting("name")) {
        (Some(organization), Some(name)) => format!("{organization}:{name}"),
        (_, Some(name)) => name,
        _ => "root".to_owned(),
    };
    out.packages.push((name.clone(), setting("version"), MAVEN));
    for dependency in crate::ecosystem::sbt_dependencies(&text) {
        out.dependencies.push(Dependency {
            ecosystem: MAVEN,
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

// What `timonier init` writes in the manifests, and `uninit` takes back (ADR 0050).

/// `build.sbt` with the SDK (ADR 0071): a `libraryDependencies ++= Seq(…)` of its own at the
/// end, between the markers of Gradle's (same comments), the API for the code, the others
/// `% Test`; Maven coordinates, `group:artifact`, written `"group" % "artifact" % "version"`.
/// Taken back by [`gradle_without_sdk`].
pub(crate) fn sbt_with_sdk(text: &str, sdk: &[Sdk<'_>]) -> String {
    let bare = without_block(text, GRADLE_BEGIN, GRADLE_END).unwrap_or_default();
    let missing: Vec<String> = sdk
        .iter()
        .filter_map(|p| {
            let (group, artifact) = p.name.split_once(':')?;
            if bare.contains(&format!("\"{artifact}\"")) {
                return None;
            }
            let test = if p.production { "" } else { " % Test" };
            Some(format!(
                "  \"{group}\" % \"{artifact}\" % \"{}\"{test},",
                p.version
            ))
        })
        .collect();
    if missing.is_empty() {
        return bare;
    }
    let body = format!("libraryDependencies ++= Seq(\n{}\n)", missing.join("\n"));
    with_block(Some(&bare), GRADLE_BEGIN, GRADLE_END, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-132: `build.sbt` takes a block of its own, the API for the code and the SDK `% Test`,
    /// and gives it back.
    #[test]
    fn the_scala_sdk_goes_in_and_out_of_build_sbt() {
        let sdk = crate::tooling::fixtures::java_sdk();
        let original = "ThisBuild / scalaVersion := \"3.10.0\"\n\nlazy val root = (project in file(\".\"))\n  .settings(name := \"shop\")\n";
        let with = sbt_with_sdk(original, &sdk);
        assert!(
            with.ends_with("// >>> timonier >>>\nlibraryDependencies ++= Seq(\n  \"io.opentelemetry\" % \"opentelemetry-api\" % \"1.67.0\",\n  \"io.opentelemetry\" % \"opentelemetry-sdk\" % \"1.67.0\" % Test,\n)\n// <<< timonier <<<\n"),
            "{with}"
        );
        assert_eq!(sbt_with_sdk(&with, &sdk), with, "twice");
        assert_eq!(gradle_without_sdk(&with), original);
    }
}
