//! The indexer on a temporary repository: walk, cache, refresh (ADR 0013).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use timonier_core::{NodeKind, ScopeKey, ScopeUpdate};
use timonier_index::{Batch, Cached, Indexer, Packs};

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().expect("a file has a parent")).expect("create dirs");
    std::fs::write(path, text).expect("write file");
}

/// A Cargo workspace with two crates, an npm workspace with one package, and files that must
/// not be indexed.
fn repository() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    for (path, text) in [
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
        (
            "crates/core/Cargo.toml",
            "[package]\nname = \"demo-core\"\n[dependencies]\nserde = \"1\"\n",
        ),
        (
            "crates/core/src/lib.rs",
            "pub mod node;\npub use node::Node;\n",
        ),
        (
            "crates/core/src/node.rs",
            "pub struct Node;\nimpl Node {\n    pub fn new() -> Self { Node }\n}\n",
        ),
        (
            "crates/store/Cargo.toml",
            "[package]\nname = \"demo-store\"\n[dependencies]\ndemo-core = { path = \"../core\" }\n",
        ),
        (
            "crates/store/src/lib.rs",
            "use demo_core::node::Node;\npub fn save() -> Node { Node::new() }\n",
        ),
        (
            "package.json",
            r#"{"name": "ws", "private": true, "workspaces": ["web"]}"#,
        ),
        (
            "web/package.json",
            r#"{"name": "@demo/web", "dependencies": {"react": "19"}}"#,
        ),
        (
            "web/src/main.tsx",
            "import { App } from \"./App\";\nexport const root = <App />;\n",
        ),
        (
            "web/src/App.tsx",
            "export function App() { return null; }\n",
        ),
        // Not indexed: ignored, hidden, or read by no pack.
        (".gitignore", "target/\nnode_modules/\n"),
        ("target/debug/build.rs", "fn main() {}\n"),
        (
            "web/node_modules/react/index.ts",
            "export const React = 1;\n",
        ),
        (".hidden/x.rs", "fn x() {}\n"),
        ("README.md", "# demo\n"),
    ] {
        write(root, path, text);
    }
    dir
}

fn cache_of(batch: &Batch) -> HashMap<String, Cached> {
    batch.cache_put.iter().cloned().collect()
}

fn scopes(batch: &Batch) -> Vec<String> {
    batch
        .updates
        .iter()
        .map(|u| format!("{}{}", if u.remove { "-" } else { "" }, u.scope))
        .collect()
}

fn update<'a>(batch: &'a Batch, scope: &str) -> &'a ScopeUpdate {
    batch
        .updates
        .iter()
        .find(|u| u.scope.as_str() == scope)
        .unwrap_or_else(|| panic!("no update for {scope}: {:?}", scopes(batch)))
}

fn indexer(root: &Path) -> Indexer {
    Indexer::new(root, Packs::builtin().expect("built-in packs")).expect("indexer")
}

#[test]
fn indexes_what_the_packs_read_and_the_gitignore_allows() {
    let dir = repository();
    let mut indexer = indexer(dir.path());
    let batch = indexer.index_all(&HashMap::new(), &[], None).unwrap();
    assert_eq!(
        scopes(&batch),
        [
            "structure",
            "file:crates/core/src/lib.rs",
            "file:crates/core/src/node.rs",
            "file:crates/store/src/lib.rs",
            "file:web/src/App.tsx",
            "file:web/src/main.tsx",
        ]
    );
    assert_eq!(
        (batch.stats.files, batch.stats.parsed, batch.stats.cached),
        (5, 5, 0)
    );
    let structure = update(&batch, "structure");
    let packages: Vec<String> = structure
        .nodes
        .as_ref()
        .unwrap()
        .iter()
        .filter(|n| n.kind == NodeKind::Package)
        .map(|n| n.id.to_string())
        .collect();
    assert_eq!(
        packages,
        [
            "pkg:cargo/demo-core",
            "pkg:cargo/demo-store",
            "pkg:npm/@demo/web",
            "pkg:cargo/serde",
            "pkg:npm/react"
        ]
    );
    let store = update(&batch, "file:crates/store/src/lib.rs");
    let edges: Vec<String> = store
        .observations
        .as_ref()
        .unwrap()
        .observations
        .iter()
        .map(|o| format!("{} {}", o.kind, o.dst))
        .collect();
    assert!(
        edges.contains(&"imports file:crates/core/src/node.rs".to_owned()),
        "{edges:?}"
    );
    assert!(
        edges.contains(&"calls file:crates/core/src/node.rs".to_owned()),
        "{edges:?}"
    );
}

/// Criterion of task 3.4: a second indexing without change parses no file.
#[test]
fn a_second_indexing_with_the_cache_parses_nothing() {
    let dir = repository();
    let mut whole = indexer(dir.path());
    let progress = whole.progress();
    let first = whole.index_all(&HashMap::new(), &[], None).unwrap();
    // T-115: every file read is counted, of all those found.
    use std::sync::atomic::Ordering;
    assert_eq!(
        (
            progress.done.load(Ordering::Relaxed),
            progress.total.load(Ordering::Relaxed)
        ),
        (5, 5)
    );
    let cache = cache_of(&first);
    let second = indexer(dir.path()).index_all(&cache, &[], None).unwrap();
    assert_eq!((second.stats.parsed, second.stats.cached), (0, 5));
    assert!(second.cache_put.is_empty());
    assert_eq!(first.updates, second.updates);
    // A changed file is parsed again, and only it.
    write(
        dir.path(),
        "web/src/App.tsx",
        "export function App() { return 1; }\n",
    );
    let third = indexer(dir.path()).index_all(&cache, &[], None).unwrap();
    assert_eq!((third.stats.parsed, third.stats.cached), (1, 4));
}

#[test]
fn files_deleted_while_away_are_removed_from_the_store_and_the_cache() {
    let dir = repository();
    let first = indexer(dir.path())
        .index_all(&HashMap::new(), &[], None)
        .unwrap();
    let stored: Vec<ScopeKey> = first.updates.iter().map(|u| u.scope.clone()).collect();
    std::fs::remove_file(dir.path().join("web/src/App.tsx")).unwrap();
    let second = indexer(dir.path())
        .index_all(&cache_of(&first), &stored, None)
        .unwrap();
    assert!(scopes(&second).contains(&"-file:web/src/App.tsx".to_owned()));
    assert_eq!(second.cache_remove, ["web/src/App.tsx"]);
    assert_eq!(second.stats.removed, 1);
}

