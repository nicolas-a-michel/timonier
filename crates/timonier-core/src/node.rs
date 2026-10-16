//! Nodes of the graph: kinds, positions, fingerprints (spec « Schéma », ADR 0008).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{Error, IdForm, Language, NodeId, Result};

/// Kinds of symbols (spec: module, type, trait, function, method, field, constant).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Module,
    Type,
    Trait,
    Function,
    Method,
    Field,
    Constant,
}

/// Kinds of nodes. Serialized as one flat name: `repo`, `component`, …, `function`, `method`…
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "&'static str")]
pub enum NodeKind {
    Repo,
    Component,
    Package,
    File,
    Symbol(SymbolKind),
}

impl NodeKind {
    pub const ALL: [Self; 11] = [
        Self::Repo,
        Self::Component,
        Self::Package,
        Self::File,
        Self::Symbol(SymbolKind::Module),
        Self::Symbol(SymbolKind::Type),
        Self::Symbol(SymbolKind::Trait),
        Self::Symbol(SymbolKind::Function),
        Self::Symbol(SymbolKind::Method),
        Self::Symbol(SymbolKind::Field),
        Self::Symbol(SymbolKind::Constant),
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Repo => "repo",
            Self::Component => "component",
            Self::Package => "package",
            Self::File => "file",
            Self::Symbol(SymbolKind::Module) => "module",
            Self::Symbol(SymbolKind::Type) => "type",
            Self::Symbol(SymbolKind::Trait) => "trait",
            Self::Symbol(SymbolKind::Function) => "function",
            Self::Symbol(SymbolKind::Method) => "method",
            Self::Symbol(SymbolKind::Field) => "field",
            Self::Symbol(SymbolKind::Constant) => "constant",
        }
    }

    pub fn parse(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str() == name)
            .ok_or_else(|| Error::UnknownName {
                what: "node kind",
                value: name.to_owned(),
            })
    }

    pub const fn is_symbol(self) -> bool {
        matches!(self, Self::Symbol(_))
    }
}

impl From<NodeKind> for &'static str {
    fn from(kind: NodeKind) -> Self {
        kind.as_str()
    }
}

impl TryFrom<String> for NodeKind {
    type Error = Error;

    fn try_from(name: String) -> Result<Self> {
        Self::parse(&name)
    }
}

impl fmt::Display for NodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A position in a file: 0-based line, column in UTF-8 bytes (ADR 0008).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

impl Position {
    pub const fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }
}

/// A range in a file; it never ends before it starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "RawSpan", into = "RawSpan")]
pub struct Span {
    start: Position,
    end: Position,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
struct RawSpan {
    start: Position,
    end: Position,
}

impl Span {
    pub fn new(start: Position, end: Position) -> Result<Self> {
        if end < start {
            return Err(Error::InvalidSpan);
        }
        Ok(Self { start, end })
    }

    pub const fn start(&self) -> Position {
        self.start
    }

    pub const fn end(&self) -> Position {
        self.end
    }
}

impl TryFrom<RawSpan> for Span {
    type Error = Error;

    fn try_from(raw: RawSpan) -> Result<Self> {
        Self::new(raw.start, raw.end)
    }
}

impl From<Span> for RawSpan {
    fn from(span: Span) -> Self {
        Self {
            start: span.start,
            end: span.end,
        }
    }
}

/// A 32-byte hash of a content (a file, or a normalised symbol body), shown in hexadecimal.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    pub fn from_hex(hex: &str) -> Result<Self> {
        if hex.len() != 64 || !hex.is_ascii() {
            return Err(Error::InvalidHash("expected 64 hexadecimal digits"));
        }
        let mut bytes = [0_u8; 32];
        let (pairs, _) = hex.as_bytes().as_chunks::<2>();
        for (byte, pair) in bytes.iter_mut().zip(pairs) {
            let pair = std::str::from_utf8(pair).map_err(|_| Error::InvalidHash("not ASCII"))?;
            *byte =
                u8::from_str_radix(pair, 16).map_err(|_| Error::InvalidHash("not hexadecimal"))?;
        }
        Ok(Self(bytes))
    }
}

impl fmt::Debug for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContentHash({}…)", &self.to_hex()[..12])
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl TryFrom<String> for ContentHash {
    type Error = Error;

    fn try_from(hex: String) -> Result<Self> {
        Self::from_hex(&hex)
    }
}

impl From<ContentHash> for String {
    fn from(hash: ContentHash) -> Self {
        hash.to_hex()
    }
}

/// A node of the graph. Its place in the hierarchy is its `parent` (a strict tree, ADR 0008).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    /// Display name, usually the last segment of the id.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<Language>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<NodeId>,
    /// Normalised signature of a symbol, from its language pack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Hash of the normalised body of a symbol, from its language pack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_hash: Option<ContentHash>,
}

impl Node {
    pub fn new(id: NodeId, kind: NodeKind, name: impl Into<String>) -> Self {
        Self {
            id,
            kind,
            name: name.into(),
            language: None,
            span: None,
            parent: None,
            signature: None,
            body_hash: None,
        }
    }

    pub fn with_parent(mut self, parent: NodeId) -> Self {
        self.parent = Some(parent);
        self
    }

    pub fn with_language(mut self, language: Language) -> Self {
        self.language = Some(language);
        self
    }

    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    pub fn with_signature(mut self, signature: impl Into<String>) -> Self {
        self.signature = Some(signature.into());
        self
    }

    pub fn with_body_hash(mut self, hash: ContentHash) -> Self {
        self.body_hash = Some(hash);
        self
    }

