//! Edges of the graph, their provenance and confidence (spec « Schema », ADR 0009).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{Error, NodeId, Result};

/// Kinds of relations (spec).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "&'static str")]
pub enum EdgeKind {
    Imports,
    Calls,
    References,
    Implements,
    Inherits,
    DependsOn,
}

impl EdgeKind {
    pub const ALL: [Self; 6] = [
        Self::Imports,
        Self::Calls,
        Self::References,
        Self::Implements,
        Self::Inherits,
        Self::DependsOn,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Imports => "imports",
            Self::Calls => "calls",
            Self::References => "references",
            Self::Implements => "implements",
            Self::Inherits => "inherits",
            Self::DependsOn => "depends_on",
        }
    }

    pub fn parse(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str() == name)
            .ok_or_else(|| Error::UnknownName {
                what: "edge kind",
                value: name.to_owned(),
            })
    }
}

impl From<EdgeKind> for &'static str {
    fn from(kind: EdgeKind) -> Self {
        kind.as_str()
    }
}

impl TryFrom<String> for EdgeKind {
    type Error = Error;

    fn try_from(name: String) -> Result<Self> {
        Self::parse(&name)
    }
}

impl fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where an edge comes from (spec). `Intended` is the whiteboard and never enters the actual graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "&'static str")]
pub enum Provenance {
    TreeSitter,
    Lsp,
    Scip,
    Manifest,
    Inferred,
    Intended,
}

impl Provenance {
    pub const ALL: [Self; 6] = [
        Self::TreeSitter,
        Self::Lsp,
        Self::Scip,
        Self::Manifest,
        Self::Inferred,
        Self::Intended,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TreeSitter => "tree_sitter",
            Self::Lsp => "lsp",
            Self::Scip => "scip",
            Self::Manifest => "manifest",
            Self::Inferred => "inferred",
            Self::Intended => "intended",
        }
    }

    pub fn parse(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|provenance| provenance.as_str() == name)
            .ok_or_else(|| Error::UnknownName {
                what: "provenance",
                value: name.to_owned(),
            })
    }

    /// Reliability rank: Scip > Lsp > Manifest > TreeSitter > Inferred (ADR 0009).
    pub const fn rank(self) -> u8 {
        match self {
            Self::Scip => 5,
            Self::Lsp => 4,
            Self::Manifest => 3,
            Self::TreeSitter => 2,
            Self::Inferred => 1,
            Self::Intended => 0,
        }
    }

    /// Compiler-grade sources, whose verification overrides approximate ones.
    pub const fn is_precise(self) -> bool {
        matches!(self, Self::Scip | Self::Lsp)
    }

    /// Sources that guess: discarded for an edge kind a precise source has verified.
    pub const fn is_approximate(self) -> bool {
        matches!(self, Self::TreeSitter | Self::Inferred)
    }
}

impl From<Provenance> for &'static str {
    fn from(provenance: Provenance) -> Self {
        provenance.as_str()
    }
}

impl TryFrom<String> for Provenance {
    type Error = Error;

    fn try_from(name: String) -> Result<Self> {
        Self::parse(&name)
    }
}

impl fmt::Display for Provenance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How sure a source is of an edge, between 0 and 1.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f32", into = "f32")]
pub struct Confidence(f32);

impl Confidence {
    pub const CERTAIN: Self = Self(1.0);

    pub fn new(value: f32) -> Result<Self> {
        if (0.0..=1.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err(Error::InvalidConfidence(value))
        }
    }

    pub const fn get(self) -> f32 {
        self.0
    }
}

impl TryFrom<f32> for Confidence {
    type Error = Error;

    fn try_from(value: f32) -> Result<Self> {
        Self::new(value)
    }
}

impl From<Confidence> for f32 {
    fn from(confidence: Confidence) -> Self {
        confidence.0
    }
}

/// Identity of an edge: there is at most one edge per (source, destination, kind).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EdgeKey {
    pub src: NodeId,
    pub dst: NodeId,
    pub kind: EdgeKind,
}

