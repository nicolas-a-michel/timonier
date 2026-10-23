//! A SCIP index read into the graph (ADR 0073, PLAN.md 11.3): three small projects, indexed by
//! rust-analyzer (2026-10-19), scip-typescript 0.5.2 and scip-python 0.6.8. Their indexes are in
//! `fixtures/scip/`; the sources are written here, the very bytes the indexers read.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use timonier_core::{EdgeKind, Provenance};
use timonier_index::scip::{self, Indexed, SCIP_KINDS};
use timonier_index::{Indexer, Packs};

const RUST: &[(&str, &str)] = &[
    (
        "Cargo.toml",
        r#"[package]
name = "shapes"
version = "0.1.0"
edition = "2021"

[workspace]
"#,
    ),
    (
        "src/lib.rs",
        r#"pub mod area;
pub mod shape;
"#,
    ),
    (
        "src/shape.rs",
        r#"pub trait Shape {
    fn area(&self) -> f64;
}

pub struct Square {
    pub side: f64,
}

pub fn unit() -> Square {
    Square { side: 1.0 }
}
"#,
    ),
    (
        "src/area.rs",
        r#"use crate::shape::{unit, Shape, Square};

impl Shape for Square {
    fn area(&self) -> f64 {
        self.side * self.side
    }
}

pub fn total() -> f64 {
    unit().area()
}
"#,
    ),
];

const TYPESCRIPT: &[(&str, &str)] = &[
    (
        "package.json",
        r#"{ "name": "shapes", "version": "0.1.0", "private": true }
"#,
    ),
    (
        "tsconfig.json",
        r#"{ "compilerOptions": { "strict": true, "target": "es2022", "module": "nodenext" }, "include": ["src"] }
"#,
    ),
    (
        "src/shape.ts",
        r#"export interface Shape {
  area(): number;
}

export class Square implements Shape {
  constructor(public side: number) {}
  area(): number {
    return this.side * this.side;
  }
}

export function unit(): Square {
  return new Square(1);
}
"#,
    ),
    (
        "src/total.ts",
        r#"import { unit, type Shape } from "./shape.js";

export function total(shape: Shape = unit()): number {
  return shape.area();
}
"#,
    ),
];

const PYTHON: &[(&str, &str)] = &[
    (
        "pyproject.toml",
        r#"[project]
name = "shapes"
version = "0.1.0"
"#,
    ),
    ("shapes/__init__.py", r#""#),
    (
        "shapes/shape.py",
        r#"class Shape:
    def area(self) -> float:
        raise NotImplementedError


class Square(Shape):
    def __init__(self, side: float) -> None:
        self.side = side

    def area(self) -> float:
        return self.side * self.side


def unit() -> Square:
    return Square(1.0)
"#,
    ),
    (
        "shapes/total.py",
        r#"from shapes.shape import Square, unit


class Big(Square):
    pass


def total() -> float:
    return unit().area()
"#,
    ),
];
/// The kinds each file verifies, as names.
type Verified = BTreeSet<Vec<String>>;
/// An edge: `(kind, from, to)`.
type Edge = (String, String, String);

/// Writes `sources`, indexes them, reads `index` into observations: the kinds each file
/// verifies, and the edges `(kind, from, to)`.
fn read(sources: &[(&str, &str)], index: &str) -> (Verified, BTreeSet<Edge>) {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    for (path, text) in sources {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("dirs");
        std::fs::write(path, text).expect("write");
    }
    let mut indexer = Indexer::new(root, Packs::builtin().expect("packs")).expect("indexer");
    indexer
        .index_all(&HashMap::new(), &[], None)
        .expect("index");
    let project = indexer.project();
    let bytes = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(index)).expect("index");
    let mut read = scip::read(
        &bytes,
        |path| {
            project.file(path).map(|f| Indexed {
                id: f.id.clone(),
                content_hash: f.content_hash,
            })
        },
        |path| std::fs::read(root.join(path)).ok(),
        |path, at| project.symbol_at(path, at),
    )
    .expect("read");
    // Tree-sitter's imports the index confirms become `Scip` edges (ADR 0073).
    let paths: BTreeSet<String> = project.files().map(|f| f.path.clone()).collect();
    let confirmed = scip::confirm_imports(&mut read, &project.file_imports(&paths));
    assert!(confirmed > 0, "no import confirmed");
    let mut verified = BTreeSet::new();
    let mut edges = BTreeSet::new();
    for update in &read.updates {
        let observations = update.observations.as_ref().expect("observations");
        assert_eq!(observations.source, Provenance::Scip);
        let file = project
            .files()
            .find(|f| f.id.as_str() == update.scope.as_str())
            .expect("an indexed file");
        // Anchored on the content the indexer read (ADR 0009).
        assert_eq!(observations.computed_on, Some(file.content_hash));
        verified.insert(
            observations
                .verified
                .iter()
                .map(ToString::to_string)
                .collect(),
        );
        for o in &observations.observations {
            edges.insert((o.kind.to_string(), o.src.to_string(), o.dst.to_string()));
        }
    }
    (verified, edges)
}