#[test]
fn a_saved_file_is_reparsed_incrementally_with_the_same_result() {
    let dir = repository();
    let mut indexer = indexer(dir.path());
    indexer.index_all(&HashMap::new(), &[], None).unwrap();
    let path = "crates/core/src/node.rs";
    let mut text = std::fs::read_to_string(dir.path().join(path)).unwrap();
    for (i, addition) in [
        "pub fn a() {}\n",
        "pub fn b(x: u32) -> u32 { x }\n",
        "// note\n",
    ]
    .iter()
    .enumerate()
    {
        text.push_str(addition);
        write(dir.path(), path, &text);
        let batch = indexer.refresh(&[PathBuf::from(path)]).unwrap();
        assert_eq!(batch.stats.parsed, 1);
        // The first save after a full indexing has no tree yet.
        assert_eq!(batch.stats.incremental, usize::from(i > 0));
        let fresh = Packs::builtin()
            .unwrap()
            .extract(path, text.as_bytes())
            .unwrap();
        assert_eq!(indexer.project().file(path).unwrap().facts, fresh);
    }
    // Saving the same content again parses nothing.
    let batch = indexer.refresh(&[dir.path().join(path)]).unwrap();
    assert_eq!((batch.stats.parsed, batch.stats.unchanged), (0, 1));
    assert!(batch.updates.is_empty());
}

#[test]
fn renaming_a_definition_resolves_its_citers_again() {
    let dir = repository();
    let mut indexer = indexer(dir.path());
    indexer.index_all(&HashMap::new(), &[], None).unwrap();
    write(
        dir.path(),
        "crates/core/src/node.rs",
        "pub struct Vertex;\nimpl Vertex {\n    pub fn new() -> Self { Vertex }\n}\n",
    );
    let batch = indexer
        .refresh(&[PathBuf::from("crates/core/src/node.rs")])
        .unwrap();
    assert_eq!(batch.stats.parsed, 1);
    assert_eq!(
        scopes(&batch),
        [
            "file:crates/core/src/node.rs",
            "file:crates/core/src/lib.rs",
            "file:crates/store/src/lib.rs"
        ]
    );
    let store = update(&batch, "file:crates/store/src/lib.rs");
    assert!(
        store.nodes.is_none() && store.content_hash.is_none(),
        "edges only"
    );
}

#[test]
fn deleted_new_and_ignored_files() {
    let dir = repository();
    let mut indexer = indexer(dir.path());
    indexer.index_all(&HashMap::new(), &[], None).unwrap();
    std::fs::remove_file(dir.path().join("web/src/App.tsx")).unwrap();
    let batch = indexer
        .refresh(&[PathBuf::from("web/src/App.tsx")])
        .unwrap();
    assert_eq!(
        scopes(&batch),
        [
            "structure",
            "file:web/src/main.tsx",
            "-file:web/src/App.tsx"
        ]
    );
    assert_eq!(batch.cache_remove, ["web/src/App.tsx"]);
    // A new directory with a file.
    write(
        dir.path(),
        "web/src/views/Map.tsx",
        "export function Map() {}\n",
    );
    let batch = indexer
        .refresh(&[dir.path().join("web/src/views")])
        .unwrap();
    assert_eq!(scopes(&batch), ["structure", "file:web/src/views/Map.tsx"]);
    // Ignored and unread files change nothing.
    write(dir.path(), "target/debug/new.rs", "fn n() {}\n");
    write(dir.path(), "notes.txt", "x\n");
    let batch = indexer
        .refresh(&[
            PathBuf::from("target/debug/new.rs"),
            PathBuf::from("notes.txt"),
        ])
        .unwrap();
    assert!(batch.updates.is_empty(), "{:?}", scopes(&batch));
    // A deleted directory removes its files.
    std::fs::remove_dir_all(dir.path().join("web/src/views")).unwrap();
    let batch = indexer.refresh(&[PathBuf::from("web/src/views")]).unwrap();
    assert!(scopes(&batch).contains(&"-file:web/src/views/Map.tsx".to_owned()));
}

#[test]
fn a_manifest_change_places_every_file_again() {
    let dir = repository();
    let mut indexer = indexer(dir.path());
    indexer.index_all(&HashMap::new(), &[], None).unwrap();
    write(
        dir.path(),
        "crates/core/Cargo.toml",
        "[package]\nname = \"demo-core\"\n[lib]\nname = \"core2\"\n[dependencies]\nserde = \"1\"\n",
    );
    let batch = indexer
        .refresh(&[PathBuf::from("crates/core/Cargo.toml")])
        .unwrap();
    assert_eq!(batch.stats.parsed, 0, "contents did not change");
    let node = update(&batch, "file:crates/core/src/node.rs");
    let ids: Vec<String> = node
        .nodes
        .as_ref()
        .unwrap()
        .iter()
        .map(|n| n.id.to_string())
        .collect();
    assert_eq!(
        ids,
        ["rust:core2::node::Node", "rust:core2::node::Node::new"]
    );
}

/// ADR 0016: a file the clustering left alone stays alone; a file that arrived since joins the
/// component most of its directory neighbours are in.
#[test]
fn singletons_stay_alone_and_newcomers_join_their_neighbours() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "Cargo.toml", "[package]\nname = \"k\"\n");
    let mut lib = String::new();
    for group in 0..2 {
        for i in 0..6 {
            let name = format!("g{group}m{i}");
            lib.push_str(&format!("pub mod {name};\n"));
            let next = format!("g{group}m{}", (i + 1) % 6);
            write(
                root,
                &format!("src/{name}.rs"),
                &format!("pub fn f() {{ crate::{next}::f(); }}\n"),
            );
        }
    }
    lib.push_str("pub mod lone;\n");
    write(root, "src/lone.rs", "pub fn alone() {}\n");
    write(root, "src/lib.rs", &lib);
    let mut indexer = indexer(root);
    let batch = indexer.index_all(&HashMap::new(), &[], None).unwrap();
    let parent = |batch: &Batch, file: &str| -> String {
        update(batch, "structure")
            .nodes
            .as_ref()
            .unwrap()
            .iter()
            .find(|n| n.id.as_str() == file)
            .and_then(|n| n.parent.clone())
            .map(|p| p.to_string())
            .unwrap_or_default()
    };
    assert!(parent(&batch, "file:src/g0m0.rs").starts_with("component:cluster/"));
    assert_eq!(parent(&batch, "file:src/lone.rs"), "pkg:cargo/k");
    write(root, "src/fresh.rs", "pub fn fresh() {}\n");
    let batch = indexer.refresh(&[PathBuf::from("src/fresh.rs")]).unwrap();
    assert!(
        parent(&batch, "file:src/fresh.rs").starts_with("component:cluster/"),
        "{:?}",
        parent(&batch, "file:src/fresh.rs")
    );
    assert_eq!(parent(&batch, "file:src/lone.rs"), "pkg:cargo/k");
}

