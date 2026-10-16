//! Stable identities of nodes and languages (ADR 0008).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Id prefixes reserved for nodes that are not symbols; they can never be languages.
pub const RESERVED_PREFIXES: [&str; 5] = ["repo", "pkg", "component", "file", "scip"];

/// A language id defined by a language pack, such as `rust` or `typescript`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Language(String);

impl Language {
    pub fn new(name: impl Into<String>) -> Result<Self> {
        let name = name.into();
        match language_problem(&name) {
            Some(reason) => Err(Error::InvalidLanguage {
                language: name,
                reason,
            }),
            None => Ok(Self(name)),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// `[a-z][a-z0-9_+-]*`, not a reserved prefix.
fn language_problem(name: &str) -> Option<&'static str> {
    let mut chars = name.chars();
    match chars.next() {
        None => return Some("empty"),
        Some(first) if !first.is_ascii_lowercase() => {
            return Some("must start with a lowercase ASCII letter");
        }
        Some(_) => {}
    }
    if !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '+' | '-'))
    {
        return Some("only lowercase ASCII letters, digits, `_`, `+` and `-` are allowed");
    }
    if RESERVED_PREFIXES.contains(&name) {
        return Some("reserved for ids of nodes that are not symbols");
    }
    None
}

impl TryFrom<String> for Language {
    type Error = Error;

    fn try_from(name: String) -> Result<Self> {
        Self::new(name)
    }
}

