//! Extraction by the built-in packs: golden snapshots of what tree-sitter yields on fixtures,
//! and invariance under reformatting (ADR 0013).

use std::fmt::Write as _;

use timonier_index::{FileFacts, Packs};

/// One line per fact, stable across runs: symbols with their parent and signature, hashes
/// shortened, then imports, references and role signals.
fn render(facts: &FileFacts) -> String {
    let mut out = String::new();
    for symbol in &facts.symbols {
        let parent = symbol
            .parent
            .map_or("-".to_owned(), |p| facts.symbols[p].path.join("/"));
        let start = symbol.span.start();
        let _ = writeln!(
            out,
            "{:<8} {:<40} under {:<24} at {}:{}  `{}`  #{}",
            timonier_core::NodeKind::Symbol(symbol.kind).to_string(),
            symbol.path.join("/"),
            parent,
            start.line,
            start.column,
            symbol.signature,
            &symbol.body_hash.to_hex()[..8],
        );
    }
    for import in &facts.imports {
        let _ = writeln!(out, "import   {} binds {:?}", import.path, import.names);
    }
    for reference in &facts.references {
        let source = reference.source.map_or(String::new(), |s| {
            format!(" from {}", facts.symbols[s].path.join("/"))
        });
        let receiver = if reference.receiver {
            " on a receiver"
        } else {
            ""
        };
        let _ = writeln!(
            out,
            "{:<10} {} ×{}{source}{receiver}",
            reference.kind,
            reference.path.join("·"),
            reference.count
        );
    }
    for signal in &facts.signals {
        let _ = writeln!(out, "signal   {} ×{}", signal.role, signal.count);
    }
    let _ = writeln!(out, "errors   {}", facts.has_errors);
    out
}

fn extract(path: &str, source: &str) -> FileFacts {
    Packs::builtin()
        .expect("the built-in packs load")
        .extract(path, source.as_bytes())
        .expect("a pack reads the fixture")
}

const RUST: &str = r#"//! A module.
use crate::{node::Node, edge::{Edge, self}};
use super::store as storage;
use std::fmt::*;
use serde::{Deserialize, Serialize};

/// A type.
#[derive(Debug, Serialize)]
pub struct Foo<T> {
    pub a: u32,
    b: T,
}

pub struct Unit;

impl<T> Foo<T> {
    pub fn new(a: u32) -> Self where T: Default {
        fn helper(x: u32) -> u32 { x }
        let n = Node::new(helper(a));
        storage::save(&n);
        n.edges.push(Edge::default());
        Self { a, b: T::default() }
    }
}

impl std::fmt::Display for Foo<u8> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.a)
    }
}

impl std::fmt::Debug for Foo<u8> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { Ok(()) }
}

pub trait Shape: Clone {
    type Output;
    const SIDES: u32;
    fn area(&self) -> f64;
    fn name(&self) -> &str { "shape" }
}

mod inner {
    pub const K: u32 = 1;
    static S: u8 = 0;
    #[cfg(unix)]
    fn platform() -> u8 { 1 }
    #[cfg(windows)]
    fn platform() -> u8 { 2 }
}

mod outer;

pub enum Kind { A, B { x: u32 } }

type Alias = Vec<u8>;

fn main() {
    let handle = std::thread::spawn(|| inner::platform());
    handle.join().unwrap();
}
"#;

#[test]
fn rust_fixture() {
    insta::assert_snapshot!(render(&extract("src/lib.rs", RUST)));
}

const TS: &str = r#"import { a, b as c } from "./host";
import * as ns from "../x";
import React from "react";
import type { T } from "./t";
export { z } from "./z";

export const K = 1;
const f = (x: number): number => x + 1;

export function g(a: string): void {
  h(a);
  ns.k();
  new Foo();
}

export default class Foo extends Bar implements I, J {
  private x = 1;
  m(a: number): T {
    return this.n(a);
  }
}

interface I extends Base {
  p: string;
  q(): void;
}

type U = string | number;

enum En {
  A,
}

namespace N {
  export function inN() {}
}

function over(a: string): void;
function over(a: any) {
  return a;
}

export function activate() {
  c();
}
"#;

#[test]
fn typescript_fixture() {
    insta::assert_snapshot!(render(&extract("src/host.ts", TS)));
}

const TSX: &str = r#"import { App } from "./App";
import { createRoot } from "react-dom/client";

export function Main({ locale }: { locale: string }) {
  return <App locale={locale} />;
}

createRoot(document.body).render(<Main locale="fr"></Main>);
"#;

#[test]
fn tsx_fixture() {
    insta::assert_snapshot!(render(&extract("src/main.tsx", TSX)));
}