/// An edge of the merged graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub src: NodeId,
    pub dst: NodeId,
    pub kind: EdgeKind,
    pub provenance: Provenance,
    pub confidence: Confidence,
    /// How many occurrences the edge stands for (call sites, imports…).
    pub weight: u32,
}

impl Edge {
    pub fn key(&self) -> EdgeKey {
        EdgeKey {
            src: self.src.clone(),
            dst: self.dst.clone(),
            kind: self.kind,
        }
    }

    /// True when both carry the same versioned attributes (ADR 0010).
    pub fn same_version(&self, other: &Self) -> bool {
        self.key() == other.key()
            && self.provenance == other.provenance
            && self.confidence == other.confidence
            && self.weight == other.weight
    }
}

/// An edge as one source saw it, before fusion (ADR 0009).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    pub src: NodeId,
    pub dst: NodeId,
    pub kind: EdgeKind,
    pub confidence: Confidence,
    pub weight: u32,
}

impl Observation {
    pub fn new(src: NodeId, dst: NodeId, kind: EdgeKind) -> Self {
        Self {
            src,
            dst,
            kind,
            confidence: Confidence::CERTAIN,
            weight: 1,
        }
    }

    pub fn with_confidence(mut self, confidence: Confidence) -> Self {
        self.confidence = confidence;
        self
    }

    pub fn with_weight(mut self, weight: u32) -> Self {
        self.weight = weight;
        self
    }

    pub fn key(&self) -> EdgeKey {
        EdgeKey {
            src: self.src.clone(),
            dst: self.dst.clone(),
            kind: self.kind,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for kind in EdgeKind::ALL {
            assert_eq!(EdgeKind::parse(kind.as_str()).unwrap(), kind);
        }
        for provenance in Provenance::ALL {
            let json = serde_json::to_string(&provenance).unwrap();
            assert_eq!(
                serde_json::from_str::<Provenance>(&json).unwrap(),
                provenance
            );
        }
        assert_eq!(
            serde_json::to_string(&Provenance::TreeSitter).unwrap(),
            "\"tree_sitter\""
        );
        assert_eq!(
            serde_json::to_string(&EdgeKind::DependsOn).unwrap(),
            "\"depends_on\""
        );
    }

    #[test]
    fn provenances_are_ranked_as_decided() {
        let mut ranked = Provenance::ALL;
        ranked.sort_by_key(|p| std::cmp::Reverse(p.rank()));
        assert_eq!(
            ranked,
            [
                Provenance::Scip,
                Provenance::Lsp,
                Provenance::Manifest,
                Provenance::TreeSitter,
                Provenance::Inferred,
                Provenance::Intended
            ]
        );
    }

    #[test]
    fn confidence_stays_between_zero_and_one() {
        assert!(Confidence::new(-0.1).is_err());
        assert!(Confidence::new(1.01).is_err());
        assert!(Confidence::new(f32::NAN).is_err());
        assert!(serde_json::from_str::<Confidence>("2.0").is_err());
        assert_eq!(Confidence::new(0.5).unwrap().get(), 0.5);
    }

    #[test]
    fn edges_round_trip_through_json() {
        let edge = Edge {
            src: NodeId::file("a.rs").unwrap(),
            dst: NodeId::file("b.rs").unwrap(),
            kind: EdgeKind::Calls,
            provenance: Provenance::TreeSitter,
            confidence: Confidence::new(0.6).unwrap(),
            weight: 3,
        };
        let json = serde_json::to_string(&edge).unwrap();
        assert_eq!(serde_json::from_str::<Edge>(&json).unwrap(), edge);
        let observation =
            Observation::new(edge.src.clone(), edge.dst.clone(), EdgeKind::Imports).with_weight(2);
        let json = serde_json::to_string(&observation).unwrap();
        assert_eq!(
            serde_json::from_str::<Observation>(&json).unwrap(),
            observation
        );
    }
}
