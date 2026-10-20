//! Language packs (ADR 0012): a manifest and tree-sitter queries whose capture names form a
//! contract. The built-in packs are embedded in the binary.

use std::collections::{BTreeMap, HashMap};

use serde::Deserialize;
use timonier_core::{Language, Role, SymbolKind};
use tree_sitter::Query;

use crate::{Error, Result, grammar};

/// The pack format this build reads.
pub const FORMAT: u32 = 1;

/// Package ecosystems with compiled-in discovery and resolution (ADR 0012).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ecosystem {
    Cargo,
    Npm,
}

impl Ecosystem {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cargo => "cargo",
            Self::Npm => "npm",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format: u32,
    pub language: String,
    pub version: String,
    pub ecosystem: Ecosystem,
    pub grammars: Vec<GrammarManifest>,
    pub naming: Naming,
    pub paths: Paths,
    pub fingerprint: FingerprintRules,
    pub signature: SignatureRules,
    /// Usage category → external package names.
    #[serde(default)]
    pub dependencies: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub roles: Vec<RoleRule>,
    #[serde(default)]
    pub lsp: LspRules,
    /// How the tests of this language run and write JUnit XML (ADR 0034).
    #[serde(default)]
    pub tests: TestRules,
    /// Reserved for later phases (ADR 0012): read, not interpreted.
    #[serde(default)]
    pub instrumentation: Option<toml::Table>,
    #[serde(default)]
    pub security: Option<toml::Table>,
}

/// The test runners of a language, tried in order: the first that fits is the project's
/// (ADR 0034).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestRules {
    #[serde(default)]
    pub runners: Vec<RunnerRule>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunnerRule {
    pub name: String,
    /// A word the manifest's `test` script holds (`vitest`); without it, the runner always fits.
    #[serde(default)]
    pub uses: Option<String>,
    /// How to run the tests so that they write JUnit XML.
    #[serde(default)]
    pub command: Option<String>,
    /// Where the JUnit XML lands, relative to the root.
    #[serde(default)]
    pub junit: Option<String>,
    /// A block `init` writes between markers into a file of the project.
    #[serde(default)]
    pub config: Option<ConfigBlock>,
    /// A program the runner needs on the `PATH`.
    #[serde(default)]
    pub program: Option<String>,
    /// What to do by hand: when `program` is missing, or what `init` does not do.
    #[serde(default)]
    pub note: Option<String>,
    /// What is good to know, with nothing to do: `doctor` says it without failing.
    #[serde(default)]
    pub info: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigBlock {
    pub file: String,
    pub text: String,
}

/// What refinement needs to know of the language (ADR 0022).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LspRules {
    /// Names defined outside any repository (the standard library, globals): a path tree-sitter
    /// leaves unresolved that starts with one of them is not asked about.
    #[serde(default)]
    pub outside: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrammarManifest {
    pub name: String,
    #[serde(rename = "crate")]
    pub crate_name: String,
    pub version: String,
    pub license: String,
    pub extensions: Vec<String>,
}

/// Separators of qualified names: between module segments, between the module and the first
/// symbol, between nested symbols.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Naming {
    pub module: String,
    pub join: String,
    pub member: String,
}

/// How paths written in the code are read.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Paths {
    pub separators: Vec<String>,
    /// The field of an import list node that prefixes the paths it contains.
    #[serde(default)]
    pub prefix_field: Option<String>,
    /// A last segment that designates the module itself (`use a::{self}`).
    #[serde(default)]
    pub trailing_self: Option<String>,
    pub bind: Bind,
}

