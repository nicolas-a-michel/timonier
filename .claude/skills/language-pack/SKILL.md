---
name: language-pack
description: How to add or modify a Timonier language pack — tree-sitter grammar, extraction queries (definitions, imports, calls/references), qualified names and body normalisation, architectural-role heuristics, external-dependency grouping, OpenTelemetry instrumentation conventions and SDK setup, test-runner (JUnit XML) configuration. Use when adding a language, changing what is extracted from Rust or TypeScript code, tuning role detection, or touching what `timonier init` does for a language.
---

# Language packs

From the spec, two principles apply:

- No analyser is written per language. Three generic sources (tree-sitter, LSP, SCIP) are
  merged into one graph.
- Everything language-dependent lives in a **versioned, extensible pack**. Adding a language
  means writing a pack, never modifying the daemon.

Built-in packs: Rust and TypeScript (the MVP, so that Timonier can index itself), JavaScript
(Phase 10, ADR 0061); Python, Go, Java and C++ follow (PLAN.md Phase 10). The format is fixed by
ADR 0012, the extraction and resolution by ADR 0013: read them before changing a pack.

What a language still needs outside its pack directory (learnt with JavaScript, ADR 0061):
- its grammar in `src/grammar.rs` (the one place a grammar is compiled in), the crate in the
  workspace `Cargo.toml`, and the pack in `BUILTIN` of `src/pack.rs`;
- its ecosystem's manifests and module resolution in `src/ecosystem.rs` and `src/project.rs`
  when it is not Cargo or npm, and its extensions in `without_extension`;
- its lockfile in the daemon's `inventory.rs`;
- `init`'s detection of the language (`crates/timonier-cli/src/init/detect.rs`): a runner is
  taken only for a language detected;
- `is_public` in `timonier-core::gaps` for the instrumentation gaps;
- the map's empty state names the languages read (`webview/src/i18n.ts`).

`[walk] skip` lists the folders never walked, ignored or not: the ecosystem's installed
dependencies (`node_modules`; for Python `.venv`, for Go `vendor`…). Never vendor a compiler, a
SDK or a build tool in the repository (Q88): tests that need one skip when it is missing, and the
CI installs it.

## Format (ADR 0012)

A pack is a directory `crates/timonier-index/packs/<language>/`, embedded at build time
(`BUILTIN` in `crates/timonier-index/src/pack.rs`):

- `pack.toml`: `format`, `language`, `version`, `ecosystem` (`cargo` | `npm`), `[[grammars]]`,
  `[naming]`, `[paths]`, `[fingerprint]`, `[signature]`, `[dependencies]`, `[[roles]]`, and the
  reserved `[lsp]`, `[tests]`, `[instrumentation]`, `[security]`. Unknown keys are refused.
- `definitions.scm`, `references.scm`, `roles.scm`; `<query>.<grammar>.scm` adds patterns for
  one grammar only (JSX lives in `references.tsx.scm`).

Capture contract (anything else is refused at load time; `@_x` captures are free helpers for
predicates):

| Query | Captures |
| ----- | -------- |
| definitions | `@definition.<module|type|trait|function|method|field|constant>`, `@name`, `@body` (optional), `@scope` + `@scope.<x>` with `#set! timonier.segment` / `timonier.parent` templates |
| references | `@import`, `@import.path`, `@import.alias`, `@import.name`, `@reference.call`, `@reference.method`, `@reference.type`, `@reference.implements`, `@reference.inherits`, `@reference.source` |
| roles | `@role.<role>` (roles of `pack::ROLES`) |

Engine rules the queries rely on:

- One definition per node: the first pattern of the file wins (methods before functions).
- One symbol per qualified path in a file: the first with a body wins (TS overloads).
- One reference per node, the outer path wins, and nothing inside an `@import` counts.
- An import path gets the `prefix_field` of its enclosing lists prepended (`use a::{b::c}`).
- A trailing `*` imports a whole module; a trailing `trailing_self` segment means the module.
- A call on a receiver (`x.f()`, `this.n()`) is `@reference.method`: it resolves through an
  imported first segment or a path, never by its name alone (ADR 0013). Use `@reference.call`
  only where the path names what is called.

Pitfalls met while writing the built-in packs:

- **Predicates go inside their pattern's parentheses**: `((impl_item) @scope (#set! …))`. A
  predicate written after a pattern becomes an empty pattern that matches every node. The
  loader refuses patterns without capture for this reason.
- A query naming a node type the grammar lacks fails as a whole: put such patterns in a
  `<query>.<grammar>.scm` file.
- **Bump `version` whenever a query or a rule changes** (from the first release on): it keys the
  extraction cache, so the old facts would be served otherwise.