/// A root reached through a symbolic link (macOS's `/var` → `/private/var`): paths named
/// through the link, or through another link to the same place, are the same files.
#[cfg(unix)]
#[test]
fn paths_through_a_symbolic_link_name_the_same_files() {
    let dir = repository();
    let link_dir = tempfile::tempdir().unwrap();
    let link = link_dir.path().join("repo");
    std::os::unix::fs::symlink(dir.path(), &link).unwrap();
    let mut indexer = indexer(&link);
    indexer.index_all(&HashMap::new(), &[], None).unwrap();
    std::fs::remove_file(link.join("web/src/App.tsx")).unwrap();
    let batch = indexer.refresh(&[link.join("web/src/App.tsx")]).unwrap();
    assert_eq!(batch.stats.removed, 1, "{:?}", scopes(&batch));
    let other = link_dir.path().join("other");
    std::os::unix::fs::symlink(dir.path(), &other).unwrap();
    std::fs::remove_file(dir.path().join("web/src/main.tsx")).unwrap();
    let batch = indexer.refresh(&[other.join("web/src/main.tsx")]).unwrap();
    assert_eq!(batch.stats.removed, 1, "{:?}", scopes(&batch));
}

/// T-119: a whole indexing whose owner stops gives up its files, at once.
#[test]
fn a_whole_indexing_gives_up_when_cancelled() {
    let dir = repository();
    let mut indexer = indexer(dir.path());
    indexer
        .progress()
        .cancelled
        .store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(matches!(
        indexer.index_all(&HashMap::new(), &[], None),
        Err(timonier_index::Error::Cancelled)
    ));
}

/// T-120: a JavaScript package, ES modules and CommonJS, with a TypeScript file among its
/// modules: its files link to one another, its symbols carry the `javascript` language, and
/// its runtime dependency is a package.
#[test]
fn a_javascript_package_links_its_modules() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "package.json",
        "{\"name\": \"shop\", \"dependencies\": {\"express\": \"^5.0.0\"}}\n",
    );
    write(
        root,
        "src/server.js",
        "import express from \"express\";\nimport { total } from \"./cart.mjs\";\nconst { price } = require(\"./prices\");\nexport function start() {\n  express().listen(total(price(1)));\n}\n",
    );
    write(
        root,
        "src/cart.mjs",
        "export function total(x) {\n  return x;\n}\n",
    );
    write(
        root,
        "src/prices/index.js",
        "exports.price = (n) => n * 2;\nfunction price(n) {\n  return n;\n}\n",
    );
    write(root, "src/money.ts", "export const euro = 1;\n");
    write(
        root,
        "src/ui/App.jsx",
        "import { euro } from \"../money\";\nexport function App() {\n  return <span>{euro}</span>;\n}\n",
    );
    let mut indexer = indexer(root);
    let batch = indexer.index_all(&HashMap::new(), &[], None).unwrap();
    let edges = |scope: &str| -> Vec<String> {
        update(&batch, scope)
            .observations
            .as_ref()
            .unwrap()
            .observations
            .iter()
            .map(|o| format!("{} {}", o.kind, o.dst))
            .collect()
    };
    let server = edges("file:src/server.js");
    for edge in [
        "imports file:src/cart.mjs",
        "calls file:src/cart.mjs",
        "imports file:src/prices/index.js",
    ] {
        assert!(server.contains(&edge.to_owned()), "{edge}: {server:?}");
    }
    let app = edges("file:src/ui/App.jsx");
    assert!(
        app.contains(&"imports file:src/money.ts".to_owned()),
        "{app:?}"
    );
    let symbols: Vec<String> = update(&batch, "file:src/server.js")
        .nodes
        .iter()
        .flatten()
        .map(|n| n.id.as_str().to_owned())
        .collect();
    assert_eq!(symbols, ["javascript:shop/src/server#start"]);
    let structure: Vec<String> = update(&batch, "structure")
        .nodes
        .iter()
        .flatten()
        .map(|n| n.id.as_str().to_owned())
        .filter(|id| id.starts_with("pkg:"))
        .collect();
    assert!(
        structure.contains(&"pkg:npm/express".to_owned()),
        "{structure:?}"
    );
}

/// T-120: `node_modules` is never walked, even in a project that does not ignore it.
#[test]
fn installed_npm_packages_are_never_indexed() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "package.json", "{\"name\": \"app\"}\n");
    write(root, "src/a.js", "export function a() {}\n");
    write(
        root,
        "node_modules/left-pad/index.js",
        "module.exports = function () {};\n",
    );
    write(
        root,
        "packages/x/node_modules/y/index.ts",
        "export const y = 1;\n",
    );
    let batch = indexer(root).index_all(&HashMap::new(), &[], None).unwrap();
    let files: Vec<String> = scopes(&batch)
        .into_iter()
        .filter(|s| s.starts_with("file:"))
        .collect();
    assert_eq!(files, ["file:src/a.js"]);
}

/// T-121: a Python package in the src layout: its modules link to one another through absolute,
/// relative and package imports, a test outside `src/` reaches them, names are dotted paths
/// from the source root, and its runtime dependencies are packages.
#[test]
fn a_python_package_links_its_modules() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "pyproject.toml",
        "[project]\nname = \"Shop_API\"\ndependencies = [\"FastAPI>=0.115\", \"pydantic[email]>=2; python_version>'3.9'\"]\n\n[dependency-groups]\ndev = [\"pytest>=9\"]\n",
    );
    write(root, "src/shop/__init__.py", "");
    write(
        root,
        "src/shop/api.py",
        "from fastapi import FastAPI\nfrom .models import Item\nfrom shop import prices\n\napp = FastAPI()\n\n\ndef total(items):\n    return sum(prices.price_of(i) for i in items)\n",
    );
    write(
        root,
        "src/shop/models.py",
        "from pydantic import BaseModel\n\n\nclass Item(BaseModel):\n    name: str\n",
    );
    write(
        root,
        "src/shop/prices.py",
        "def price_of(name):\n    return 2\n",
    );
    write(
        root,
        "tests/test_prices.py",
        "from shop.prices import price_of\n\n\ndef test_apple():\n    assert price_of('apple') == 2\n",
    );
    write(root, ".venv/lib/x.py", "def hidden():\n    pass\n");
    write(
        root,
        "src/shop/__pycache__/api.cpython-314.py",
        "def cached():\n    pass\n",
    );
    let batch = indexer(root).index_all(&HashMap::new(), &[], None).unwrap();
    let files: Vec<String> = scopes(&batch)
        .into_iter()
        .filter(|s| s.starts_with("file:"))
        .collect();
    assert_eq!(
        files,
        [
            "file:src/shop/__init__.py",
            "file:src/shop/api.py",
            "file:src/shop/models.py",
            "file:src/shop/prices.py",
            "file:tests/test_prices.py"
        ]
    );
    let edges = |scope: &str| -> Vec<String> {
        update(&batch, scope)
            .observations
            .as_ref()
            .unwrap()
            .observations
            .iter()
            .map(|o| format!("{} {}", o.kind, o.dst))
            .collect()
    };
    let api = edges("file:src/shop/api.py");
    for edge in [
        "imports file:src/shop/models.py",
        "imports file:src/shop/prices.py",
        "calls file:src/shop/prices.py",
    ] {
        assert!(api.contains(&edge.to_owned()), "{edge}: {api:?}");
    }
    let test = edges("file:tests/test_prices.py");
    assert!(
        test.contains(&"imports file:src/shop/prices.py".to_owned()),
        "{test:?}"
    );
    let symbols: Vec<String> = update(&batch, "file:src/shop/models.py")
        .nodes
        .iter()
        .flatten()
        .map(|n| n.id.as_str().to_owned())
        .collect();
    assert_eq!(
        symbols,
        ["python:shop.models#Item", "python:shop.models#Item.name"]
    );
    let packages: Vec<String> = update(&batch, "structure")
        .nodes
        .iter()
        .flatten()
        .map(|n| n.id.as_str().to_owned())
        .filter(|id| id.starts_with("pkg:"))
        .collect();
    assert_eq!(
        packages,
        ["pkg:pypi/shop-api", "pkg:pypi/fastapi", "pkg:pypi/pydantic"]
    );
}

