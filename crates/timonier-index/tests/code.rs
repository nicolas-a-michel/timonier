//! The packs' `[code]` rules (ADR 0049, 0077): what each language counts as public, and as test
//! code, for the instrumentation gaps.

use timonier_core::gaps::{self, Callable};
use timonier_index::Packs;

fn callable(id: &str, signature: &str, file: &str) -> Callable {
    Callable {
        id: id.to_owned(),
        name: id
            .rsplit([':', '#', '.'])
            .next()
            .expect("a name")
            .to_owned(),
        file: Some(file.to_owned()),
        line: Some(4),
        signature: Some(signature.to_owned()),
    }
}

#[test]
fn each_language_s_pack_says_what_is_public_and_what_is_test_code() {
    let rules = Packs::code_rules().expect("the packs' rules");
    let public = |id: &str, signature: &str, file: &str| {
        gaps::is_public(&callable(id, signature, file), &rules)
    };
    assert!(public("rust:demo::f", "pub fn f()", "src/lib.rs"));
    assert!(public(
        "rust:demo::Thing::g",
        "pub(crate) fn g(&self)",
        "src/lib.rs"
    ));
    assert!(!public("rust:demo::p", "fn p()", "src/lib.rs"));
    // T-120: JavaScript as TypeScript; a `#private` method is not a boundary.
    assert!(public(
        "javascript:shop/src/cart#total",
        "function total(items)",
        "src/cart.js"
    ));
    assert!(!public(
        "javascript:shop/src/cart#Cart.#sum",
        "#sum(a)",
        "src/cart.js"
    ));
    // T-121: Python by its leading underscore; `test_` files are test code.
    assert!(public(
        "python:shop.cart#total",
        "def total(items):",
        "src/shop/cart.py"
    ));
    assert!(!public(
        "python:shop.cart#_round",
        "def _round(x):",
        "src/shop/cart.py"
    ));
    assert!(!public(
        "python:test_cart#test_total",
        "def test_total():",
        "test_cart.py"
    ));
    // T-128: Swift by `public` or `open`.
    assert!(public(
        "swift:Shop/Cart#Cart.add",
        "public func add(_ item: Item)",
        "Sources/Shop/Cart.swift"
    ));
    assert!(!public(
        "swift:Shop/Cart#Cart.make",
        "static func make() -> Cart",
        "Sources/Shop/Cart.swift"
    ));
    // T-127: Kotlin but `private`, `internal` and `protected`.
    assert!(public(
        "kotlin:com.shop.Cart#Cart.total",
        "fun total(): Int",
        "src/main/kotlin/com/shop/Cart.kt"
    ));
    assert!(!public(
        "kotlin:com.shop.Cart#Cart.helper",
        "private fun helper(): Int",
        "src/main/kotlin/com/shop/Cart.kt"
    ));
    // T-129: C as C++: but `static` functions; `_test.c` files are test code.
    assert!(public(
        "c:src/cart.c#cart_total",
        "int cart_total(const struct cart * c)",
        "src/cart.c"
    ));
    assert!(!public(
        "c:src/cart.c#round_up",
        "static int round_up(int x)",
        "src/cart.c"
    ));
    assert!(!public(
        "c:tests/cart_test.c#check",
        "void check(void)",
        "tests/cart_test.c"
    ));
    // T-124: C++ but `static` functions; `_test.cpp` files are test code.
    assert!(public(
        "cpp:src/cart.cpp#shop::Cart::total",
        "int Cart::total() const",
        "src/cart.cpp"
    ));
    assert!(!public(
        "cpp:src/cart.cpp#helper",
        "static int helper()",
        "src/cart.cpp"
    ));
    assert!(!public(
        "cpp:src/cart_test.cpp#check",
        "void check()",
        "src/cart_test.cpp"
    ));
    // T-130: a .NET test project is test code, its methods public as they are.
    assert!(!public(
        "csharp:Shop.Tests/CartTests#CartTests.Total",
        "public void Total()",
        "tests/Shop.Tests/CartTests.cs"
    ));
    assert!(!public(
        "csharp:Shop.Tests/A#A.B",
        "public void B()",
        "src/Shop.Tests/A.cs"
    ));
    assert!(public(
        "csharp:Shop/Cart#Cart.Total",
        "public int Total()",
        "src/Shop/Cart.cs"
    ));
    // T-123: Java by `public`; `src/test/` is test code.
    assert!(public(
        "java:com.shop.Cart#Cart.total",
        "@Override public int total()",
        "src/main/java/com/shop/Cart.java"
    ));
    assert!(!public(
        "java:com.shop.Cart#Cart.helper",
        "private int helper()",
        "src/main/java/com/shop/Cart.java"
    ));
    assert!(!public(
        "java:com.shop.CartTest#CartTest.total",
        "public void total()",
        "src/test/java/com/shop/CartTest.java"
    ));
    // T-122: Go by its capital letter; `_test.go` files are test code.
    assert!(public(
        "go:example.com/shop/cart/cart#Cart.Total",
        "func(c *Cart) Total() int",
        "cart/cart.go"
    ));
    assert!(!public(
        "go:example.com/shop/cart/cart#round",
        "func round(x int) int",
        "cart/cart.go"
    ));
    assert!(!public(
        "go:example.com/shop/cart/cart_test#TestTotal",
        "func TestTotal(t *testing.T)",
        "cart/cart_test.go"
    ));
    assert!(!public(
        "rust:demo::tests::helper",
        "pub fn helper()",
        "src/lib.rs"
    ));
    assert!(!public(
        "rust:demo::@tests::it::run",
        "pub fn run()",
        "tests/it.rs"
    ));
    assert!(public(
        "typescript:app/src/a#draw",
        "function draw()",
        "src/a.ts"
    ));
    assert!(!public(
        "typescript:app/src/a#A.hide",
        "private hide()",
        "src/a.ts"
    ));
    assert!(!public(
        "typescript:app/src/a.test#t",
        "function t()",
        "src/a.test.ts"
    ));
    assert!(!public(
        "typescript:app/test/x#t",
        "function t()",
        "test/x.ts"
    ));
}

#[test]
fn typescript_and_javascript_say_what_they_approximate() {
    let rules = Packs::code_rules().expect("the packs' rules");
    let said: Vec<&str> = rules
        .languages
        .iter()
        .filter_map(|(_, code)| code.approximation.as_deref())
        .collect();
    assert_eq!(said.len(), 2, "{said:?}");
    assert!(said[0].starts_with("TypeScript:"));
    assert!(said[1].starts_with("JavaScript:"));
    // Every language of the packs has its rule of public code.
    for (language, code) in &rules.languages {
        assert!(code.public.is_some(), "{language} has no [code.public]");
    }
}
