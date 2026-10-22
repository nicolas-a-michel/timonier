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

const PY: &str = r#"import os
import shop.prices
import numpy as np
from . import cart
from .models import Item, Basket as B
from ..util import helper

TAX = 0.2


@dataclass
class Order(Base, models.Mixin):
    total: int = 0

    def add(self, item: Item) -> None:
        self.items.append(item)
        helper(item)

    @property
    def count(self):
        return len(self.items)


def checkout(order):
    shop.prices.price_of("apple")
    return Order()


if __name__ == "__main__":
    checkout(None)
"#;

/// T-121: modules, classes, methods (decorated or not), fields, constants, imports absolute and
/// relative, calls and heritage, in one Python file.
#[test]
fn python_fixture() {
    insta::assert_snapshot!(render(&extract("src/shop/orders.py", PY)));
}

/// T-121: a reformatting (quotes, blank lines, a call broken over lines) keeps every identity,
/// signature and body hash of a Python file.
#[test]
fn reformatting_python_changes_no_identity() {
    let reformatted = PY
        .replace('"', "'")
        .replace("\n\n\ndef checkout", "\n\ndef checkout")
        .replace(
            "    shop.prices.price_of(\"apple\")",
            "    shop.prices.price_of(\n        'apple',\n    )",
        );
    let identity = |facts: &FileFacts| -> Vec<(String, String, String)> {
        facts
            .symbols
            .iter()
            .map(|s| (s.path.join("/"), s.signature.clone(), s.body_hash.to_hex()))
            .collect()
    };
    assert_eq!(
        identity(&extract("src/shop/orders.py", &reformatted)),
        identity(&extract("src/shop/orders.py", PY))
    );
}

const GO: &str = r#"package cart

import (
	"fmt"
	"net/http"
	str "strings"
	_ "embed"

	"example.com/shop/prices"
)

const Tax = 0.2

var (
	Default = New()
	limit   int
)

// Cart holds the items.
type Cart struct {
	prices.Store
	Items []Item
	total int
}

type Item struct {
	Name  string `json:"name"`
	Price int
}

type Shape interface {
	fmt.Stringer
	Area() float64
}

type ID = string

func New() *Cart {
	return &Cart{}
}

func (c *Cart) Add(item Item) {
	c.Items = append(c.Items, item)
	c.total += prices.PriceOf(item.Name)
}

func (c Cart) Total() int {
	return c.total
}

func main() {
	go fmt.Println(str.ToUpper("x"))
	http.ListenAndServe(":8080", nil)
}
"#;

/// T-122: package-level constants and variables, structs with embedded and named fields,
/// interfaces, aliases, functions, methods ranged under their receiver's type, imports with an
/// alias or none, calls and embedding, in one Go file.
#[test]
fn go_fixture() {
    insta::assert_snapshot!(render(&extract("cart/cart.go", GO)));
}

/// T-122: a reformatting (a single import, spaces, a call broken over lines) keeps every
/// identity, signature and body hash of a Go file.
#[test]
fn reformatting_go_changes_no_identity() {
    let reformatted = GO
        .replace("func New() *Cart {", "func New()  *Cart{")
        .replace(
            "c.total += prices.PriceOf(item.Name)",
            "c.total += prices.PriceOf(\n\t\titem.Name,\n\t)",
        )
        .replace(
            "var (\n\tDefault = New()\n\tlimit   int\n)",
            "var (\n\tDefault = New()\n\tlimit int\n)",
        );
    let identity = |facts: &FileFacts| -> Vec<(String, String, String)> {
        facts
            .symbols
            .iter()
            .map(|s| (s.path.join("/"), s.signature.clone(), s.body_hash.to_hex()))
            .collect()
    };
    assert_eq!(
        identity(&extract("cart/cart.go", &reformatted)),
        identity(&extract("cart/cart.go", GO))
    );
}

/// T-122: a Go method declared before its type, or in a file without it, keeps its receiver's
/// name in its path.
#[test]
fn a_go_method_is_named_by_its_receiver() {
    let paths = |source: &str| -> Vec<String> {
        extract("cart/cart.go", source)
            .symbols
            .iter()
            .map(|s| s.path.join("/"))
            .collect()
    };
    assert_eq!(
        paths("package cart\nfunc (c *Cart) Total() int { return 0 }\ntype Cart struct{}\n"),
        ["Cart/Total", "Cart"]
    );
    assert_eq!(
        paths("package cart\nfunc (c *Cart[T]) Total() int { return 0 }\n"),
        ["Cart/Total"]
    );
}