/// The local names an import binds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bind {
    /// The last segment of each imported path, or its alias (`use a::B as C`).
    LastSegment,
    /// The `@import.name` captures (`import { a } from "./x"`).
    Captures,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FingerprintRules {
    /// Node kinds kept as one token, with their text verbatim.
    #[serde(default)]
    pub atoms: Vec<String>,
    /// Tokens dropped everywhere.
    #[serde(default)]
    pub drop: Vec<String>,
    /// Tokens dropped when a closing delimiter follows them (trailing commas).
    #[serde(default)]
    pub drop_before_closing: Vec<String>,
    #[serde(default)]
    pub closing: Vec<String>,
    /// Tokens dropped at the start or after an opening token (`type U = | a | b`).
    #[serde(default)]
    pub drop_leading: Vec<String>,
    #[serde(default)]
    pub opening: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignatureRules {
    #[serde(default)]
    pub no_space_before: Vec<String>,
    #[serde(default)]
    pub no_space_after: Vec<String>,
    /// Tokens without space before them when they follow a name or a closing bracket.
    #[serde(default)]
    pub tight_after_word: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleRule {
    pub role: Role,
    #[serde(default)]
    pub libraries: Vec<String>,
    #[serde(default)]
    pub paths: Vec<String>,
    pub confidence: f32,
}

/// What a definitions query captures (ADR 0012), by capture index.
#[derive(Debug)]
pub(crate) struct Definitions {
    pub query: Query,
    pub kinds: Vec<Option<SymbolKind>>,
    pub name: Option<u32>,
    pub body: Option<u32>,
    pub scope: Option<u32>,
    /// `scope.<x>` captures, by their name `scope.<x>`.
    pub scope_parts: HashMap<u32, String>,
    /// `timonier.segment` and `timonier.parent`, by pattern.
    pub segment: Vec<Option<String>>,
    pub parent: Vec<Option<String>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Capture {
    Import,
    ImportPath,
    ImportAlias,
    ImportName,
    Call,
    Method,
    Type,
    Implements,
    Inherits,
    Source,
}

#[derive(Debug)]
pub(crate) struct References {
    pub query: Query,
    pub captures: Vec<Option<Capture>>,
}

#[derive(Debug)]
pub(crate) struct Roles {
    pub query: Query,
    pub roles: Vec<Option<Role>>,
}

/// A pack's queries, compiled for one of its grammars.
#[derive(Debug)]
pub(crate) struct Grammar {
    pub name: String,
    /// The grammar crate's version, from the manifest: part of the cache key (ADR 0013).
    pub version: String,
    pub language: tree_sitter::Language,
    pub definitions: Definitions,
    pub references: References,
    pub roles: Roles,
}

#[derive(Debug)]
pub struct Pack {
    pub manifest: Manifest,
    pub language: Language,
    pub(crate) grammars: Vec<Grammar>,
    /// `conventions/<name>.md`, by name: copied into `.timonier/conventions/<language>/` by
    /// `timonier init` (ADR 0034).
    pub conventions: BTreeMap<String, String>,
}

impl Pack {
    /// Builds a pack from its files: `pack.toml`, the three queries, and optional
    /// `<query>.<grammar>.scm` additions.
    pub fn from_files(name: &str, files: &BTreeMap<String, String>) -> Result<Self> {
        let pack_error = |reason: String| Error::Pack {
            pack: name.to_owned(),
            reason,
        };
        let manifest_text = files
            .get("pack.toml")
            .ok_or_else(|| pack_error("missing pack.toml".into()))?;
        let manifest: Manifest =
            toml::from_str(manifest_text).map_err(|source| Error::Manifest {
                pack: name.to_owned(),
                source: Box::new(source),
            })?;
        if manifest.format != FORMAT {
            return Err(pack_error(format!(
                "format {} is not supported (this build reads format {FORMAT})",
                manifest.format
            )));
        }
        let language = Language::new(manifest.language.clone())?;
        if !is_version(&manifest.version) {
            return Err(pack_error(format!(
                "version `{}` is not `major.minor.patch`",
                manifest.version
            )));
        }
        if manifest.grammars.is_empty() {
            return Err(pack_error("no grammar".into()));
        }
        if manifest.paths.separators.is_empty()
            || manifest.paths.separators.iter().any(String::is_empty)
        {
            return Err(pack_error(
                "paths.separators needs non-empty separators".into(),
            ));
        }
        for rule in &manifest.roles {
            if !(0.0..=1.0).contains(&rule.confidence) {
                return Err(pack_error(format!(
                    "role `{}`: confidence {} is outside [0, 1]",
                    rule.role, rule.confidence
                )));
            }
        }
        let known: Vec<String> = manifest
            .grammars
            .iter()
            .flat_map(|g| {
                ["definitions", "references", "roles"].map(|q| format!("{q}.{}.scm", g.name))
            })
            .chain(
                [
                    "pack.toml",
                    "definitions.scm",
                    "references.scm",
                    "roles.scm",
                ]
                .map(String::from),
            )
            .collect();
        let convention = |file: &str| file.starts_with("conventions/") && file.ends_with(".md");
        if let Some(stray) = files
            .keys()
            .find(|file| !known.contains(file) && !convention(file))
        {
            return Err(pack_error(format!("unexpected file `{stray}`")));
        }
        let mut grammars = Vec::new();
        for entry in &manifest.grammars {
            let ts = grammar::by_name(&entry.name).ok_or_else(|| {
                pack_error(format!(
                    "grammar `{}` is not compiled into this build",
                    entry.name
                ))
            })?;
            let source = |query: &'static str| -> Result<String> {
                let base = files
                    .get(&format!("{query}.scm"))
                    .ok_or_else(|| pack_error(format!("missing {query}.scm")))?;
                Ok(match files.get(&format!("{query}.{}.scm", entry.name)) {
                    Some(extra) => format!("{base}\n{extra}"),
                    None => base.clone(),
                })
            };
            let compile = |query: &'static str| -> Result<Query> {
                Query::new(&ts, &source(query)?).map_err(|e| Error::Query {
                    pack: name.to_owned(),
                    grammar: entry.name.clone(),
                    query,
                    message: format!("line {}, column {}: {}", e.row + 1, e.column + 1, e.message),
                })
            };
            let contract = |query: &'static str, message: String| Error::Query {
                pack: name.to_owned(),
                grammar: entry.name.clone(),
                query,
                message,
            };
            let compile = |query: &'static str| -> Result<Query> {
                let compiled = compile(query)?;
                every_pattern_captures(&compiled, &source(query)?)
                    .map_err(|m| contract(query, m))?;
                Ok(compiled)
            };
            let definitions =
                definitions(compile("definitions")?).map_err(|m| contract("definitions", m))?;
            let references =
                references(compile("references")?).map_err(|m| contract("references", m))?;
            let roles = roles(compile("roles")?).map_err(|m| contract("roles", m))?;
            grammars.push(Grammar {
                name: entry.name.clone(),
                version: entry.version.clone(),
                language: ts,
                definitions,
                references,
                roles,
            });
        }
        let conventions = files
            .iter()
            .filter_map(|(file, text)| {
                let name = file.strip_prefix("conventions/")?;
                Some((name.to_owned(), text.clone()))
            })
            .collect();
        Ok(Self {
            manifest,
            language,
            grammars,
            conventions,
        })
    }
}