    /// Checks that the id's prefix designates this kind of node, and that a qualified symbol id
    /// names the node's language (ADR 0008).
    pub fn check_identity(&self) -> Result<()> {
        let consistent = match (self.kind, self.id.form()) {
            (NodeKind::Repo, IdForm::Repo { .. })
            | (NodeKind::Package, IdForm::Package { .. })
            | (NodeKind::Component, IdForm::Component { .. })
            | (NodeKind::File, IdForm::File { .. })
            | (NodeKind::Symbol(_), IdForm::Scip { .. }) => true,
            (NodeKind::Symbol(_), IdForm::Qualified { language, .. }) => self
                .language
                .as_ref()
                .is_none_or(|own| own.as_str() == language),
            _ => false,
        };
        if consistent {
            Ok(())
        } else {
            Err(Error::KindMismatch {
                id: self.id.clone(),
                kind: self.kind,
            })
        }
    }

    /// True when both carry the same versioned attributes: everything but the span, which
    /// moves with every edit above the node (ADR 0010).
    pub fn same_version(&self, other: &Self) -> bool {
        self.id == other.id
            && self.kind == other.kind
            && self.name == other.name
            && self.language == other.language
            && self.parent == other.parent
            && self.signature == other.signature
            && self.body_hash == other.body_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_kinds_have_one_flat_name_each() {
        for kind in NodeKind::ALL {
            assert_eq!(NodeKind::parse(kind.as_str()).unwrap(), kind);
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(serde_json::from_str::<NodeKind>(&json).unwrap(), kind);
        }
        assert_eq!(
            serde_json::to_string(&NodeKind::Symbol(SymbolKind::Method)).unwrap(),
            "\"method\""
        );
        assert!(NodeKind::parse("symbol").is_err());
    }

    #[test]
    fn symbol_kinds_serialize_alone() {
        assert_eq!(
            serde_json::to_string(&SymbolKind::Constant).unwrap(),
            "\"constant\""
        );
        assert_eq!(
            serde_json::from_str::<SymbolKind>("\"trait\"").unwrap(),
            SymbolKind::Trait
        );
    }

    #[test]
    fn the_id_prefix_must_match_the_kind() {
        let node = |id: &str, kind| Node::new(NodeId::parse(id).unwrap(), kind, "x");
        assert!(node("file:a.rs", NodeKind::File).check_identity().is_ok());
        assert!(
            node("pkg:cargo/a", NodeKind::Package)
                .check_identity()
                .is_ok()
        );
        assert!(
            node("scip:x y z", NodeKind::Symbol(SymbolKind::Type))
                .check_identity()
                .is_ok()
        );
        assert!(
            node("file:a.rs", NodeKind::Package)
                .check_identity()
                .is_err()
        );
        assert!(node("rust:a::f", NodeKind::File).check_identity().is_err());
        let typescript = Language::new("typescript").unwrap();
        let mismatched =
            node("rust:a::f", NodeKind::Symbol(SymbolKind::Function)).with_language(typescript);
        assert!(mismatched.check_identity().is_err());
    }

    #[test]
    fn spans_never_end_before_they_start() {
        assert!(Span::new(Position::new(2, 0), Position::new(1, 9)).is_err());
        assert!(
            serde_json::from_str::<Span>(
                r#"{"start":{"line":3,"column":0},"end":{"line":1,"column":0}}"#
            )
            .is_err()
        );
        let span = Span::new(Position::new(1, 4), Position::new(1, 4)).unwrap();
        assert_eq!(
            serde_json::from_str::<Span>(&serde_json::to_string(&span).unwrap()).unwrap(),
            span
        );
    }

    #[test]
    fn content_hashes_are_hexadecimal() {
        let hash = ContentHash::from_bytes([0xab; 32]);
        assert_eq!(hash.to_hex(), "ab".repeat(32));
        assert_eq!(ContentHash::from_hex(&hash.to_hex()).unwrap(), hash);
        assert!(ContentHash::from_hex("ab").is_err());
        assert!(ContentHash::from_hex(&"zz".repeat(32)).is_err());
        assert!(ContentHash::from_hex(&"é".repeat(32)).is_err());
    }

    #[test]
    fn nodes_round_trip_through_json() {
        let node = Node::new(
            NodeId::parse("rust:timonier_core::node::Node").unwrap(),
            NodeKind::Symbol(SymbolKind::Type),
            "Node",
        )
        .with_parent(NodeId::file("crates/timonier-core/src/node.rs").unwrap())
        .with_language(Language::new("rust").unwrap())
        .with_span(Span::new(Position::new(10, 0), Position::new(20, 1)).unwrap())
        .with_signature("pub struct Node")
        .with_body_hash(ContentHash::from_bytes([7; 32]));
        let json = serde_json::to_string(&node).unwrap();
        assert_eq!(serde_json::from_str::<Node>(&json).unwrap(), node);

        let bare = Node::new(
            NodeId::repo("timonier").unwrap(),
            NodeKind::Repo,
            "timonier",
        );
        assert_eq!(
            serde_json::to_string(&bare).unwrap(),
            r#"{"id":"repo:timonier","kind":"repo","name":"timonier"}"#
        );
    }

    #[test]
    fn the_span_is_not_part_of_the_version() {
        let id = NodeId::parse("rust:a::f").unwrap();
        let before = Node::new(id.clone(), NodeKind::Symbol(SymbolKind::Function), "f")
            .with_span(Span::new(Position::new(1, 0), Position::new(2, 0)).unwrap());
        let shifted = before
            .clone()
            .with_span(Span::new(Position::new(5, 0), Position::new(6, 0)).unwrap());
        assert!(before.same_version(&shifted));
        assert!(!before.same_version(&shifted.with_signature("fn f()")));
    }
}