Check a change with `cargo test -p timonier-index` (snapshots in `tests/snapshots/`; review
them, then `INSTA_UPDATE=always` to accept) and judge the graph of a real repository with
`cargo run --release -p timonier-daemon --example index_dump [repository]`.

## What a pack contains

1. **Grammar**: the tree-sitter grammar crate, with its name, version and licence verified on
   crates.io and recorded in the pack manifest, plus one line in `src/grammar.rs`.
   - Verified on 2026-10-16: `tree-sitter-rust` 0.24.2 and `tree-sitter-typescript` 0.23.2 are
     both MIT and load in `tree-sitter` 0.27.1 (ABI 15 and 14).
2. **Extraction queries** (`.scm`):
   - definitions → `Symbol` nodes (kind, span, parent);
   - imports → `Imports` edges;
   - calls and references → **weighted file → file edges** with `TreeSitter` provenance.
     Function → function edges come only from the LSP or SCIP (see the `graph-model` skill).
3. **Qualified names**: the rule that builds `lang:qualified_name`.
   - Rust: crate name plus module path.
   - TypeScript: package name, path inside the package, then the exported name.
   - The name must survive reformatting and in-file reordering.
4. **Normalised body**: what is stripped (whitespace, comments) before hashing, for move
   detection.
5. **Role heuristics**: signals that map to an architectural role, with a confidence. Roles
   *(spec)*: server or entry point, data model, persistence, interface, background task, group
   of external libraries, utility.
   - Signal examples from the spec: a `TcpListener` or an HTTP router means a server; an ORM
     means persistence; serialised types mean a data model.
   - Decided by the human (Q6): the heuristics produce the names and roles by default. The
     connected agent can then refine them through a structured MCP write tool. The daemon
     never holds an LLM API key.
6. **External dependency grouping** by usage: network, serialisation, database, async. This is
   a mapping from package names to categories, kept in the pack.
7. **Instrumentation conventions**, the pack's `conventions/*.md` (embedded in `pack.rs`'s
   `BUILTIN` list), copied as they are into `.timonier/conventions/<lang>/` by
   `timonier init` (ADR 0034). They cover:
   - which OpenTelemetry SDK packages to add;
   - the initialisation module, behind a feature or a configuration switch;
   - how the rules of the `otel-instrumentation` skill translate to this language.
7b. **Instrumentation for `init`** (ADR 0050): `[instrumentation]` lists the SDK packages
   (name, version, features), the Cargo feature, the initialisation module from the pack's
   `instrumentation/` templates; `[tests.runners.smoke]` gives the smoke test `doctor` runs and
   its file. Templates are written into other projects: lint and type them there, not here.
8. **Test runner**: how to detect it, how to make it produce JUnit XML, and how each test
   becomes a root span. Format fixed by ADR 0034: `[[tests.runners]]` in `pack.toml`, tried in
   order (`name`, `uses` = a word of the `test` script, `command`, `junit`, `config` = a block
   `init` writes into a project file, `program`, `note`). `timonier init` writes the chosen one
   to `.timonier/tests.json`; Rust uses cargo-nextest (`cargo test` has no stable JUnit).
9. **LSP**: which server the pack relies on and which capabilities (definition, references,
   call hierarchy).
   - `[lsp] outside` (ADR 0022): names no repository defines (the standard library, the
     language's globals). A path tree-sitter leaves unresolved that starts with one of them is
     not asked about: keep the list to names that are never a repository's own, since a
     crate's own `Result` is resolved by tree-sitter first. Bump the pack version when the
     list changes.
   - Decided by the human (Q5): one LSP interface with two back-ends. The first relays the
     IDE's language servers through the extension and comes first in the MVP. The second is
     servers launched by the daemon, for headless or CI use, later.
10. **Security-surface queries**: these are v2. Keep room for them in the format, but do not
    implement them in the MVP.

## Adding a language: checklist

- [ ] The grammar crate is verified (existence, version, licence) and recorded in the pack.
- [ ] Fixture sources, inline in `crates/timonier-index/tests/extract.rs`, with golden
      snapshots of the extracted facts (`insta`). Keep fixtures inline: a fixture manifest in
      the repository would be indexed as part of the crate around it.
- [ ] Qualified names are stable across a reformat (test).
- [ ] Moving a function between two files is detected as a move (test, see
      `crates/timonier-daemon/tests/indexing.rs`).
- [ ] Role heuristics are tested on the fixture: the expected role for each fixture component.
- [ ] Indexing time per thousand lines is measured and recorded in `PLAN.md`.
- [ ] Instrumentation conventions and runner configuration are written, and `timonier init
      --dry-run` shows the expected diff on the fixture.
- [ ] The user documentation (French) lists the language and its limits.
