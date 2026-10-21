//! Snapshots (spec « Storage », ADR 0010).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "&'static str")]
pub enum SnapshotKind {
    Commit,
    ReviewMark,
    WorkingTree,
}

impl SnapshotKind {
    pub const ALL: [Self; 3] = [Self::Commit, Self::ReviewMark, Self::WorkingTree];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Commit => "commit",
            Self::ReviewMark => "review_mark",
            Self::WorkingTree => "working_tree",
        }
    }

    pub fn parse(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str() == name)
            .ok_or_else(|| Error::UnknownName {
                what: "snapshot kind",
                value: name.to_owned(),
            })
    }
}

impl From<SnapshotKind> for &'static str {
    fn from(kind: SnapshotKind) -> Self {
        kind.as_str()
    }
}

impl TryFrom<String> for SnapshotKind {
    type Error = Error;

    fn try_from(name: String) -> Result<Self> {
        Self::parse(&name)
    }
}

impl fmt::Display for SnapshotKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A point of the graph's history. Exactly one snapshot is open (not frozen): the working-tree
/// head that every change is recorded in (ADR 0010).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub seq: u64,
    pub kind: SnapshotKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_sha: Option<String>,
    /// Milliseconds since the Unix epoch: when it was frozen (when it was opened, for the head).
    pub timestamp_ms: i64,
    pub frozen: bool,
}

/// How to freeze the head.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Freeze {
    Commit { sha: String },
    ReviewMark,
    WorkingTree,
}

impl Freeze {
    pub const fn kind(&self) -> SnapshotKind {
        match self {
            Self::Commit { .. } => SnapshotKind::Commit,
            Self::ReviewMark => SnapshotKind::ReviewMark,
            Self::WorkingTree => SnapshotKind::WorkingTree,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_use_the_spec_names() {
        let names: Vec<_> = SnapshotKind::ALL.iter().map(|k| k.as_str()).collect();
        assert_eq!(names, ["commit", "review_mark", "working_tree"]);
        for kind in SnapshotKind::ALL {
            assert_eq!(SnapshotKind::parse(kind.as_str()).unwrap(), kind);
        }
    }

    #[test]
    fn snapshots_round_trip_through_json() {
        let snapshot = Snapshot {
            seq: 4,
            kind: SnapshotKind::Commit,
            commit_sha: Some("6d3e0f8".into()),
            timestamp_ms: 1_760_000_000_000,
            frozen: true,
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        assert_eq!(serde_json::from_str::<Snapshot>(&json).unwrap(), snapshot);
        let freeze = Freeze::Commit { sha: "abc".into() };
        assert_eq!(
            serde_json::to_string(&freeze).unwrap(),
            r#"{"kind":"commit","sha":"abc"}"#
        );
        assert_eq!(freeze.kind(), SnapshotKind::Commit);
        for freeze in [freeze, Freeze::ReviewMark, Freeze::WorkingTree] {
            let json = serde_json::to_string(&freeze).unwrap();
            assert_eq!(serde_json::from_str::<Freeze>(&json).unwrap(), freeze);
        }
    }
}