/// A pattern without capture is a mistake: a predicate written after its pattern instead of
/// inside it, for instance, becomes an empty pattern that matches every node.
fn every_pattern_captures(query: &Query, source: &str) -> Result<(), String> {
    for pattern in 0..query.pattern_count() {
        let text =
            &source[query.start_byte_for_pattern(pattern)..query.end_byte_for_pattern(pattern)];
        if !text.contains('@') {
            let line = source[..query.start_byte_for_pattern(pattern)]
                .lines()
                .count()
                + 1;
            return Err(format!(
                "the pattern at line {line} captures nothing: predicates go inside their pattern's parentheses"
            ));
        }
    }
    Ok(())
}

fn is_version(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Captures starting with `_` are helpers for predicates, outside the contract.
fn is_helper(name: &str) -> bool {
    name.starts_with('_')
}

fn definitions(query: Query) -> Result<Definitions, String> {
    let mut kinds = Vec::new();
    let (mut name, mut body, mut scope) = (None, None, None);
    let mut scope_parts = HashMap::new();
    for (index, capture) in query.capture_names().iter().enumerate() {
        let index = index as u32;
        let mut kind = None;
        match *capture {
            "name" => name = Some(index),
            "body" => body = Some(index),
            "scope" => scope = Some(index),
            other if other.starts_with("scope.") && other.len() > "scope.".len() => {
                scope_parts.insert(index, other.to_owned());
            }
            other if other.starts_with("definition.") => {
                let suffix = &other["definition.".len()..];
                kind = Some(symbol_kind(suffix).ok_or_else(|| {
                    format!("unknown symbol type in `@{other}` (module, type, trait, function, method, field, constant)")
                })?);
            }
            other if is_helper(other) => {}
            other => {
                return Err(format!(
                    "capture `@{other}` is not part of the contract (ADR 0012)"
                ));
            }
        }
        kinds.push(kind);
    }
    let mut segment = Vec::new();
    let mut parent = Vec::new();
    for pattern in 0..query.pattern_count() {
        let (mut seg, mut par) = (None, None);
        for property in query.property_settings(pattern) {
            let value = property.value.as_deref().map(str::to_owned);
            match &*property.key {
                "timonier.segment" => seg = value,
                "timonier.parent" => par = value,
                other => {
                    return Err(format!(
                        "property `{other}` is not part of the contract (ADR 0012)"
                    ));
                }
            }
        }
        segment.push(seg);
        parent.push(par);
    }
    Ok(Definitions {
        query,
        kinds,
        name,
        body,
        scope,
        scope_parts,
        segment,
        parent,
    })
}

fn symbol_kind(name: &str) -> Option<SymbolKind> {
    Some(match name {
        "module" => SymbolKind::Module,
        "type" => SymbolKind::Type,
        "trait" => SymbolKind::Trait,
        "function" => SymbolKind::Function,
        "method" => SymbolKind::Method,
        "field" => SymbolKind::Field,
        "constant" => SymbolKind::Constant,
        _ => return None,
    })
}

fn references(query: Query) -> Result<References, String> {
    let mut captures = Vec::new();
    for capture in query.capture_names() {
        captures.push(match *capture {
            "import" => Some(Capture::Import),
            "import.path" => Some(Capture::ImportPath),
            "import.alias" => Some(Capture::ImportAlias),
            "import.name" => Some(Capture::ImportName),
            "reference.call" => Some(Capture::Call),
            "reference.method" => Some(Capture::Method),
            "reference.type" => Some(Capture::Type),
            "reference.implements" => Some(Capture::Implements),
            "reference.inherits" => Some(Capture::Inherits),
            "reference.source" => Some(Capture::Source),
            other if is_helper(other) => None,
            other => {
                return Err(format!(
                    "capture `@{other}` is not part of the contract (ADR 0012)"
                ));
            }
        });
    }
    no_properties(&query)?;
    Ok(References { query, captures })
}

fn roles(query: Query) -> Result<Roles, String> {
    let mut roles = Vec::new();
    for capture in query.capture_names() {
        roles.push(match capture.strip_prefix("role.") {
            Some(role) => {
                Some(Role::parse(role).map_err(|_| format!("unknown role in `@{capture}`"))?)
            }
            None if is_helper(capture) => None,
            None => {
                return Err(format!(
                    "capture `@{capture}` is not part of the contract (ADR 0012)"
                ));
            }
        });
    }
    no_properties(&query)?;
    Ok(Roles { query, roles })
}

fn no_properties(query: &Query) -> Result<(), String> {
    for pattern in 0..query.pattern_count() {
        if let Some(property) = query.property_settings(pattern).first() {
            return Err(format!(
                "property `{}` is not part of the contract (ADR 0012)",
                property.key
            ));
        }
    }
    Ok(())
}

/// The packs of this build, indexed by file extension.
#[derive(Debug)]
pub struct Packs {
    packs: Vec<Pack>,
    by_extension: HashMap<String, (usize, usize)>,
}

macro_rules! pack_files {
    ($dir:literal, [$($file:literal),* $(,)?]) => {
        ($dir, &[$(($file, include_str!(concat!("../packs/", $dir, "/", $file)))),*])
    };
}

const BUILTIN: [(&str, &[(&str, &str)]); 2] = [
    pack_files!(
        "rust",
        [
            "pack.toml",
            "definitions.scm",
            "references.scm",
            "roles.scm",
            "conventions/instrumentation.md"
        ]
    ),
    pack_files!(
        "typescript",
        [
            "pack.toml",
            "definitions.scm",
            "references.scm",
            "references.tsx.scm",
            "roles.scm",
            "conventions/instrumentation.md"
        ]
    ),
];

impl Packs {
    /// The packs embedded in this build: Rust and TypeScript.
    pub fn builtin() -> Result<Self> {
        let packs = BUILTIN
            .iter()
            .map(|(name, files)| {
                let files = files
                    .iter()
                    .map(|(file, text)| ((*file).to_owned(), (*text).to_owned()))
                    .collect();
                Pack::from_files(name, &files)
            })
            .collect::<Result<Vec<_>>>()?;
        Self::new(packs)
    }

    pub fn new(packs: Vec<Pack>) -> Result<Self> {
        let mut by_extension = HashMap::new();
        let mut languages = Vec::new();
        for (p, pack) in packs.iter().enumerate() {
            if languages.contains(&pack.language) {
                return Err(Error::Pack {
                    pack: pack.language.to_string(),
                    reason: "two packs define this language".into(),
                });
            }
            languages.push(pack.language.clone());
            for (g, grammar) in pack.manifest.grammars.iter().enumerate() {
                for extension in &grammar.extensions {
                    if by_extension.insert(extension.clone(), (p, g)).is_some() {
                        return Err(Error::Pack {
                            pack: pack.language.to_string(),
                            reason: format!("extension `.{extension}` is claimed twice"),
                        });
                    }
                }
            }
        }
        Ok(Self {
            packs,
            by_extension,
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = &Pack> {
        self.packs.iter()
    }

    /// The pack and grammar that read a file, from its extension. `.d.ts` reads as `.ts`.
    pub(crate) fn for_path(&self, path: &str) -> Option<(&Pack, &Grammar)> {
        let file = path.rsplit('/').next().unwrap_or(path);
        let (_, extension) = file.rsplit_once('.')?;
        let (p, g) = self.by_extension.get(extension)?;
        let pack = &self.packs[*p];
        Some((pack, &pack.grammars[*g]))
    }

    /// The file extensions the packs read, sorted (`.d.ts` reads as `.ts`).
    pub fn extensions(&self) -> Vec<String> {
        let mut extensions: Vec<String> = self.by_extension.keys().cloned().collect();
        extensions.sort();
        extensions
    }

    /// Whether some pack reads this file.
    pub fn reads(&self, path: &str) -> bool {
        self.for_path(path).is_some()
    }

    pub fn language_of(&self, path: &str) -> Option<&Language> {
        self.for_path(path).map(|(pack, _)| &pack.language)
    }

    pub fn by_language(&self, language: &Language) -> Option<&Pack> {
        self.packs.iter().find(|p| p.language == *language)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn builtin_files(name: &str) -> BTreeMap<String, String> {
        let (_, files) = BUILTIN.iter().find(|(n, _)| *n == name).unwrap();
        files
            .iter()
            .map(|(f, t)| ((*f).to_owned(), (*t).to_owned()))
            .collect()
    }

    /// ADR 0034: each pack says how its tests run and write JUnit XML, and brings its
    /// conventions.
    #[test]
    fn builtin_packs_describe_their_runners_and_conventions() {
        let packs = Packs::builtin().unwrap();
        let pack = |name: &str| packs.iter().find(|p| p.language.as_str() == name).unwrap();
        let rust = &pack("rust").manifest.tests.runners;
        assert_eq!(rust.len(), 1);
        assert_eq!(rust[0].name, "cargo-nextest");
        assert!(rust[0].uses.is_none(), "always the Rust runner");
        let config = rust[0].config.as_ref().unwrap();
        assert_eq!(config.file, ".config/nextest.toml");
        assert!(config.text.contains("[profile.timonier.junit]"));
        let typescript: Vec<&str> = pack("typescript")
            .manifest
            .tests
            .runners
            .iter()
            .map(|r| r.name.as_str())
            .collect();
        assert_eq!(
            typescript,
            ["npm workspaces", "vitest", "node:test", "jest"]
        );
        for name in ["rust", "typescript"] {
            let conventions = &pack(name).conventions;
            assert!(conventions["instrumentation.md"].contains("code.function.name"));
        }
    }

    #[test]
    fn builtin_packs_load_and_compile_for_every_grammar() {
        let packs = Packs::builtin().unwrap();
        let languages: Vec<_> = packs.iter().map(|p| p.language.as_str()).collect();
        assert_eq!(languages, ["rust", "typescript"]);
        for (path, language, grammar) in [
            ("src/lib.rs", "rust", "rust"),
            ("src/App.tsx", "typescript", "tsx"),
            ("src/host.ts", "typescript", "typescript"),
            ("types/vscode.d.ts", "typescript", "typescript"),
            ("a.mts", "typescript", "typescript"),
        ] {
            let (pack, g) = packs.for_path(path).unwrap();
            assert_eq!(
                (pack.language.as_str(), g.name.as_str()),
                (language, grammar),
                "{path}"
            );
        }
        for path in ["README.md", "Makefile", "src/x.js", ".rs/x"] {
            assert!(!packs.reads(path), "{path}");
        }
        for pack in packs.iter() {
            for grammar in &pack.grammars {
                assert!(grammar.definitions.name.is_some(), "{}", grammar.name);
                assert!(grammar.definitions.kinds.iter().any(Option::is_some));
                assert!(
                    grammar
                        .references
                        .captures
                        .contains(&Some(Capture::ImportPath))
                );
            }
        }
        let tsx = &packs
            .by_language(&Language::new("typescript").unwrap())
            .unwrap()
            .grammars[1];
        let ts = &packs
            .by_language(&Language::new("typescript").unwrap())
            .unwrap()
            .grammars[0];
        assert!(tsx.references.query.pattern_count() > ts.references.query.pattern_count());
    }

    fn refused(files: &BTreeMap<String, String>) -> String {
        Pack::from_files("t", files).unwrap_err().to_string()
    }

    /// The cache key carries the grammar's version from the pack (ADR 0013): it must be the
    /// version this build compiles, as `Cargo.lock` resolves it.
    #[test]
    fn packs_name_the_grammar_versions_of_the_build() {
        let lock: toml::Table =
            toml::from_str(include_str!("../../../Cargo.lock")).expect("Cargo.lock parses");
        let locked = |name: &str| -> Vec<String> {
            lock["package"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|p| p["name"].as_str() == Some(name))
                .map(|p| p["version"].as_str().unwrap().to_owned())
                .collect()
        };
        for pack in Packs::builtin().unwrap().iter() {
            for grammar in &pack.manifest.grammars {
                assert_eq!(
                    locked(&grammar.crate_name),
                    std::slice::from_ref(&grammar.version),
                    "{} in the {} pack",
                    grammar.crate_name,
                    pack.language
                );
            }
        }
    }

    #[test]
    fn manifests_are_checked() {
        let mut files = builtin_files("rust");
        let manifest = files["pack.toml"].clone();
        let mut with = |old: &str, new: &str| {
            assert!(manifest.contains(old), "{old}");
            files.insert("pack.toml".into(), manifest.replacen(old, new, 1));
            refused(&files)
        };
        assert!(with("format = 1", "format = 1\ncolour = \"red\"").contains("colour"));
        assert!(with("format = 1", "format = 2").contains("format 2"));
        assert!(with("language = \"rust\"", "language = \"file\"").contains("reserved"));
        assert!(with("version = \"1.1.0\"", "version = \"1.0\"").contains("major.minor.patch"));
        assert!(with("[lsp]", "[lsp]\nserver = \"rust-analyzer\"").contains("server"));
        assert!(with("ecosystem = \"cargo\"", "ecosystem = \"maven\"").contains("maven"));
        assert!(with("name = \"rust\"", "name = \"cobol\"").contains("not compiled"));
        assert!(with("role = \"persistence\"", "role = \"database\"").contains("database"));
        assert!(with("confidence = 0.8", "confidence = 1.5").contains("outside"));
    }

    #[test]
    fn queries_follow_the_capture_contract() {
        let base = builtin_files("rust");
        let with = |query: &str, text: &str| {
            let mut files = base.clone();
            files.insert(query.into(), format!("{}\n{text}", base[query]));
            refused(&files)
        };
        assert!(
            with("definitions.scm", "(function_item) @definition.macro")
                .contains("unknown symbol type")
        );
        assert!(with("definitions.scm", "(function_item) @thing").contains("@thing"));
        assert!(
            with(
                "definitions.scm",
                "((function_item) @definition.function (#set! colour \"red\"))"
            )
            .contains("colour")
        );
        assert!(with("references.scm", "(identifier) @reference.read").contains("@reference.read"));
        assert!(with("roles.scm", "(identifier) @role.database").contains("unknown role"));
        assert!(with("definitions.scm", "(no_such_node) @name").contains("line"));
        // Helpers for predicates are allowed.
        let mut files = base.clone();
        files.insert(
            "roles.scm".into(),
            "((identifier) @_x @role.utility (#eq? @_x \"util\"))".into(),
        );
        Pack::from_files("t", &files).unwrap();
        // Grammar-specific additions only for the pack's grammars.
        files.insert("references.cobol.scm".into(), String::new());
        assert!(refused(&files).contains("unexpected file"));
    }

    #[test]
    fn two_packs_cannot_claim_an_extension_or_a_language() {
        let rust = || Pack::from_files("rust", &builtin_files("rust")).unwrap();
        let err = Packs::new(vec![rust(), rust()]).unwrap_err().to_string();
        assert!(err.contains("two packs"), "{err}");
        let mut other = builtin_files("rust");
        other.insert(
            "pack.toml".into(),
            other["pack.toml"].replace("language = \"rust\"", "language = \"rust2\""),
        );
        let err = Packs::new(vec![rust(), Pack::from_files("rust2", &other).unwrap()])
            .unwrap_err()
            .to_string();
        assert!(err.contains("claimed twice"), "{err}");
    }
}