/// T-122: a Go module: a package's files link to another package through its import path, a
/// method cited through its package reaches the file that defines it, names are import paths
/// then the file's stem, `vendor/` is skipped, and the module's direct requirements are
/// packages (an indirect one is not).
#[test]
fn a_go_module_links_its_packages() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "go.mod",
        "module example.com/shop\n\ngo 1.27\n\nrequire (\n\tgithub.com/gin-gonic/gin v1.11.0\n\tgolang.org/x/text v0.30.0 // indirect\n)\n",
    );
    write(
        root,
        "main.go",
        "package main\n\nimport (\n\t\"fmt\"\n\n\t\"example.com/shop/cart\"\n\t\"github.com/gin-gonic/gin\"\n)\n\nfunc main() {\n\tc := cart.New()\n\tfmt.Println(c.Total())\n\tgin.Default()\n}\n",
    );
    write(
        root,
        "cart/cart.go",
        "package cart\n\ntype Cart struct{ total int }\n\nfunc (c *Cart) Total() int { return c.total }\n",
    );
    write(
        root,
        "cart/new.go",
        "package cart\n\nfunc New() *Cart { return &Cart{} }\n",
    );
    write(
        root,
        "cart/cart_test.go",
        "package cart\n\nimport \"testing\"\n\nfunc TestTotal(t *testing.T) {\n\tc := New()\n\tif c.Total() != 0 {\n\t\tt.Fail()\n\t}\n}\n",
    );
    write(
        root,
        "vendor/github.com/gin-gonic/gin/gin.go",
        "package gin\n\nfunc Default() {}\n",
    );
    let batch = indexer(root).index_all(&HashMap::new(), &[], None).unwrap();
    let files: Vec<String> = scopes(&batch)
        .into_iter()
        .filter(|s| s.starts_with("file:"))
        .collect();
    assert_eq!(
        files,
        [
            "file:cart/cart.go",
            "file:cart/cart_test.go",
            "file:cart/new.go",
            "file:main.go"
        ]
    );
    let edges = |scope: &str| -> Vec<String> {
        update(&batch, scope)
            .observations
            .as_ref()
            .unwrap()
            .observations
            .iter()
            .map(|o| format!("{} {}", o.kind, o.dst))
            .collect()
    };
    let main = edges("file:main.go");
    for edge in [
        // The package's first file, tests aside; a call, the file that defines it.
        "imports file:cart/cart.go",
        "calls file:cart/new.go",
        "imports pkg:gomod/github.com/gin-gonic/gin",
    ] {
        assert!(main.contains(&edge.to_owned()), "{edge}: {main:?}");
    }
    // In its own package, a name needs no import.
    let test = edges("file:cart/cart_test.go");
    assert!(
        test.contains(&"calls file:cart/new.go".to_owned()),
        "{test:?}"
    );
    let new = edges("file:cart/new.go");
    assert!(
        new.contains(&"references file:cart/cart.go".to_owned()),
        "{new:?}"
    );
    let symbols: Vec<String> = update(&batch, "file:cart/cart.go")
        .nodes
        .iter()
        .flatten()
        .map(|n| n.id.as_str().to_owned())
        .collect();
    assert_eq!(
        symbols,
        [
            "go:example.com/shop/cart/cart#Cart",
            "go:example.com/shop/cart/cart#Cart.total",
            "go:example.com/shop/cart/cart#Cart.Total"
        ]
    );
    let packages: Vec<String> = update(&batch, "structure")
        .nodes
        .iter()
        .flatten()
        .map(|n| n.id.as_str().to_owned())
        .filter(|id| id.starts_with("pkg:"))
        .collect();
    assert_eq!(
        packages,
        [
            "pkg:gomod/example.com/shop",
            "pkg:gomod/github.com/gin-gonic/gin"
        ]
    );
}

/// T-123: a Maven project: classes link to one another through imports of a class, of a
/// package (`*`) and of a static member, and without an import inside their package; a test
/// under `src/test/java` reaches them; names are dotted paths from the source root; `target/`
/// is skipped; the runtime dependencies are packages, the test ones are not.
#[test]
fn a_maven_project_links_its_classes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "pom.xml",
        r#"<?xml version="1.0"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
  <modelVersion>4.0.0</modelVersion>
  <groupId>com.shop</groupId>
  <artifactId>shop</artifactId>
  <version>1.0</version>
  <dependencies>
    <dependency><groupId>com.google.code.gson</groupId><artifactId>gson</artifactId><version>2.13.2</version></dependency>
    <dependency><groupId>org.junit.jupiter</groupId><artifactId>junit-jupiter</artifactId><version>6.1.3</version><scope>test</scope></dependency>
  </dependencies>