const JAVA: &str = r#"package com.shop;

import java.util.List;
import java.util.concurrent.*;
import static com.shop.Prices.priceOf;
import com.shop.model.Item;

@Service
public class Cart extends Base implements Comparable<Cart>, java.io.Serializable {
    public static final double TAX = 0.2;
    private final List<Item> items = new ArrayList<>();

    public Cart() {}

    public void add(Item item) {
        items.add(item);
        Prices.check(item);
    }

    public int total() {
        return priceOf("apple") + helper();
    }

    public int total(int extra) {
        return total() + extra;
    }

    private int helper() {
        return 0;
    }

    enum Kind { SMALL, LARGE }

    record Line(String name, int price) {}

    public static void main(String[] args) {
        new Thread(() -> {}).start();
    }
}

interface Shape extends Comparable<Shape> {
    double area();
}
"#;

/// T-123: classes, records, enums and interfaces, fields, constructors, overloads, imports of a
/// class, a package and a static member, calls, creations, annotations and heritage, in one
/// Java file.
#[test]
fn java_fixture() {
    insta::assert_snapshot!(render(&extract("src/main/java/com/shop/Cart.java", JAVA)));
}

/// T-123: a reformatting (line breaks, spaces) keeps every identity, signature and body hash of
/// a Java file.
#[test]
fn reformatting_java_changes_no_identity() {
    let reformatted = JAVA
        .replace(
            "public void add(Item item) {",
            "public void add(\n            Item item) {",
        )
        .replace(
            "return priceOf(\"apple\") + helper();",
            "return priceOf( \"apple\" )\n            + helper();",
        )
        .replace(
            "enum Kind { SMALL, LARGE }",
            "enum Kind {\n        SMALL,\n        LARGE\n    }",
        );
    let identity = |facts: &FileFacts| -> Vec<(String, String, String)> {
        facts
            .symbols
            .iter()
            .map(|s| (s.path.join("/"), s.signature.clone(), s.body_hash.to_hex()))
            .collect()
    };
    assert_eq!(
        identity(&extract("src/main/java/com/shop/Cart.java", &reformatted)),
        identity(&extract("src/main/java/com/shop/Cart.java", JAVA))
    );
}

const CPP: &str = r#"#include "cart.hpp"
#include "model/item.hpp"
#include <fmt/core.h>
#include <thread>
#include <vector>

namespace shop {

constexpr double kTax = 0.2;

enum class Kind { Small, Large };

using Prices = std::vector<int>;

struct Line {
    std::string name;
    int price;
};

class Cart : public Base, public Printable<Cart> {
public:
    Cart() = default;
    void add(const Item& item);
    int total() const { return helper() + sum(prices_); }
    static Cart* make();

private:
    int helper() const;
    Prices prices_;
};

int Cart::helper() const {
    return fmt::format("{}", 1).size();
}

Cart* Cart::make() {
    return new Cart();
}

int price(const Item& item);

}  // namespace shop

int main() {
    shop::Cart cart;
    cart.add(Item{});
    std::thread worker([] { shop::price(Item{}); });
    worker.join();
    return shop::Cart::make()->total();
}
"#;

/// T-124: namespaces, classes and structs, enums, aliases, methods defined in and out of their
/// class, declarations, fields, includes local and of the system, calls qualified or not, and
/// heritage, in one C++ file.
#[test]
fn cpp_fixture() {
    insta::assert_snapshot!(render(&extract("src/cart.cpp", CPP)));
}