/// What reformatting must keep: every identity, signature and body hash.
fn identities(facts: &FileFacts) -> Vec<(String, String, String, Option<usize>)> {
    let mut ids: Vec<_> = facts
        .symbols
        .iter()
        .map(|s| {
            (
                s.path.join("/"),
                s.signature.clone(),
                s.body_hash.to_hex(),
                s.parent,
            )
        })
        .collect();
    ids.sort();
    ids
}

#[test]
fn reformatting_rust_changes_no_identity() {
    let reformatted = r#"//! A module, reformatted.
use crate::{
    edge::{self, Edge},
    node::Node,
};
use serde::{Deserialize, Serialize};
use std::fmt::*;
use super::store as storage;

/// A type.
#[derive(Debug, Serialize)]
pub struct Foo<T> { pub a: u32, b: T }

pub struct Unit;

impl<T> Foo<T>
{
    pub fn new(
        a: u32,
    ) -> Self
    where
        T: Default,
    {
        fn helper(x: u32) -> u32 {
            x // identity
        }
        let n = Node::new(helper(a));
        storage::save(&n);
        n.edges.push(Edge::default());
        Self {
            a,
            b: T::default(),
        }
    }
}

impl std::fmt::Display for Foo<u8> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.a) }
}

impl std::fmt::Debug for Foo<u8> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

pub trait Shape: Clone {
    type Output;
    const SIDES: u32;
    fn area(&self) -> f64;
    fn name(&self) -> &str {
        "shape"
    }
}

mod inner {
    pub const K: u32 = 1;
    static S: u8 = 0;
    #[cfg(unix)]
    fn platform() -> u8 {
        1
    }
    #[cfg(windows)]
    fn platform() -> u8 {
        2
    }
}

mod outer;

pub enum Kind {
    A,
    B { x: u32 },
}

type Alias = Vec<u8>;

fn main() {
    let handle = std::thread::spawn(|| inner::platform());
    handle.join().unwrap();
}
"#;
    assert_eq!(
        identities(&extract("src/lib.rs", RUST)),
        identities(&extract("src/lib.rs", reformatted))
    );
}

#[test]
fn reordering_rust_definitions_changes_no_identity() {
    let a = "struct A; impl A { fn f(&self) {} } fn g() {}";
    let b = "fn g() {} impl A { fn f(&self) {} } struct A;";
    let names = |s| {
        let mut n: Vec<String> = extract("src/lib.rs", s)
            .symbols
            .iter()
            .map(|s| s.path.join("/"))
            .collect();
        n.sort();
        n
    };
    assert_eq!(names(a), names(b));
    assert_eq!(names(a), ["A", "A/f", "g"]);
}

#[test]
fn reformatting_typescript_changes_no_identity() {
    let reformatted = r#"import { a, b as c } from './host'
import * as ns from '../x'
import React from 'react'
import type { T } from './t'
export { z } from './z'

export const K = 1
const f = (x: number): number =>
  x + 1

export function g(
  a: string,
): void {
  h(a)
  ns.k()
  new Foo()
}

export default class Foo extends Bar implements I, J {
  private x = 1
  m(a: number): T { return this.n(a) }
}

interface I extends Base { p: string; q(): void }

type U =
  | string
  | number

enum En { A }

namespace N { export function inN() {} }

function over(a: string): void
function over(a: any) { return a }

export function activate() { c() }
"#;
    assert_eq!(
        identities(&extract("src/host.ts", TS)),
        identities(&extract("src/host.ts", reformatted))
    );
}

#[test]
fn a_changed_body_changes_only_its_hash() {
    let before = extract("src/lib.rs", "fn f() -> u32 { 1 }\nfn g() {}");
    let after = extract("src/lib.rs", "fn f() -> u32 { 2 }\nfn g() {}");
    assert_eq!(before.symbols[0].signature, after.symbols[0].signature);
    assert_ne!(before.symbols[0].body_hash, after.symbols[0].body_hash);
    assert_eq!(before.symbols[1], after.symbols[1]);
}

#[test]
fn syntax_errors_are_flagged_not_fatal() {
    let facts = extract("src/lib.rs", "fn ok() {}\nfn broken( {\n");
    assert!(facts.has_errors);
    assert!(facts.symbols.iter().any(|s| s.path == ["ok"]));
}

#[test]
fn facts_round_trip_through_json() {
    let facts = extract("src/lib.rs", RUST);
    let json = serde_json::to_string(&facts).unwrap();
    assert_eq!(serde_json::from_str::<FileFacts>(&json).unwrap(), facts);
}