</project>
"#,
    );
    write(
        root,
        "src/main/java/com/shop/App.java",
        "package com.shop;\n\nimport com.shop.model.*;\nimport static com.shop.Prices.priceOf;\nimport com.google.gson.Gson;\n\npublic class App {\n    public static void main(String[] args) {\n        Item item = new Item();\n        System.out.println(new Gson().toJson(item) + priceOf(item));\n        Cart.empty();\n    }\n}\n",
    );
    write(
        root,
        "src/main/java/com/shop/Cart.java",
        "package com.shop;\n\npublic class Cart {\n    public static Cart empty() { return new Cart(); }\n}\n",
    );
    write(
        root,
        "src/main/java/com/shop/Prices.java",
        "package com.shop;\n\nimport com.shop.model.Item;\n\npublic final class Prices {\n    public static int priceOf(Item item) { return 2; }\n}\n",
    );
    write(
        root,
        "src/main/java/com/shop/model/Item.java",
        "package com.shop.model;\n\npublic class Item {\n    public String name;\n}\n",
    );
    write(
        root,
        "src/test/java/com/shop/PricesTest.java",
        "package com.shop;\n\nimport org.junit.jupiter.api.Test;\nimport com.shop.model.Item;\n\nclass PricesTest {\n    @Test\n    void apple() { Prices.priceOf(new Item()); }\n}\n",
    );
    write(root, "target/generated-sources/X.java", "class X {}\n");
    let batch = indexer(root).index_all(&HashMap::new(), &[], None).unwrap();
    let files: Vec<String> = scopes(&batch)
        .into_iter()
        .filter(|s| s.starts_with("file:"))
        .collect();
    assert_eq!(
        files,
        [
            "file:src/main/java/com/shop/App.java",
            "file:src/main/java/com/shop/Cart.java",
            "file:src/main/java/com/shop/Prices.java",
            "file:src/main/java/com/shop/model/Item.java",
            "file:src/test/java/com/shop/PricesTest.java"
        ]
    );
    let edges = |scope: &str| -> Vec<String> {
        update(&batch, scope)
            .observations
            .as_ref()
            .unwrap()
            .observations
            .iter()
            .map(|o| format!("{} {}", o.kind, o.dst))
            .collect()
    };
    let app = edges("file:src/main/java/com/shop/App.java");
    for edge in [
        "imports file:src/main/java/com/shop/Prices.java",
        "imports pkg:maven/com.google.code.gson:gson",
        // Through `com.shop.model.*`.
        "calls file:src/main/java/com/shop/model/Item.java",
        // Its own package, no import.
        "calls file:src/main/java/com/shop/Cart.java",
        "calls file:src/main/java/com/shop/Prices.java",
    ] {
        assert!(app.contains(&edge.to_owned()), "{edge}: {app:?}");
    }
    let test = edges("file:src/test/java/com/shop/PricesTest.java");
    assert!(
        test.contains(&"calls file:src/main/java/com/shop/Prices.java".to_owned()),
        "{test:?}"
    );
    let symbols: Vec<String> = update(&batch, "file:src/main/java/com/shop/model/Item.java")
        .nodes
        .iter()
        .flatten()
        .map(|n| n.id.as_str().to_owned())
        .collect();
    assert_eq!(
        symbols,
        [
            "java:com.shop.model.Item#Item",
            "java:com.shop.model.Item#Item.name"
        ]
    );
    let packages: Vec<String> = update(&batch, "structure")
        .nodes
        .iter()
        .flatten()
        .map(|n| n.id.as_str().to_owned())
        .filter(|id| id.starts_with("pkg:"))
        .collect();
    assert_eq!(
        packages,
        [
            "pkg:maven/com.shop:shop",
            "pkg:maven/com.google.code.gson:gson"
        ]
    );
}

/// T-124: a CMake project with vcpkg: a source includes its header beside it, another one
/// through `include/`; a name declared in a header and defined in a source leads to the
/// header; includes of a dependency lead to it, the system's to nothing; names are paths with
/// their extension; `build/` is skipped; the test framework is not at run time.
#[test]
fn a_cmake_project_links_its_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "CMakeLists.txt",
        "cmake_minimum_required(VERSION 3.20)\nproject(shop VERSION 1.0 LANGUAGES CXX)\nfind_package(fmt CONFIG REQUIRED)\nfind_package(GTest CONFIG REQUIRED) # tests\nadd_subdirectory(tests)\n",
    );
    write(
        root,
        "vcpkg.json",
        r#"{"name": "shop", "dependencies": ["fmt", {"name": "nlohmann-json"}, "gtest"]}"#,
    );
    write(
        root,
        "include/shop/cart.hpp",
        "#pragma once\n#include <vector>\nnamespace shop {\nclass Cart {\npublic:\n    int total() const;\nprivate:\n    std::vector<int> prices_;\n};\nint price(int item);\n}\n",
    );
    write(
        root,
        "src/cart.cpp",
        "#include \"shop/cart.hpp\"\n#include <fmt/core.h>\nnamespace shop {\nint Cart::total() const { return price(1); }\n}\n",
    );
    write(
        root,
        "src/price.cpp",
        "#include \"shop/cart.hpp\"\nnamespace shop {\nint price(int item) { return item * 2; }\n}\n",
    );
    write(
        root,
        "src/main.cpp",
        "#include <shop/cart.hpp>\n#include <nlohmann/json.hpp>\nint main() {\n    shop::Cart cart;\n    return cart.total() + shop::price(2);\n}\n",
    );
    write(
        root,
        "tests/cart_test.cpp",
        "#include <gtest/gtest.h>\n#include \"../include/shop/cart.hpp\"\nTEST(Cart, Empty) { EXPECT_EQ(shop::Cart{}.total(), 0); }\n",
    );
    write(
        root,
        "tests/CMakeLists.txt",
        "add_executable(cart_test cart_test.cpp)\n",
    );
    write(
        root,
        "build/generated.cpp",
        "int generated() { return 0; }\n",
    );
    let batch = indexer(root).index_all(&HashMap::new(), &[], None).unwrap();
    let files: Vec<String> = scopes(&batch)
        .into_iter()
        .filter(|s| s.starts_with("file:"))
        .collect();
    assert_eq!(
        files,
        [
            "file:include/shop/cart.hpp",
            "file:src/cart.cpp",
            "file:src/main.cpp",
            "file:src/price.cpp",
            "file:tests/cart_test.cpp"
        ]
    );
    let edges = |scope: &str| -> Vec<String> {
        update(&batch, scope)
            .observations
            .as_ref()
            .unwrap()
            .observations
            .iter()
            .map(|o| format!("{} {}", o.kind, o.dst))
            .collect()
    };
    let main = edges("file:src/main.cpp");
    for edge in [
        "imports file:include/shop/cart.hpp",
        "imports pkg:cmake/nlohmann-json",
        // Declared in the header the file includes, defined in `src/price.cpp`.
        "calls file:include/shop/cart.hpp",
    ] {
        assert!(main.contains(&edge.to_owned()), "{edge}: {main:?}");
    }
    let cart = edges("file:src/cart.cpp");
    for edge in [
        "imports file:include/shop/cart.hpp",
        "imports pkg:cmake/fmt",
        "calls file:include/shop/cart.hpp",
    ] {
        assert!(cart.contains(&edge.to_owned()), "{edge}: {cart:?}");
    }
    let test = edges("file:tests/cart_test.cpp");
    assert!(
        test.contains(&"imports file:include/shop/cart.hpp".to_owned()),
        "{test:?}"
    );
    let symbols: Vec<String> = update(&batch, "file:src/cart.cpp")
        .nodes
        .iter()
        .flatten()
        .map(|n| n.id.as_str().to_owned())
        .collect();
    assert_eq!(
        symbols,
        [
            "cpp:src/cart.cpp#shop",
            "cpp:src/cart.cpp#shop::Cart::total"
        ]
    );
    let packages: Vec<String> = update(&batch, "structure")
        .nodes
        .iter()
        .flatten()
        .map(|n| n.id.as_str().to_owned())
        .filter(|id| id.starts_with("pkg:"))
        .collect();
    assert_eq!(
        packages,
        ["pkg:cmake/shop", "pkg:cmake/fmt", "pkg:cmake/nlohmann-json"]
    );
}