/// T-124: a reformatting (braces, spaces, line breaks) keeps every identity, signature and body
/// hash of a C++ file.
#[test]
fn reformatting_cpp_changes_no_identity() {
    let reformatted = CPP
        .replace("int Cart::helper() const {", "int Cart::helper() const\n{")
        .replace(
            "enum class Kind { Small, Large };",
            "enum class Kind {\n    Small,\n    Large,\n};",
        )
        .replace("return new Cart();", "return new Cart( );");
    let identity = |facts: &FileFacts| -> Vec<(String, String, String)> {
        facts
            .symbols
            .iter()
            .map(|s| (s.path.join("/"), s.signature.clone(), s.body_hash.to_hex()))
            .collect()
    };
    assert_eq!(
        identity(&extract("src/cart.cpp", &reformatted)),
        identity(&extract("src/cart.cpp", CPP))
    );
}

const KOTLIN: &str = r#"package com.shop

import com.shop.model.Item
import com.shop.util.*
import java.time.Instant as Now

const val TAX = 0.2

class Cart(val owner: String, count: Int) : Base(), Printable<Cart> {
    private val items = mutableListOf<Item>()

    fun add(item: Item) {
        items.add(item)
        log(item.name)
    }

    fun total(): Int = items.sumOf { it.price } + helper()

    companion object {
        fun create() = Cart("a", 1)
    }
}

interface Shape : Comparable<Shape> {
    fun area(): Double
}

data class Line(val name: String, val price: Int)

object Registry {
    val carts = listOf<Cart>()
}

typealias Prices = List<Int>

enum class Kind {
    SMALL,
    LARGE,
}

fun main() {
    Cart.create().total()
    Registry.carts.size
    thread { Now.now() }
}
"#;

/// T-127: classes, interfaces, data classes, objects and companions, aliases, enums, properties
/// of the constructor and of the body, functions of a file, imports with an alias or a `*`,
/// calls, and heritage, in one Kotlin file.
#[test]
fn kotlin_fixture() {
    insta::assert_snapshot!(render(&extract("src/main/kotlin/com/shop/Cart.kt", KOTLIN)));
}

/// T-127: a reformatting (line breaks, spaces, a trailing comma) keeps every identity,
/// signature and body hash of a Kotlin file.
#[test]
fn reformatting_kotlin_changes_no_identity() {
    let reformatted = KOTLIN
        .replace(
            "fun add(item: Item) {",
            "fun add(\n        item: Item,\n    ) {",
        )
        .replace("    SMALL,\n    LARGE,\n", "    SMALL, LARGE\n")
        .replace(
            "fun create() = Cart(\"a\", 1)",
            "fun create() = Cart( \"a\", 1 )",
        );
    let identity = |facts: &FileFacts| -> Vec<(String, String, String)> {
        facts
            .symbols
            .iter()
            .map(|s| (s.path.join("/"), s.signature.clone(), s.body_hash.to_hex()))
            .collect()
    };
    assert_eq!(
        identity(&extract("src/main/kotlin/com/shop/Cart.kt", &reformatted)),
        identity(&extract("src/main/kotlin/com/shop/Cart.kt", KOTLIN))
    );
}

const SWIFT: &str = r#"import Foundation
import Logging
@testable import Shop

public let tax = 0.2

public protocol Priced { var price: Int { get } }

public struct Item: Priced, Codable {
    public let name: String
    public var price: Int
}

public final class Cart: Base, Priced {
    private var items: [Item] = []
    public init() {}
    public func add(_ item: Item) { items.append(item) }
    public var price: Int { items.reduce(0) { $0 + $1.price } }
    static func make() -> Cart { Cart() }
}

extension Cart {
    func describe() -> String { "\(price)" }
}

enum Kind { case small, large }

typealias Prices = [Int]

func total(_ cart: Cart) -> Int {
    let c = Cart.make()
    Logger(label: "x").info("y")
    return cart.price + c.price
}
"#;

/// T-128: protocols, structures, classes, extensions ranged under their type, enumerations,
/// aliases, properties, initialisers, functions of a file, imports, calls and heritage, in one
/// Swift file.
#[test]
fn swift_fixture() {
    insta::assert_snapshot!(render(&extract("Sources/Shop/Cart.swift", SWIFT)));
}

