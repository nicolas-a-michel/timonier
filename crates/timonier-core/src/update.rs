//! Scope updates: what producers (indexing, LSP, structure) hand to the store (ADR 0009).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{ContentHash, Error, IdForm, Node, NodeId, Result, SourceObservations};

/// The unit of replacement: `structure`, or `file:<path>` for the content of one file.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ScopeKey(String);

impl ScopeKey {
    pub const STRUCTURE: &'static str = "structure";

    /// `structure`, or the id of a file (`file:<path>`) for its content (ADR 0009).
    pub fn new(key: impl Into<String>) -> Result<Self> {
        let key = key.into();
        if key == Self::STRUCTURE {
            return Ok(Self(key));
        }
        match NodeId::parse(key.clone()) {
            Ok(id) if matches!(id.form(), IdForm::File { .. }) => Ok(Self(key)),
            _ => Err(Error::InvalidScope {
                key,
                reason: "a scope is `structure` or a file id, `file:<path>`",
            }),
        }
    }

    /// The scope of the repo, packages, components and the placement of files.
    pub fn structure() -> Self {
        Self(Self::STRUCTURE.to_owned())
    }

    /// The scope of a file's content: its symbols and the edges leaving its code. It shares the
    /// file node's id.
    pub fn of_file(file: &NodeId) -> Result<Self> {
        match file.form() {
            IdForm::File { .. } => Ok(Self(file.as_str().to_owned())),
            _ => Err(Error::InvalidScope {
                key: file.to_string(),
                reason: "not a file id",
            }),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The file whose content this scope holds; `None` for the structure.
    pub fn file(&self) -> Option<NodeId> {
        if self.0 == Self::STRUCTURE {
            None
        } else {
            NodeId::parse(self.0.clone()).ok()
        }
    }
}

impl TryFrom<String> for ScopeKey {
    type Error = Error;

    fn try_from(key: String) -> Result<Self> {
        Self::new(key)
    }
}

impl From<ScopeKey> for String {
    fn from(key: ScopeKey) -> Self {
        key.0
    }
}

impl fmt::Display for ScopeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A complete contribution to one scope. What is given replaces what the scope had; what is
/// `None` is kept. A new content hash makes the observations of the other sources stale.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScopeUpdate {
    pub scope: ScopeKey,
    /// Drop everything the scope holds (its file was deleted, for example).
    #[serde(default)]
    pub remove: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<ContentHash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nodes: Option<Vec<Node>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observations: Option<SourceObservations>,
}

impl ScopeUpdate {
    pub fn new(scope: ScopeKey) -> Self {
        Self {
            scope,
            remove: false,
            content_hash: None,
            nodes: None,
            observations: None,
        }
    }

    /// Everything the scope holds disappears.
    pub fn removal(scope: ScopeKey) -> Self {
        Self {
            remove: true,
            ..Self::new(scope)
        }
    }

    pub fn with_content_hash(mut self, hash: ContentHash) -> Self {
        self.content_hash = Some(hash);
        self
    }

    pub fn with_nodes(mut self, nodes: Vec<Node>) -> Self {
        self.nodes = Some(nodes);
        self
    }

    pub fn with_observations(mut self, observations: SourceObservations) -> Self {
        self.observations = Some(observations);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EdgeKind, NodeKind, Observation, Provenance};

    #[test]
    fn file_scopes_share_the_file_id() {
        let file = NodeId::file("src/lib.rs").unwrap();
        assert_eq!(
            ScopeKey::of_file(&file).unwrap().as_str(),
            "file:src/lib.rs"
        );
        assert_eq!(ScopeKey::structure().as_str(), "structure");
        assert_eq!(
            ScopeKey::new("file:src/lib.rs").unwrap(),
            ScopeKey::of_file(&file).unwrap()
        );
        for key in ["", "elsewhere", "pkg:cargo/a", "file:../x", "a\u{0}b"] {
            assert!(ScopeKey::new(key).is_err(), "{key:?}");
        }
        assert!(ScopeKey::of_file(&NodeId::repo("r").unwrap()).is_err());
        assert_eq!(ScopeKey::of_file(&file).unwrap().file(), Some(file));
        assert_eq!(ScopeKey::structure().file(), None);
    }

    #[test]
    fn updates_round_trip_through_json() {
        let file = NodeId::file("src/lib.rs").unwrap();
        let symbol = NodeId::parse("rust:crate::f").unwrap();
        let update = ScopeUpdate::new(ScopeKey::of_file(&file).unwrap())
            .with_content_hash(ContentHash::from_bytes([1; 32]))
            .with_nodes(vec![
                Node::new(symbol.clone(), NodeKind::parse("function").unwrap(), "f")
                    .with_parent(file.clone()),
            ])
            .with_observations(
                SourceObservations::new(
                    Provenance::Lsp,
                    vec![Observation::new(symbol.clone(), symbol, EdgeKind::Calls)],
                )
                .verifying([EdgeKind::Calls])
                .computed_on(ContentHash::from_bytes([1; 32])),
            );
        let json = serde_json::to_string(&update).unwrap();
        assert_eq!(serde_json::from_str::<ScopeUpdate>(&json).unwrap(), update);
        assert!(ScopeUpdate::removal(ScopeKey::structure()).remove);
    }
}