/// T-127: a Gradle project of Kotlin and Java: a top-level function imported by name, a class
/// of the same package without an import, Kotlin calling Java and Java calling Kotlin; names
/// are dotted paths from `src/main/kotlin`.
#[test]
fn a_kotlin_project_links_its_files_and_java() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "build.gradle.kts",
        "plugins {\n    kotlin(\"jvm\") version \"2.4.10\"\n}\n\ngroup = \"com.shop\"\n\ndependencies {\n    implementation(\"io.ktor:ktor-server-core:3.3.1\")\n}\n",
    );
    write(
        root,
        "src/main/kotlin/com/shop/pricing/Pricing.kt",
        "package com.shop.pricing\n\nfun total(prices: List<Int>): Int = prices.sum()\n",
    );
    write(
        root,
        "src/main/kotlin/com/shop/Cart.kt",
        "package com.shop\n\nimport com.shop.pricing.total\nimport com.shop.legacy.Ledger\n\nclass Cart {\n    private val prices = mutableListOf<Int>()\n\n    fun sum(): Int = total(prices)\n\n    fun record() {\n        Ledger.write(sum())\n    }\n}\n",
    );
    write(
        root,
        "src/main/kotlin/com/shop/App.kt",
        "package com.shop\n\nimport io.ktor.server.engine.embeddedServer\n\nfun main() {\n    Cart().record()\n    embeddedServer()\n}\n",
    );
    write(
        root,
        "src/main/java/com/shop/legacy/Ledger.java",
        "package com.shop.legacy;\n\nimport com.shop.Cart;\n\npublic final class Ledger {\n    public static void write(int total) {}\n    public static Cart fresh() { return new Cart(); }\n}\n",
    );
    let batch = indexer(root).index_all(&HashMap::new(), &[], None).unwrap();
    let edges = |scope: &str| -> Vec<String> {
        update(&batch, scope)
            .observations
            .as_ref()
            .unwrap()
            .observations
            .iter()
            .map(|o| format!("{} {}", o.kind, o.dst))
            .collect()
    };
    let cart = edges("file:src/main/kotlin/com/shop/Cart.kt");
    for edge in [
        "imports file:src/main/kotlin/com/shop/pricing/Pricing.kt",
        "calls file:src/main/kotlin/com/shop/pricing/Pricing.kt",
        "imports file:src/main/java/com/shop/legacy/Ledger.java",
        "calls file:src/main/java/com/shop/legacy/Ledger.java",
    ] {
        assert!(cart.contains(&edge.to_owned()), "{edge}: {cart:?}");
    }
    let app = edges("file:src/main/kotlin/com/shop/App.kt");
    for edge in [
        // Its own package, no import.
        "calls file:src/main/kotlin/com/shop/Cart.kt",
        // By the group of the dependency.
        "imports pkg:maven/io.ktor:ktor-server-core",
    ] {
        assert!(app.contains(&edge.to_owned()), "{edge}: {app:?}");
    }
    let ledger = edges("file:src/main/java/com/shop/legacy/Ledger.java");
    assert!(
        ledger.contains(&"imports file:src/main/kotlin/com/shop/Cart.kt".to_owned()),
        "{ledger:?}"
    );
    let symbols: Vec<String> = update(&batch, "file:src/main/kotlin/com/shop/Cart.kt")
        .nodes
        .iter()
        .flatten()
        .map(|n| n.id.as_str().to_owned())
        .collect();
    assert_eq!(
        symbols,
        [
            "kotlin:com.shop.Cart#Cart",
            "kotlin:com.shop.Cart#Cart.prices",
            "kotlin:com.shop.Cart#Cart.sum",
            "kotlin:com.shop.Cart#Cart.record"
        ]
    );
}

/// T-128: a Swift package: a target's files see each other's names without an import, an
/// extension's members are ranged under their type, a test target reaches the target it
/// imports, an executable's top-level names stay in it, a dependency's module leads to it.
#[test]
fn a_swift_package_links_its_targets() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "Package.swift",
        "// swift-tools-version: 6.0\nimport PackageDescription\n\nlet package = Package(\n    name: \"Shop\",\n    dependencies: [\n        .package(url: \"https://github.com/apple/swift-log.git\", from: \"1.6.0\"),\n    ],\n    targets: [\n        .target(name: \"Shop\"),\n        .executableTarget(name: \"App\", dependencies: [\"Shop\", .product(name: \"Logging\", package: \"swift-log\")]),\n        .testTarget(name: \"ShopTests\", dependencies: [\"Shop\"]),\n    ]\n)\n",
    );
    write(
        root,
        "Sources/Shop/Model/Item.swift",
        "public struct Item {\n    public let price: Int\n}\n",
    );
    write(
        root,
        "Sources/Shop/Cart.swift",
        "public struct Cart {\n    public var items: [Item] = []\n}\n",
    );
    write(
        root,
        "Sources/Shop/Cart+Total.swift",
        "extension Cart {\n    public var total: Int { items.count }\n    public func describe() -> String { \"\" }\n}\n",
    );
    write(
        root,
        "Sources/App/main.swift",
        "import Logging\nimport Shop\n\nlet cart = Cart()\nLogger(label: \"x\").info(\"\\(cart.total)\")\n",
    );
    write(
        root,
        "Tests/ShopTests/CartTests.swift",
        "import XCTest\n@testable import Shop\n\nfinal class CartTests: XCTestCase {\n    func testTotal() {\n        let cart = Cart()\n        XCTAssertEqual(cart.total, 0)\n    }\n}\n",
    );
    let batch = indexer(root).index_all(&HashMap::new(), &[], None).unwrap();
    let edges = |scope: &str| -> Vec<String> {
        update(&batch, scope)
            .observations
            .as_ref()
            .unwrap()
            .observations
            .iter()
            .map(|o| format!("{} {}", o.kind, o.dst))
            .collect()
    };
    let cart = edges("file:Sources/Shop/Cart.swift");
    assert!(
        cart.contains(&"references file:Sources/Shop/Model/Item.swift".to_owned()),
        "{cart:?}"
    );
    let main = edges("file:Sources/App/main.swift");
    for edge in [
        "imports pkg:swiftpm/github.com/apple/swift-log",
        "calls file:Sources/Shop/Cart.swift",
    ] {
        assert!(main.contains(&edge.to_owned()), "{edge}: {main:?}");
    }
    let test = edges("file:Tests/ShopTests/CartTests.swift");
    assert!(
        test.contains(&"calls file:Sources/Shop/Cart.swift".to_owned()),
        "{test:?}"
    );
    assert!(
        !test.iter().any(|e| e.ends_with("Sources/App/main.swift")),
        "the executable's `cart` is not the test's: {test:?}"
    );
    let symbols: Vec<String> = update(&batch, "file:Sources/Shop/Cart+Total.swift")
        .nodes
        .iter()
        .flatten()
        .map(|n| n.id.as_str().to_owned())
        .collect();
    assert_eq!(
        symbols,
        [
            "swift:Shop/Cart+Total#Cart.total",
            "swift:Shop/Cart+Total#Cart.describe"
        ]
    );
}