fn edge(kind: EdgeKind, from: &str, to: &str) -> Edge {
    (kind.to_string(), from.to_owned(), to.to_owned())
}

fn kinds(list: &[EdgeKind]) -> Verified {
    BTreeSet::from([list.iter().map(ToString::to_string).collect()])
}

/// rust-analyzer: calls between symbols, a trait method's impl included; references between
/// files, `mod` declarations included. It records no relationship: tree-sitter keeps
/// `Implements` and `Inherits`.
#[test]
fn rust_analyzer_s_index() {
    let (verified, edges) = read(RUST, "tests/fixtures/scip/rust.scip");
    assert_eq!(verified, kinds(&SCIP_KINDS[..2]));
    for expected in [
        edge(
            EdgeKind::Calls,
            "rust:shapes::area::total",
            "rust:shapes::shape::unit",
        ),
        edge(
            EdgeKind::Calls,
            "rust:shapes::area::total",
            "rust:shapes::area::<Square as Shape>::area",
        ),
        edge(
            EdgeKind::References,
            "file:src/area.rs",
            "file:src/shape.rs",
        ),
        edge(EdgeKind::References, "file:src/lib.rs", "file:src/area.rs"),
    ] {
        assert!(edges.contains(&expected), "{expected:?} in {edges:#?}");
    }
}

/// scip-typescript: UTF-16 columns, calls through an interface, `implements` between types,
/// and no edge between a method and the one it implements.
#[test]
fn scip_typescript_s_index() {
    let (verified, edges) = read(TYPESCRIPT, "tests/fixtures/scip/typescript.scip");
    assert_eq!(verified, kinds(&SCIP_KINDS));
    for expected in [
        edge(EdgeKind::Imports, "file:src/total.ts", "file:src/shape.ts"),
        edge(
            EdgeKind::Calls,
            "typescript:shapes/src/total#total",
            "typescript:shapes/src/shape#unit",
        ),
        edge(
            EdgeKind::Calls,
            "typescript:shapes/src/total#total",
            "typescript:shapes/src/shape#Shape.area",
        ),
        edge(
            EdgeKind::Implements,
            "typescript:shapes/src/shape#Square",
            "typescript:shapes/src/shape#Shape",
        ),
        edge(
            EdgeKind::References,
            "file:src/total.ts",
            "file:src/shape.ts",
        ),
    ] {
        assert!(edges.contains(&expected), "{expected:?} in {edges:#?}");
    }
    assert!(
        !edges
            .iter()
            .any(|(_, from, _)| from.ends_with("Square.area")),
        "{edges:#?}"
    );
}

/// scip-python: calls across modules, and class inheritance within a file and across files.
#[test]
fn scip_python_s_index() {
    let (verified, edges) = read(PYTHON, "tests/fixtures/scip/python.scip");
    assert_eq!(verified, kinds(&SCIP_KINDS));
    for expected in [
        edge(
            EdgeKind::Imports,
            "file:shapes/total.py",
            "file:shapes/shape.py",
        ),
        edge(
            EdgeKind::Calls,
            "python:shapes.total#total",
            "python:shapes.shape#unit",
        ),
        edge(
            EdgeKind::Calls,
            "python:shapes.total#total",
            "python:shapes.shape#Square.area",
        ),
        edge(
            EdgeKind::Inherits,
            "python:shapes.shape#Square",
            "python:shapes.shape#Shape",
        ),
        edge(
            EdgeKind::Inherits,
            "python:shapes.total#Big",
            "python:shapes.shape#Square",
        ),
        edge(
            EdgeKind::References,
            "file:shapes/total.py",
            "file:shapes/shape.py",
        ),
    ] {
        assert!(edges.contains(&expected), "{expected:?} in {edges:#?}");
    }
}

/// The completeness check (ADR 0073): the share of tree-sitter's links the index confirms.
#[test]
fn an_index_that_confirms_too_few_links_is_incomplete() {
    let link = |a: &str, b: &str| (a.to_owned(), b.to_owned());
    let tree_sitter = BTreeSet::from([
        link("a", "b"),
        link("a", "c"),
        link("b", "c"),
        link("c", "d"),
    ]);
    let full = BTreeSet::from([
        link("a", "b"),
        link("a", "c"),
        link("b", "c"),
        link("c", "e"),
    ]);
    let half = BTreeSet::from([link("a", "b"), link("b", "c")]);
    assert_eq!(scip::confirmed_share(&full, &tree_sitter), Some(0.75));
    assert!(scip::confirmed_share(&full, &tree_sitter).unwrap() >= scip::COMPLETE_ENOUGH);
    assert!(scip::confirmed_share(&half, &tree_sitter).unwrap() < scip::COMPLETE_ENOUGH);
    assert_eq!(scip::confirmed_share(&half, &BTreeSet::new()), None);
}

/// A file that is not an index is an error, not a panic.
#[test]
fn a_broken_index_is_an_error() {
    let error = scip::read(b"not a scip index at all", |_| None, |_| None, |_, _| None);
    assert!(error.is_err());
}

