---
name: graph-model
description: Timonier's unified code graph — node and edge kinds, provenance and confidence, the strict parent tree, stable identity (SCIP symbol or lang:qualified_name), move detection, multi-source fusion rules, valid_from/valid_to intervals, snapshots and snapshot diffs, aggregated component edges, and how to write a SQLite migration. Use whenever touching timonier-core types, the timonier-store schema or queries, indexing output, or any code that reads, merges or diffs the graph.
---

# Graph model

The unified graph is the **only source of truth**: the IDE reads it over JSON-RPC, the agent
over MCP. Everything below marked *(spec)* comes from `docs/spec.md` and is not negotiable
without asking the human. Everything else is decided by ADR before it is implemented.

## Schema (spec)

**Nodes**: `Repo`, `Component`, `Package`, `File`, `Symbol`. Symbol kinds: module, type, trait,
function, method, field, constant. Each node has a name, a language, a span and a parent.

**Edges**: `Imports`, `Calls`, `References`, `Implements`, `Inherits`, `DependsOn`. Each edge has:

- a provenance ∈ `TreeSitter`, `Lsp`, `Scip`, `Manifest`, `Inferred`, `Intended`;
- a `confidence`;
- a `weight`.

**Structure**

- The parent hierarchy is a **strict tree**; relations form a free graph.
- Edges between components are **aggregated from symbol edges, never stored by hand**.
- `Intended` comes only from the whiteboard (`.timonier/intent.json`). It is never merged into
  the actual graph. The intent ↔ actual comparison is computed and never written back.

## Granularity of `Calls` edges (spec, « Décisions techniques »)

- Function → function edges are stored **only** when they come from `Lsp` or `Scip`.
- Otherwise the graph stores weighted file → file edges.
- Display is always aggregated at the current zoom level, starting from the high-level view by
  role.
- LSP refinement is lazy: it runs when a file is opened.
- Edges are compacted beyond roughly 50,000 symbol edges.
- Each file records "verified by LSP at hash X".

## Identity and moves (spec)

- Stable id: the SCIP symbol string when one exists, else `lang:qualified_name`.
- At diff time, moves are detected by similarity: signature plus hash of the normalised body.
  A refactoring must never show up as "everything deleted, everything recreated".
- Identity must not change with formatting, comments or reordering inside a file.

## Fusion of sources (spec)

- An edge seen by several sources keeps the most reliable provenance.
- A `TreeSitter` edge contradicted by the `Lsp` is removed.
- The source ranking and what "contradicted" means precisely are fixed by ADR in Phase 2.

## Storage (spec)

- SQLite, updated continuously, with a single writer in WAL mode (see the `rust-daemon` skill).
- Every node and edge carries a `valid_from` / `valid_to` interval of snapshot sequence numbers.
  `valid_to` NULL means "still valid".
- `snapshot(seq, commit_sha, timestamp, kind)` with `kind ∈ {commit, review_mark, working_tree}`.
- **The diff between two snapshots is a query**, not a recomputation.
- `.timonier/actual.json` is regenerated at commit time by a git hook.

## Decisions (ADR 0008 to 0011, Phase 2): read them before changing the model

- **Hierarchy** (Q4, ADR 0008): `Repo → Component* → Package → Component* → File → Symbol*`.
  The allowed parents are in `timonier_core::tree::parent_allowed`. One repo; no package under a
  package; every node except the repo has a parent. The store checks the tree at the end of
  every batch and refuses the whole batch on a violation.
- **Ids** (ADR 0008): `repo:<name>`, `pkg:<ecosystem>/<name>`, `component:<key>`,
  `file:<relative/posix/path>`, `scip:<SCIP symbol verbatim>`, `<language>:<qualified name>`.
  Languages are `[a-z][a-z0-9_+-]*` and never one of the reserved prefixes. Positions are
  0-based lines with UTF-8 byte columns.
- **Identity checks**: `Node::check_identity` requires the id prefix to match the kind (and the
  language of a `<language>:` id). Scope keys are only `structure` or a file id.
- **Scopes** (ADR 0009):
  - `structure` holds the repo, packages, components and files;
  - `file:<path>` holds only the symbols of that file, under the file or under one another;
  - an observed edge leaves something the scope provides.
- **Ownership** (ADR 0009): when several scopes provide the same node, the scope with the
  **smallest key** owns it; for an edge, the **most reliable provenance** wins, then the smallest
  key.
  - The other versions wait in `node_claim` / `edge_claim`, and the next one takes over when the
    owner lets go.
  - The head depends only on what each scope currently provides: never on the order of the
    updates; re-applying writes nothing.
  - Waiting contributions count in the orphan check: remove a file's scope in the same batch as
    (or before) removing the file from the structure.
