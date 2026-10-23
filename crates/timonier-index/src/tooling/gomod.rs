//! Go modules: `go.mod` and `go.sum` (ADR 0063).

use std::fmt::Write;

use super::text::*;
use super::{Found, SdkPlace, WithSdk};
use std::path::Path;

use timonier_core::inventory::{Dependency, Scope};

use super::GO;
use super::{Lockfiles, Packages, Run, Tooling};

pub(crate) struct Gomod;

impl Tooling for Gomod {
    fn name(&self) -> &'static str {
        "gomod"
    }

    fn packages(&self) -> &'static [Packages] {
        &super::PACKAGES[3..4]
    }

    fn dependencies(&self, root: &Path, _run: &Run<'_>) -> Lockfiles {
        match std::fs::read_to_string(root.join("go.mod")) {
            Ok(text) => go_mod(&text),
            Err(_) => Lockfiles::default(),
        }
    }

    /// A module at the root (ADR 0063).
    fn detect(&self, root: &Path) -> Option<Found> {
        self.runs(root).then_some(Found {
            language: "go",
            manager: "go",
            test_script: None,
        })
    }

    fn runs(&self, root: &Path) -> bool {
        root.join("go.mod").is_file()
    }

    fn sdk_place(&self, _root: &Path) -> SdkPlace {
        SdkPlace {
            manifest: "go.mod".to_owned(),
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
        go_mod_with_sdk(text, sdk).map(WithSdk::Text)
    }

    fn without_sdk(
        &self,
        path: &str,
        text: &str,
        _original: &str,
    ) -> Option<Result<Option<String>, String>> {
        (path == "go.mod").then(|| Ok(Some(go_mod_without_sdk(text))))
    }
}

/// `go.mod` (ADR 0063): the module, and every requirement with its version, which Go's
/// minimal version selection builds; `// indirect` ones are not direct. Go records no
/// licences.
pub(crate) fn go_mod(text: &str) -> Lockfiles {
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
                out.packages.push((module.clone(), None, GO));
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
            ecosystem: GO,
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

// What `timonier init` writes in the manifests, and `uninit` takes back (ADR 0050).

/// The markers of the block `init` adds to `go.mod` (ADR 0063).
pub(crate) const GO_BEGIN: &str = "// >>> timonier >>>";
pub(crate) const GO_END: &str = "// <<< timonier <<<";

/// `go.mod` with the SDK (ADR 0063): a `require` block between markers, at the end, with the
/// modules the file does not require yet. `go mod tidy` then fills `go.sum`. `Err` without a
/// `module` line.
pub(crate) fn go_mod_with_sdk(text: &str, sdk: &[Sdk<'_>]) -> Result<String, String> {
    if !text
        .lines()
        .any(|l| l.split_whitespace().next() == Some("module"))
    {
        return Err("it has no `module` line".to_owned());
    }
    // Every module required already, in the block or not: the file as it is, since
    // `go mod tidy` sorts the block and adds its own lines to it.
    let required = go_requires(text);
    if sdk.iter().all(|p| required.iter().any(|r| r == p.name)) {
        return Ok(text.to_owned());
    }
    let bare = go_mod_without_sdk(text);
    let required = go_requires(&bare);
    let missing: Vec<&Sdk<'_>> = sdk
        .iter()
        .filter(|p| !required.iter().any(|r| r == p.name))
        .collect();
    if missing.is_empty() {
        return Ok(bare);
    }
    let mut body = String::from("require (\n");
    for p in missing {
        let _ = writeln!(body, "\t{} {}", p.name, p.version);
    }
    body.push(')');
    Ok(with_block(Some(&bare), GO_BEGIN, GO_END, &body))
}

/// The modules a `go.mod` requires: the first word of a `require` line or of a line of its block.
pub(crate) fn go_requires(text: &str) -> Vec<String> {
    let mut required = Vec::new();
    let mut in_block = false;
    for line in text.lines() {
        let code = line.split("//").next().unwrap_or("");
        let words: Vec<&str> = code.split_whitespace().collect();
        match words[..] {
            ["require", "("] => in_block = true,
            [")"] => in_block = false,
            ["require", name, ..] => required.push(name.trim_matches('"').to_owned()),
            [name, _, ..] if in_block => required.push(name.trim_matches('"').to_owned()),
            _ => {}
        }
    }
    required
}

/// `go.mod` without the block [`go_mod_with_sdk`] added; what `go mod tidy` put in it goes too,
/// and comes back at the next `go mod tidy`.
pub(crate) fn go_mod_without_sdk(text: &str) -> String {
    without_block(text, GO_BEGIN, GO_END).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-122: `go.mod` gives the module and each requirement with its version; an indirect one
    /// is not direct.
    #[test]
    fn go_mod_gives_the_requirements_and_their_versions() {
        let read = go_mod(
            "module example.com/shop\n\ngo 1.27\n\nrequire github.com/gin-gonic/gin v1.11.0\n\nrequire (\n\tgolang.org/x/text v0.30.0 // indirect\n\t\"gotest.tools/v3\" v3.5.2\n)\n",
        );
        assert_eq!(read.packages, [("example.com/shop".to_owned(), None, GO)]);
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

    /// T-122: the SDK goes into `go.mod` as a marked `require` block and comes out, the file
    /// as it was; a second `init` changes nothing; a module required already is left.
    #[test]
    fn the_go_sdk_goes_in_and_out_of_go_mod() {
        let sdk = [
            Sdk {
                name: "go.opentelemetry.io/otel",
                version: "v1.47.0",
                features: &[],
                default_features: true,
                production: true,
            },
            Sdk {
                name: "go.opentelemetry.io/otel/sdk",
                version: "v1.47.0",
                features: &[],
                default_features: true,
                production: false,
            },
        ];
        let bare = "module example.com/shop\n\ngo 1.27\n";
        let requiring = "module example.com/shop\n\ngo 1.27\n\nrequire (\n\tgithub.com/gin-gonic/gin v1.11.0\n\tgo.opentelemetry.io/otel v1.40.0 // indirect\n)\n";
        for original in [bare, requiring] {
            let with = go_mod_with_sdk(original, &sdk).unwrap();
            assert!(
                with.contains("// >>> timonier >>>\nrequire (\n\t"),
                "{with}"
            );
            assert!(
                with.contains("\tgo.opentelemetry.io/otel/sdk v1.47.0\n"),
                "{with}"
            );
            assert_eq!(
                with.contains("\tgo.opentelemetry.io/otel v1.47.0"),
                original == bare,
                "{with}"
            );
            assert_eq!(go_mod_with_sdk(&with, &sdk).unwrap(), with, "twice");
            assert_eq!(go_mod_without_sdk(&with), original, "{with}");
        }
        // Tidied: the block sorted, with lines of its own. Nothing to repair.
        let tidied = "module example.com/shop\n\n// >>> timonier >>>\nrequire (\n\tgo.opentelemetry.io/otel v1.47.0\n\tgo.opentelemetry.io/otel/sdk v1.47.0\n)\n\nrequire github.com/go-logr/logr v1.4.4 // indirect\n\n// <<< timonier <<<\n";
        assert_eq!(go_mod_with_sdk(tidied, &sdk).unwrap(), tidied);
        assert!(
            go_mod_with_sdk("go 1.27\n", &sdk)
                .unwrap_err()
                .contains("module")
        );
    }
}