/// Review of Phase 11, I4: an occurrence placed by `typed_range` as by `range`, and the
/// columns of UTF-16 and UTF-32 indexes, on a line with characters outside ASCII, become the
/// UTF-8 bytes the graph uses (ADR 0008).
#[test]
fn typed_ranges_and_utf16_or_utf32_columns_become_utf8_bytes() {
    use ::scip::types::{
        Document, Index, Occurrence, PositionEncoding, SingleLineRange, occurrence::Typed_range,
    };
    use protobuf::Message;
    use timonier_core::{ContentHash, NodeId, Position};

    let shape = "export function unit() {}\n";
    let total = "/* é𝄞 */ unit();\n";
    let symbol = "scip-typescript npm demo 1.0.0 src/`shape.ts`/unit().";
    // `unit` in `total.ts`: 10 UTF-16 units, 9 UTF-32 ones, 13 bytes from the line's start.
    let bytes = u32::try_from(total.find("unit").expect("a reference")).expect("small");
    for (encoding, column) in [
        (PositionEncoding::UTF16CodeUnitOffsetFromLineStart, 10),
        (PositionEncoding::UTF32CodeUnitOffsetFromLineStart, 9),
    ] {
        let definition = Occurrence {
            symbol: symbol.to_owned(),
            symbol_roles: 1,
            range: vec![0, 16, 20],
            ..Occurrence::default()
        };
        let reference = Occurrence {
            symbol: symbol.to_owned(),
            typed_range: Some(Typed_range::SingleLineRange(SingleLineRange {
                line: 0,
                start_character: column,
                end_character: column + 4,
                ..SingleLineRange::default()
            })),
            ..Occurrence::default()
        };
        let document = |path: &str, occurrences: Vec<Occurrence>| Document {
            relative_path: path.to_owned(),
            occurrences,
            position_encoding: encoding.into(),
            ..Document::default()
        };
        let index = Index {
            documents: vec![
                document("src/shape.ts", vec![definition]),
                document("src/total.ts", vec![reference]),
            ],
            ..Index::default()
        };
        let mut seen = Vec::new();
        let read = scip::read(
            &index.write_to_bytes().expect("an index"),
            |path| {
                Some(Indexed {
                    id: NodeId::file(path).ok()?,
                    content_hash: ContentHash::from_bytes([7; 32]),
                })
            },
            |path| Some(if path == "src/shape.ts" { shape } else { total }.into()),
            |path, at| {
                seen.push((path.to_owned(), at));
                NodeId::parse(if path == "src/shape.ts" {
                    "typescript:demo/src/shape#unit".to_owned()
                } else {
                    "typescript:demo/src/total#main".to_owned()
                })
                .ok()
            },
        )
        .expect("read");
        assert!(
            seen.contains(&("src/total.ts".to_owned(), Position::new(0, bytes))),
            "{encoding:?}: {seen:?}"
        );
        // `range`, without `typed_range`, places the definition: `unit` at byte 16.
        assert!(
            seen.contains(&("src/shape.ts".to_owned(), Position::new(0, 16))),
            "{seen:?}"
        );
        assert!(
            read.links
                .contains(&("src/total.ts".to_owned(), "src/shape.ts".to_owned()))
        );
    }
}

/// PLAN.md 11.1, review of Phase 11 (I9): tree-sitter's file links against an index's, on the
/// files the index read; a link to a folder's `mod.rs` stands, leniently, for the index's
/// links to that folder's files.
#[test]
fn tree_sitter_s_links_are_compared_with_the_index_s() {
    use timonier_core::{NodeId, ScopeKey, ScopeUpdate};
    let link = |a: &str, b: &str| (a.to_owned(), b.to_owned());
    let mut read = scip::Read::default();
    for path in ["src/lib.rs", "src/a/mod.rs", "src/a/x.rs", "src/b.rs"] {
        let id = NodeId::file(path).expect("a file");
        read.updates
            .push(ScopeUpdate::new(ScopeKey::of_file(&id).expect("a scope")));
    }
    read.links = [
        link("src/lib.rs", "src/a/x.rs"),
        link("src/lib.rs", "src/b.rs"),
        link("src/b.rs", "src/a/x.rs"),
        // A file the index read alone: out of the comparison.
        link("src/b.rs", "build.rs"),
    ]
    .into();
    let tree_sitter = [
        link("src/lib.rs", "src/a/mod.rs"),
        link("src/lib.rs", "src/b.rs"),
        link("src/a/x.rs", "src/b.rs"),
    ]
    .into();
    let compared = scip::compare(&read, &tree_sitter);
    assert_eq!(
        (
            compared.files,
            compared.index_links,
            compared.tree_sitter_links
        ),
        (4, 3, 3)
    );
    assert!((compared.precision - 1.0 / 3.0).abs() < 1e-9);
    assert!((compared.recall - 1.0 / 3.0).abs() < 1e-9);
    assert!((compared.lenient_precision - 2.0 / 3.0).abs() < 1e-9);
    assert!((compared.lenient_recall - 2.0 / 3.0).abs() < 1e-9);
}
