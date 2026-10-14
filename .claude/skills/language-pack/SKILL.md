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

The MVP ships two packs, Rust and TypeScript, so that Timonier can index itself. The on-disk
pack format and where packs live are fixed by an ADR in Phase 3. Until then, the list below is
the content a pack must provide, taken from the spec and the start-up prompt.

## What a pack contains

1. **Grammar**: the tree-sitter grammar crate, with its name, version and licence verified on
   crates.io and recorded in the pack manifest.
   - Verified on 2026-10-14: `tree-sitter-rust` 0.24.2 and `tree-sitter-typescript` 0.23.2 are
     both MIT and load in `tree-sitter` 0.27 (ABI 15 and 14).
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
7. **Instrumentation conventions**, copied into `.timonier/conventions/<lang>/` by
   `timonier init`. They cover:
   - which OpenTelemetry SDK packages to add;
   - the initialisation module, behind a feature or a configuration switch;
   - how the rules of the `otel-instrumentation` skill translate to this language.
8. **Test runner**: how to detect it, how to make it produce JUnit XML, and how each test
   becomes a root span.
9. **LSP**: which server the pack relies on and which capabilities (definition, references,
   call hierarchy).
   - Decided by the human (Q5): one LSP interface with two back-ends. The first relays the
     IDE's language servers through the extension and comes first in the MVP. The second is
     servers launched by the daemon, for headless or CI use, later.
10. **Security-surface queries**: these are v2. Keep room for them in the format, but do not
    implement them in the MVP.

## Adding a language: checklist

- [ ] The grammar crate is verified (existence, version, licence) and recorded in the pack.
- [ ] A small fixture project lives under the pack's tests, with golden snapshots of the
      extracted nodes and edges (`insta`).
- [ ] Qualified names are stable across a reformat (test).
- [ ] Moving a function between two files is detected as a move (test).
- [ ] Role heuristics are tested on the fixture: the expected role for each fixture component.
- [ ] Indexing time per thousand lines is measured and recorded in `PLAN.md`.
- [ ] Instrumentation conventions and runner configuration are written, and `timonier init
      --dry-run` shows the expected diff on the fixture.
- [ ] The user documentation (French) lists the language and its limits.