/// T-129: a CMake project in C: a source includes its header (read by the C++ pack), a name
/// declared in the header and defined in the source leads to the header, a test reaches it,
/// and the names of C carry their path.
#[test]
fn a_c_project_links_its_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "CMakeLists.txt",
        "cmake_minimum_required(VERSION 3.20)\nproject(cart LANGUAGES C)\nfind_package(unofficial-sqlite3 CONFIG REQUIRED)\nadd_library(cart src/cart.c)\n",
    );
    write(
        root,
        "include/cart.h",
        "#pragma once\nstruct cart { int count; };\nint cart_total(const struct cart *c);\n",
    );
    write(
        root,
        "src/cart.c",
        "#include \"cart.h\"\n#include <sqlite3.h>\nint cart_total(const struct cart *c) { return c->count; }\n",
    );
    write(
        root,
        "tests/cart_test.c",
        "#include <cart.h>\nint main(void) { struct cart c = {0}; return cart_total(&c); }\n",
    );
    let batch = indexer(root).index_all(&HashMap::new(), &[], None).unwrap();
    let edges = |scope: &str| -> Vec<String> {
        update(&batch, scope)
            .observations
            .as_ref()
            .unwrap()
            .observations
            .iter()
            .map(|o| format!("{} {}", o.kind, o.dst))
            .collect()
    };
    let source = edges("file:src/cart.c");
    for edge in [
        "imports file:include/cart.h",
        "imports pkg:cmake/unofficial-sqlite3",
    ] {
        assert!(source.contains(&edge.to_owned()), "{edge}: {source:?}");
    }
    let test = edges("file:tests/cart_test.c");
    for edge in ["imports file:include/cart.h", "calls file:include/cart.h"] {
        assert!(test.contains(&edge.to_owned()), "{edge}: {test:?}");
    }
    let symbols: Vec<String> = update(&batch, "file:src/cart.c")
        .nodes
        .iter()
        .flatten()
        .map(|n| n.id.as_str().to_owned())
        .collect();
    assert_eq!(symbols, ["c:src/cart.c#cart_total"]);
}

/// T-130: a .NET solution: two projects named by their assembly, a `using` leads to the files
/// of its namespace, a type of another project is reached by its name, a test project's
/// packages are not at run time, NuGet packages lead by their namespace.
#[test]
fn a_dotnet_solution_links_its_projects() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "src/Shop/Shop.csproj",
        "\u{feff}<Project Sdk=\"Microsoft.NET.Sdk\">\n  <ItemGroup>\n    <PackageReference Include=\"Newtonsoft.Json\" Version=\"13.0.4\" />\n  </ItemGroup>\n</Project>\n",
    );
    write(
        root,
        "src/Shop/Model/Item.cs",
        "namespace Shop.Model;\n\npublic record Item(string Name, int Price);\n",
    );
    write(
        root,
        "src/Shop/Cart.cs",
        "using Newtonsoft.Json;\nusing Shop.Model;\n\nnamespace Shop;\n\npublic class Cart\n{\n    private readonly List<Item> _items = new();\n    public string Json() => JsonConvert.SerializeObject(_items);\n}\n",
    );
    write(
        root,
        "tests/Shop.Tests/Shop.Tests.csproj",
        "<Project Sdk=\"Microsoft.NET.Sdk\">\n  <ItemGroup>\n    <PackageReference Include=\"Microsoft.NET.Test.Sdk\" Version=\"17.14.1\" />\n    <PackageReference Include=\"xunit\" Version=\"2.9.3\" />\n  </ItemGroup>\n</Project>\n",
    );
    write(
        root,
        "tests/Shop.Tests/CartTests.cs",
        "using Shop;\n\nnamespace Shop.Tests;\n\npublic class CartTests\n{\n    [Fact]\n    public void Serialises() => Assert.NotNull(new Cart().Json());\n}\n",
    );
    write(root, "src/Shop/obj/Generated.cs", "class Generated {}\n");
    let batch = indexer(root).index_all(&HashMap::new(), &[], None).unwrap();
    let edges = |scope: &str| -> Vec<String> {
        update(&batch, scope)
            .observations
            .as_ref()
            .unwrap()
            .observations
            .iter()
            .map(|o| format!("{} {}", o.kind, o.dst))
            .collect()
    };
    let cart = edges("file:src/Shop/Cart.cs");
    for edge in [
        "imports file:src/Shop/Model/Item.cs",
        "imports pkg:nuget/Newtonsoft.Json",
    ] {
        assert!(cart.contains(&edge.to_owned()), "{edge}: {cart:?}");
    }
    let test = edges("file:tests/Shop.Tests/CartTests.cs");
    assert!(
        test.contains(&"calls file:src/Shop/Cart.cs".to_owned()),
        "{test:?}"
    );
    let packages: Vec<String> = update(&batch, "structure")
        .nodes
        .iter()
        .flatten()
        .map(|n| n.id.as_str().to_owned())
        .filter(|id| id.starts_with("pkg:"))
        .collect();
    assert_eq!(
        packages,
        [
            "pkg:nuget/Shop",
            "pkg:nuget/Shop.Tests",
            "pkg:nuget/Newtonsoft.Json"
        ]
    );
    let files: Vec<String> = scopes(&batch)
        .into_iter()
        .filter(|s| s.contains("obj"))
        .collect();
    assert!(files.is_empty(), "{files:?}");
}