/// T-128: a reformatting (line breaks, spaces) keeps every identity, signature and body hash
/// of a Swift file.
#[test]
fn reformatting_swift_changes_no_identity() {
    let reformatted = SWIFT
        .replace(
            "public func add(_ item: Item) { items.append(item) }",
            "public func add(\n        _ item: Item\n    ) {\n        items.append(item)\n    }",
        )
        .replace(
            "enum Kind { case small, large }",
            "enum Kind {\n    case small, large\n}",
        );
    let identity = |facts: &FileFacts| -> Vec<(String, String, String)> {
        facts
            .symbols
            .iter()
            .map(|s| (s.path.join("/"), s.signature.clone(), s.body_hash.to_hex()))
            .collect()
    };
    assert_eq!(
        identity(&extract("Sources/Shop/Cart.swift", &reformatted)),
        identity(&extract("Sources/Shop/Cart.swift", SWIFT))
    );
}

const C: &str = r#"#include "cart.h"
#include <stdlib.h>
#include <sqlite3.h>
#include <pthread.h>

#define MAX_ITEMS 16
#define SQUARE(x) ((x) * (x))

typedef struct item {
    const char *name;
    int price;
} item;

struct cart {
    item items[MAX_ITEMS];
    int count;
};

enum kind { SMALL, LARGE };

static int round_up(int x) { return x + 1; }

int cart_total(const struct cart *c) {
    int sum = 0;
    for (int i = 0; i < c->count; i++) sum += c->items[i].price;
    return round_up(sum);
}

void *worker(void *arg);

int main(void) {
    pthread_t t;
    pthread_create(&t, NULL, worker, NULL);
    struct cart *c = calloc(1, sizeof *c);
    return cart_total(c);
}
"#;

/// T-129: structures, a typedef, enumerators, macros, static and public functions, a
/// prototype, includes local and of the system, and calls, in one C file.
#[test]
fn c_fixture() {
    insta::assert_snapshot!(render(&extract("src/cart.c", C)));
}

/// T-129: a reformatting (braces, spaces) keeps every identity, signature and body hash of a
/// C file.
#[test]
fn reformatting_c_changes_no_identity() {
    let reformatted = C
        .replace(
            "static int round_up(int x) { return x + 1; }",
            "static int round_up(int x)\n{\n    return x + 1;\n}",
        )
        .replace(
            "enum kind { SMALL, LARGE };",
            "enum kind {\n    SMALL,\n    LARGE,\n};",
        );
    let identity = |facts: &FileFacts| -> Vec<(String, String, String)> {
        facts
            .symbols
            .iter()
            .map(|s| (s.path.join("/"), s.signature.clone(), s.body_hash.to_hex()))
            .collect()
    };
    assert_eq!(
        identity(&extract("src/cart.c", &reformatted)),
        identity(&extract("src/cart.c", C))
    );
}

const CSHARP: &str = r#"using System.Text.Json;
using Shop.Model;
using static Shop.Prices;

namespace Shop;

public interface IPriced { int Price { get; } }

public record Item(string Name, int Price) : IPriced;

public class Cart : Base, IPriced
{
    private readonly List<Item> _items = new();
    public const double Tax = 0.2;
    public Cart() { }
    public void Add(Item item) => _items.Add(item);
    public int Price => _items.Sum(i => i.Price);
    public static Cart Make() => new Cart();
    public enum Kind { Small, Large }
}

public struct Point { public int X; }

class App
{
    static void Main()
    {
        var c = Cart.Make();
        Console.WriteLine(JsonSerializer.Serialize(c.Price + Total(c)));
    }
}
"#;

/// T-130: a file-scoped namespace, interfaces, records and their parameters, classes, structs,
/// nested enums, fields, properties, constructors, methods, usings, calls, creations and
/// heritage, in one C# file.
#[test]
fn csharp_fixture() {
    insta::assert_snapshot!(render(&extract("src/Shop/Cart.cs", CSHARP)));
}

/// T-130: a reformatting (braces, line breaks) keeps every identity, signature and body hash of
/// a C# file.
#[test]
fn reformatting_csharp_changes_no_identity() {
    let reformatted = CSHARP
        .replace("public Cart() { }", "public Cart()\n    {\n    }")
        .replace(
            "public enum Kind { Small, Large }",
            "public enum Kind\n    {\n        Small,\n        Large,\n    }",
        );
    let identity = |facts: &FileFacts| -> Vec<(String, String, String)> {
        facts
            .symbols
            .iter()
            .map(|s| (s.path.join("/"), s.signature.clone(), s.body_hash.to_hex()))
            .collect()
    };
    assert_eq!(
        identity(&extract("src/Shop/Cart.cs", &reformatted)),
        identity(&extract("src/Shop/Cart.cs", CSHARP))
    );
}