- **Content anchoring** (ADR 0009): `SourceObservations::computed_on` says which content the
  observations describe. Observations computed on another content than the scope's current one
  are ignored (`ApplyStats::stale_observations`), and so are observations about a scope without
  content (even approximate ones). Precise sources (Lsp, Scip) must give it, and never bring a
  content hash nor nodes (`Error::PreciseSourceSetsContent`). A removal brings nothing else
  (`Error::RemovalWithContent`).
- **Refused inputs** (`SourceObservations::validate`):
  - `Intended`;
  - edges touching a `component:` or `repo:` id;
  - **`Calls` between symbols from any source but Lsp and Scip**: the others report calls
    between files only (spec).
- **Fusion** (ADR 0009, `timonier_core::fusion::fuse`), per scope:
  - if Scip or Lsp verified an edge kind, the TreeSitter and Inferred observations of that kind
    are dropped;
  - otherwise the best rank wins: Scip > Lsp > Manifest > TreeSitter > Inferred;
  - `Intended` is refused.
- **Dangling edges** (ADR 0009) are allowed: they stay invisible in views until both endpoints
  exist.
- **History** (ADR 0010): one open head snapshot (`working_tree`) takes every change; freezing it
  (`Store::freeze`) opens the next one.
  - A row created and deleted within the head is deleted physically.
  - A row created in the head is rewritten in place.
  - Otherwise the old version is closed and a new one opened.
  - The span is not versioned (updated in place).
- **Diff** (ADR 0010, `Reader::diff`): it reads only the rows whose `valid_from` or `valid_to`
  falls in ]from, to], then `timonier_core::diff::assemble` pairs moves:
  - first pass: same body is a move. A body shared by several symbols only pairs symbols that
    kept their name;
  - second pass: same name and signature is a move with a changed body, when exactly one
    removed and one added symbol share them;
  - a symbol that kept its id but changed file is a move (`SameId`);
  - a file is moved when at least half of its changed symbols followed it (including the
    `SameId` ones);
  - edges that only follow moves are hidden.
- **Aggregation** (`Reader::aggregate`): the edges of a zoom level, mapped to the nearest
  visible ancestors; never stored.
- **SQLite** (ADR 0011): rusqlite with `bundled`, migrations in
  `crates/timonier-store/migrations/` run by rusqlite_migration.
  - Pragmas: WAL, `synchronous=NORMAL`, `foreign_keys=ON`, `busy_timeout=5s`.
  - One writer thread with a synchronous API; read-only `Reader`s, one per thread.

## Update by difference

Re-indexing a file computes its new set of nodes and edges, compares it with the currently
valid set for that file, and then:

- closes the intervals of what disappeared;
- opens intervals for what appeared;
- leaves the rest untouched.

Re-applying an identical file must produce **zero row changes**. Test it.

## Writing a SQLite migration

1. Never edit a committed migration: add a new one.
2. One numbered file per migration: `crates/timonier-store/migrations/NNNN_short_description.sql`.
   Add it to `MIGRATION_SLICE` in `crates/timonier-store/src/schema.rs` with `M::up(include_str!(…))`.
3. One migration is one transaction, and migrations only move forward. The database is a
   rebuildable cache: an incompatible change may drop tables and trigger a full reindex, as
   long as the migration says so.
4. Every table holding graph facts has `valid_from` / `valid_to`, plus indexes that make the
   snapshot-diff queries cheap.
5. Add a test that applies all migrations to an empty database and to a fixture of the
   previous schema, then runs the diff queries.
6. If a concept of the schema changes, update this skill in the same change.

## Baseline (bench, 2026-10-14)

Run `cargo run --release -p timonier-store --example bench`. The bench pairs, for every file,
three tree-sitter file → file calls with two verified LSP calls per function. On the
development machine (Ryzen 9 5950X), with 1,000 files, 20,000 symbols and 40,000 edges:
- initial load: 0.50 s;
- one changed file: 0.76 ms (rows born in the head) or 1.0 ms (after a freeze);
- diff of 10 % of the files: 20 ms;
- package-level aggregation: 45 ms.

At 200,000 symbols the per-file update stays under 2 ms, but aggregating the whole graph takes
0.57 s: Phase 4 must cache aggregation per view. A test (`read.rs`, « reads use the indexes… »)
pins the query plans: head reads use the partial indexes of current rows.

## Test checklist for any graph change

- insertion;
- update by difference, including the zero-change reapplication;
- snapshot diff;
- file move and symbol move detection;
- fusion precedence;
- strict-tree invariant (property test with `proptest`);
- serde round-trip of every public type.