/// T-131: a Composer project: names follow the PSR-4 map, a `use` leads to the class's file or
/// to the package that autoloads its namespace (from `composer.lock`), a class of the same
/// namespace needs no `use`, `vendor/` is skipped.
#[test]
fn a_composer_project_links_its_classes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "composer.json",
        "{\n  \"name\": \"shop/shop\",\n  \"require\": {\"php\": \">=8.4\", \"monolog/monolog\": \"^3.9\"},\n  \"require-dev\": {\"phpunit/phpunit\": \"^13.4\"},\n  \"autoload\": {\"psr-4\": {\"Shop\\\\\": \"src/\"}}\n}\n",
    );
    write(
        root,
        "composer.lock",
        "{\"packages\": [{\"name\": \"monolog/monolog\", \"version\": \"3.9.0\", \"autoload\": {\"psr-4\": {\"Monolog\\\\\": \"src/Monolog\"}}}], \"packages-dev\": []}\n",
    );
    write(
        root,
        "src/Model/Item.php",
        "<?php\nnamespace Shop\\Model;\nfinal class Item { public int $price = 0; }\n",
    );
    write(
        root,
        "src/Cart.php",
        "<?php\nnamespace Shop;\nuse Shop\\Model\\Item;\nuse Monolog\\Logger;\nfinal class Cart {\n    public function add(Item $item, Logger $log): Store { return new Store(); }\n}\n",
    );
    write(
        root,
        "src/Store.php",
        "<?php\nnamespace Shop;\nfinal class Store {}\n",
    );
    write(
        root,
        "vendor/monolog/monolog/src/Logger.php",
        "<?php\nclass Logger {}\n",
    );
    let batch = indexer(root).index_all(&HashMap::new(), &[], None).unwrap();
    let edges = |scope: &str| -> Vec<String> {
        update(&batch, scope)
            .observations
            .as_ref()
            .unwrap()
            .observations
            .iter()
            .map(|o| format!("{} {}", o.kind, o.dst))
            .collect()
    };
    let cart = edges("file:src/Cart.php");
    for edge in [
        "imports file:src/Model/Item.php",
        "imports pkg:composer/monolog/monolog",
        "calls file:src/Store.php",
    ] {
        assert!(cart.contains(&edge.to_owned()), "{edge}: {cart:?}");
    }
    let symbols: Vec<String> = update(&batch, "file:src/Model/Item.php")
        .nodes
        .iter()
        .flatten()
        .map(|n| n.id.as_str().to_owned())
        .collect();
    assert_eq!(
        symbols,
        [
            "php:Shop\\Model\\Item#Item",
            "php:Shop\\Model\\Item#Item.price"
        ]
    );
    let files: Vec<String> = scopes(&batch)
        .into_iter()
        .filter(|s| s.contains("vendor"))
        .collect();
    assert!(files.is_empty(), "{files:?}");
}

/// T-132: an sbt build: Scala's imports with selectors lead to their classes, a class of the
/// same package needs no import, a test reaches the code, a dependency leads by its group, and
/// Scala sees Java's classes.
#[test]
fn an_sbt_build_links_its_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "build.sbt",
        "ThisBuild / scalaVersion := \"3.10.0\"\nThisBuild / organization := \"com.shop\"\nname := \"shop\"\nlibraryDependencies += \"com.lihaoyi\" %% \"upickle\" % \"4.4.3\"\n",
    );
    write(
        root,
        "src/main/scala/com/shop/model/Item.scala",
        "package com.shop.model\n\nfinal case class Item(name: String, price: Int)\n",
    );
    write(
        root,
        "src/main/scala/com/shop/Cart.scala",
        "package com.shop\n\nimport com.shop.model.{Item, Price => P}\nimport upickle.default.*\n\nclass Cart:\n  def add(item: Item): Store = Store.empty\n",
    );
    write(
        root,
        "src/main/scala/com/shop/Store.scala",
        "package com.shop\n\nobject Store:\n  def empty: Store = new Store\nclass Store\n",
    );
    write(
        root,
        "src/main/java/com/shop/legacy/Ledger.java",
        "package com.shop.legacy;\npublic class Ledger {}\n",
    );
    write(
        root,
        "src/test/scala/com/shop/CartSuite.scala",
        "package com.shop\n\nimport com.shop.legacy.Ledger\n\nclass CartSuite extends munit.FunSuite:\n  test(\"add\") { new Cart; new Ledger }\n",
    );
    let batch = indexer(root).index_all(&HashMap::new(), &[], None).unwrap();
    let edges = |scope: &str| -> Vec<String> {
        update(&batch, scope)
            .observations
            .as_ref()
            .unwrap()
            .observations
            .iter()
            .map(|o| format!("{} {}", o.kind, o.dst))
            .collect()
    };
    let cart = edges("file:src/main/scala/com/shop/Cart.scala");
    for edge in [
        "imports file:src/main/scala/com/shop/model/Item.scala",
        "imports pkg:sbt/com.lihaoyi:upickle_3",
        "calls file:src/main/scala/com/shop/Store.scala",
    ] {
        assert!(cart.contains(&edge.to_owned()), "{edge}: {cart:?}");
    }
    let suite = edges("file:src/test/scala/com/shop/CartSuite.scala");
    for edge in [
        "imports file:src/main/java/com/shop/legacy/Ledger.java",
        "calls file:src/main/scala/com/shop/Cart.scala",
    ] {
        assert!(suite.contains(&edge.to_owned()), "{edge}: {suite:?}");
    }
}

/// A pack's skipped folders are skipped beside a manifest of its ecosystem only: an npm
/// workspace's `packages/` and a crate's `src/bin/` are code, a .NET project's `bin/` is not.
#[test]
fn skipped_folders_belong_to_their_ecosystem() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "package.json",
        r#"{"name": "root", "workspaces": ["packages/*"]}"#,
    );
    write(
        root,
        "packages/core/package.json",
        r#"{"name": "@shop/core"}"#,
    );
    write(root, "packages/core/src/index.ts", "export const k = 1;\n");
    write(
        root,
        "crate/Cargo.toml",
        "[package]\nname = \"tool\"\nversion = \"0.1.0\"\n",
    );
    write(root, "crate/src/lib.rs", "pub fn f() {}\n");
    write(root, "crate/src/bin/tool.rs", "fn main() {}\n");
    write(
        root,
        "dotnet/Shop.csproj",
        "<Project Sdk=\"Microsoft.NET.Sdk\"></Project>\n",
    );
    write(root, "dotnet/Cart.cs", "class Cart {}\n");
    write(
        root,
        "dotnet/bin/Debug/Generated.cs",
        "class Generated {}\n",
    );
    write(
        root,
        "packages/core/node_modules/dep/index.ts",
        "export const x = 1;\n",
    );
    let batch = indexer(root).index_all(&HashMap::new(), &[], None).unwrap();
    let files: Vec<String> = scopes(&batch)
        .into_iter()
        .filter(|s| s.starts_with("file:"))
        .collect();
    assert_eq!(
        files,
        [
            "file:crate/src/bin/tool.rs",
            "file:crate/src/lib.rs",
            "file:dotnet/Cart.cs",
            "file:packages/core/src/index.ts"
        ]
    );
}
