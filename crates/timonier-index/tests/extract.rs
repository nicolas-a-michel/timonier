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
            "{:<10} {} ×{} at {}:{}{source}{receiver}",
            reference.kind,
            reference.path.join("·"),
            reference.count,
            reference.at.line,
            reference.at.column
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

/// A server is seen in the code, not in its imports: a client of `node:net` is not one.
#[test]
fn a_typescript_server_is_seen_in_the_code() {
    let servers = |path: &str, source: &str| -> u32 {
        extract(path, source)
            .signals
            .iter()
            .filter(|s| s.role == timonier_core::Role::EntryPoint)
            .map(|s| s.count)
            .sum()
    };
    let client = "import { createConnection } from \"node:net\";\nexport const s = createConnection(\"/run/a.sock\");\n";
    assert_eq!(servers("src/client.ts", client), 0);
    let server = "import { createServer } from \"node:http\";\ncreateServer(() => {});\n";
    assert!(servers("src/server.ts", server) > 0);
    let member = "import * as net from \"node:net\";\nnet.createServer();\n";
    assert!(servers("src/net.ts", member) > 0);
}

/// ADR 0022: the language server is asked at the name of each path, wherever it sits in the
/// cited node: a generic trait carries its arguments after its name.
#[test]
fn a_path_is_asked_about_at_its_name() {
    let source = "pub trait Tr<T> {}\npub struct S;\npub struct Q;\nimpl Tr<Q> for S {}\nimpl crate::a::Long<Q, Q> for S {}\npub fn f(x: Option<Q>) {}\n";
    let facts = extract("src/lib.rs", source);
    let lines: Vec<&str> = source.lines().collect();
    let named = |kind: timonier_core::EdgeKind, path: &[&str]| {
        let fact = facts
            .references
            .iter()
            .find(|r| r.kind == kind && r.path == path)
            .unwrap_or_else(|| panic!("no {kind} {path:?}"));
        let line = lines[fact.at.line as usize];
        let column = fact.at.column as usize;
        line[column..].to_owned()
    };
    use timonier_core::EdgeKind::{Implements, References};
    assert!(named(Implements, &["Tr"]).starts_with("Tr<Q>"));
    assert!(named(Implements, &["crate", "a", "Long"]).starts_with("Long<Q, Q>"));
    assert!(named(References, &["Option"]).starts_with("Option<Q>"));
}

const JS: &str = r#"import { a, b as c } from "./host.js";
import * as ns from "../x";
import React from "react";
export { z } from "./z.mjs";
const { readFile, join: j } = require("node:fs");
const lodash = require("lodash");

export const K = 1;
const f = (x) => x + 1;
var legacy = function (y) {
  return y;
};

export function g(a) {
  h(a);
  ns.k();
  new Foo();
}

export default class Foo extends Bar {
  x = 1;
  m(a) {
    return this.n(a);
  }
}

function* gen() {
  yield 1;
}

export function App() {
  return <Panel title="t" />;
}
"#;

/// T-120: ES modules and CommonJS, classes, fields, generators and JSX, in one JavaScript file.
#[test]
fn javascript_fixture() {
    insta::assert_snapshot!(render(&extract("src/host.jsx", JS)));
}

/// T-120: a reformatting (quotes, semicolons, line breaks) keeps every identity, signature and
/// body hash of a JavaScript file.
#[test]
fn reformatting_javascript_changes_no_identity() {
    let reformatted = JS
        .replace('"', "'")
        .replace(";\n", "\n")
        .replace("export function g(a) {", "export function g(\n  a,\n) {");
    let identity = |facts: &FileFacts| -> Vec<(String, String, String)> {
        facts
            .symbols
            .iter()
            .map(|s| (s.path.join("/"), s.signature.clone(), s.body_hash.to_hex()))
            .collect()
    };
    assert_eq!(
        identity(&extract("src/host.js", &reformatted)),
        identity(&extract("src/host.js", JS))
    );
}

/// T-120: a JavaScript server is seen in the code, as a TypeScript one is.
#[test]
fn a_javascript_server_is_seen_in_the_code() {
    let servers = |source: &str| -> u32 {
        extract("src/server.js", source)
            .signals
            .iter()
            .filter(|s| s.role == timonier_core::Role::EntryPoint)
            .map(|s| s.count)
            .sum()
    };
    assert_eq!(
        servers("const net = require(\"node:net\");\nnet.connect(1);\n"),
        0
    );
    assert!(
        servers("const http = require(\"node:http\");\nhttp.createServer().listen(8080);\n") > 0
    );
}