const PHP: &str = r#"<?php

declare(strict_types=1);

namespace Shop;

use Shop\Model\Item;
use Psr\Log\LoggerInterface as Logger;

interface Priced
{
    public function price(): int;
}

enum Kind
{
    case Small;
    case Large;
}

final class Cart extends Base implements Priced
{
    public const TAX = 0.2;

    private array $items = [];

    public function __construct(private Logger $logger)
    {
    }

    public function add(Item $item): void
    {
        $this->items[] = $item;
        $this->logger->info('added');
    }

    public function price(): int
    {
        return array_sum(array_map(fn (Item $i) => $i->price, $this->items));
    }

    public static function make(): self
    {
        return new Cart(new NullLogger());
    }
}

function total(Cart $cart): int
{
    return Cart::make()->price() + $cart->price();
}
"#;

/// T-131: interfaces, enums and their cases, classes with constants, properties, promoted
/// properties and methods, functions, uses with an alias, calls, creations and heritage, in one
/// PHP file.
#[test]
fn php_fixture() {
    insta::assert_snapshot!(render(&extract("src/Cart.php", PHP)));
}

/// T-131: a reformatting (braces, line breaks) keeps every identity, signature and body hash of
/// a PHP file.
#[test]
fn reformatting_php_changes_no_identity() {
    let reformatted = PHP
        .replace(
            "    public function add(Item $item): void\n    {",
            "    public function add(\n        Item $item,\n    ): void {",
        )
        .replace("    {\n    }\n", "    {}\n");
    let identity = |facts: &FileFacts| -> Vec<(String, String, String)> {
        facts
            .symbols
            .iter()
            .map(|s| (s.path.join("/"), s.signature.clone(), s.body_hash.to_hex()))
            .collect()
    };
    assert_eq!(
        identity(&extract("src/Cart.php", &reformatted)),
        identity(&extract("src/Cart.php", PHP))
    );
}

const SCALA: &str = r#"package com.shop

import com.shop.model.{Item, Price => P}
import scala.collection.mutable
import com.shop.util.*

trait Priced:
  def price: Int

final case class Line(name: String, price: Int) extends Priced

class Cart(owner: String) extends Base with Priced:
  private val items = mutable.ListBuffer.empty[Item]
  val Tax = 0.2
  def add(item: Item): Unit = items += item
  def price: Int = items.map(_.price).sum + helper()

object Cart:
  def make(): Cart = new Cart("a")

enum Kind:
  case Small, Large

type Prices = List[Int]

def total(cart: Cart): Int = Cart.make().price + cart.price

@main def run(): Unit = println(total(Cart.make()))
"#;

/// T-132: traits, case classes and their parameters, classes, companion objects, enums, type
/// aliases, members, functions of a file, imports with selectors, a renaming and a wildcard,
/// calls and heritage, in one Scala 3 file.
#[test]
fn scala_fixture() {
    insta::assert_snapshot!(render(&extract(
        "src/main/scala/com/shop/Cart.scala",
        SCALA
    )));
}

/// T-132: a reformatting (spaces, line breaks) keeps every identity, signature and body hash of
/// a Scala file.
#[test]
fn reformatting_scala_changes_no_identity() {
    let reformatted = SCALA
        .replace(
            "def add(item: Item): Unit = items += item",
            "def add(\n      item: Item\n  ): Unit =\n    items += item",
        )
        .replace(
            "def make(): Cart = new Cart(\"a\")",
            "def make(): Cart = new Cart( \"a\" )",
        );
    let identity = |facts: &FileFacts| -> Vec<(String, String, String)> {
        facts
            .symbols
            .iter()
            .map(|s| (s.path.join("/"), s.signature.clone(), s.body_hash.to_hex()))
            .collect()
    };
    assert_eq!(
        identity(&extract("src/main/scala/com/shop/Cart.scala", &reformatted)),
        identity(&extract("src/main/scala/com/shop/Cart.scala", SCALA))
    );
}