impl From<Language> for String {
    fn from(language: Language) -> Self {
        language.0
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a node id designates, read from its prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdForm<'a> {
    Repo { name: &'a str },
    Package { ecosystem: &'a str, name: &'a str },
    Component { key: &'a str },
    File { path: &'a str },
    Scip { symbol: &'a str },
    Qualified { language: &'a str, name: &'a str },
}

/// The stable identity of a node, as a string whose prefix says what it designates:
/// `repo:`, `pkg:<ecosystem>/`, `component:`, `file:`, `scip:` or `<language>:`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NodeId(String);

impl NodeId {
    pub fn repo(name: &str) -> Result<Self> {
        Self::parse(format!("repo:{name}"))
    }

    /// A package, with an ecosystem as in package URLs: `cargo`, `npm`…
    pub fn package(ecosystem: &str, name: &str) -> Result<Self> {
        Self::parse(format!("pkg:{ecosystem}/{name}"))
    }

    pub fn component(key: &str) -> Result<Self> {
        Self::parse(format!("component:{key}"))
    }

    /// A file, by its path relative to the repository root, with `/` separators.
    pub fn file(path: &str) -> Result<Self> {
        Self::parse(format!("file:{path}"))
    }

    /// A symbol known to SCIP, by its SCIP symbol string, kept verbatim.
    pub fn scip(symbol: &str) -> Result<Self> {
        Self::parse(format!("scip:{symbol}"))
    }

    /// A symbol without SCIP symbol: `<language>:<qualified name>`.
    pub fn qualified(language: &Language, name: &str) -> Result<Self> {
        Self::parse(format!("{language}:{name}"))
    }

    pub fn parse(id: impl Into<String>) -> Result<Self> {
        let id = id.into();
        match form_of(&id) {
            Ok(_) => Ok(Self(id)),
            Err(reason) => Err(Error::InvalidId { id, reason }),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn form(&self) -> IdForm<'_> {
        form_of(&self.0).expect("a NodeId is validated when it is built")
    }
}

fn form_of(id: &str) -> Result<IdForm<'_>, &'static str> {
    if id.chars().any(char::is_control) {
        return Err("contains a control character");
    }
    let (prefix, rest) = id.split_once(':').ok_or("missing `<prefix>:`")?;
    if rest.is_empty() {
        return Err("nothing after the prefix");
    }
    match prefix {
        "repo" => Ok(IdForm::Repo { name: rest }),
        "pkg" => {
            let (ecosystem, name) = rest
                .split_once('/')
                .ok_or("a package id is `pkg:<ecosystem>/<name>`")?;
            let mut chars = ecosystem.chars();
            let valid_ecosystem = chars.next().is_some_and(|c| c.is_ascii_lowercase())
                && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
            if !valid_ecosystem {
                return Err("a package ecosystem is `[a-z][a-z0-9-]*`");
            }
            if name.is_empty() {
                return Err("a package id needs a name");
            }
            Ok(IdForm::Package { ecosystem, name })
        }
        "component" => Ok(IdForm::Component { key: rest }),
        "file" => {
            if rest.starts_with('/') || rest.contains('\\') {
                return Err("a file path is relative and uses `/` separators");
            }
            if rest
                .split('/')
                .any(|segment| matches!(segment, "" | "." | ".."))
            {
                return Err("a file path has no empty, `.` or `..` segment");
            }
            Ok(IdForm::File { path: rest })
        }
        "scip" => Ok(IdForm::Scip { symbol: rest }),
        language => match language_problem(language) {
            Some(reason) => Err(reason),
            None => Ok(IdForm::Qualified {
                language,
                name: rest,
            }),
        },
    }
}

impl TryFrom<String> for NodeId {
    type Error = Error;

    fn try_from(id: String) -> Result<Self> {
        Self::parse(id)
    }
}

impl From<NodeId> for String {
    fn from(id: NodeId) -> Self {
        id.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn rust() -> Language {
        Language::new("rust").unwrap()
    }

    #[test]
    fn builds_every_form() {
        assert_eq!(
            NodeId::repo("timonier").unwrap().form(),
            IdForm::Repo { name: "timonier" }
        );
        assert_eq!(
            NodeId::package("npm", "@timonier/webview").unwrap().form(),
            IdForm::Package {
                ecosystem: "npm",
                name: "@timonier/webview"
            }
        );
        assert_eq!(
            NodeId::file("crates/timonier-core/src/lib.rs")
                .unwrap()
                .form(),
            IdForm::File {
                path: "crates/timonier-core/src/lib.rs"
            }
        );
        let symbol = "rust-analyzer cargo timonier-core 0.1.0 id/NodeId#";
        assert_eq!(
            NodeId::scip(symbol).unwrap().form(),
            IdForm::Scip { symbol }
        );
        assert_eq!(
            NodeId::qualified(&rust(), "timonier_core::id::NodeId")
                .unwrap()
                .as_str(),
            "rust:timonier_core::id::NodeId"
        );
    }

    #[test]
    fn rejects_malformed_ids() {
        for id in [
            "",
            "no-prefix",
            "repo:",
            "pkg:cargo",
            "pkg:Cargo/x",
            "pkg:cargo/",
            "file:/abs/path",
            "file:a//b",
            "file:a/../b",
            "file:a\\b",
            "Rust:x",
            "rust:a\nb",
        ] {
            assert!(NodeId::parse(id).is_err(), "{id:?} should be rejected");
        }
    }

    #[test]
    fn reserved_prefixes_are_not_languages() {
        for prefix in RESERVED_PREFIXES {
            assert!(Language::new(prefix).is_err(), "{prefix}");
        }
        assert!(Language::new("typescript").is_ok());
        assert!(Language::new("c++").is_ok());
    }

    #[test]
    fn serializes_as_a_plain_string() {
        let id = NodeId::file("src/main.rs").unwrap();
        assert_eq!(serde_json::to_string(&id).unwrap(), "\"file:src/main.rs\"");
        assert_eq!(
            serde_json::from_str::<NodeId>("\"file:src/main.rs\"").unwrap(),
            id
        );
        assert!(serde_json::from_str::<NodeId>("\"file:../x\"").is_err());
    }

    fn segment() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9_.-]{1,12}".prop_filter("not . or ..", |s| s != "." && s != "..")
    }

    proptest! {
        #[test]
        fn file_ids_round_trip(segments in prop::collection::vec(segment(), 1..6)) {
            let path = segments.join("/");
            let id = NodeId::file(&path).unwrap();
            prop_assert_eq!(id.form(), IdForm::File { path: &path });
            prop_assert_eq!(NodeId::parse(id.as_str()).unwrap(), id);
        }

        #[test]
        fn qualified_ids_round_trip(language in "[a-z][a-z0-9_+-]{0,10}", name in "[^\\p{Cc}]{1,40}") {
            prop_assume!(!RESERVED_PREFIXES.contains(&language.as_str()));
            let language = Language::new(language).unwrap();
            let id = NodeId::qualified(&language, &name).unwrap();
            prop_assert_eq!(id.form(), IdForm::Qualified { language: language.as_str(), name: &name });
            let json = serde_json::to_string(&id).unwrap();
            prop_assert_eq!(serde_json::from_str::<NodeId>(&json).unwrap(), id);
        }
    }
}
