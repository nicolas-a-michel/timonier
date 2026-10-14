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

## Open points: decide by ADR, do not improvise

Check `PLAN.md` (« Questions ouvertes ») and `docs/adr/` first.

**Decided by the human** (Q4, 2026-10-14): `Component` nodes are part of the strict tree:
`Repo → Component* → Package → Component* → File → Symbol`. Clustering never splits a package.
At the top it groups whole packages; inside a package it groups files and directories. The
Phase 2 ADR records this.

**Still open as of Phase 0:**

- the reliability ranking between sources;
- the semantics of `working_tree` snapshots (one rolling snapshot, or one per save);
- the similarity threshold for move detection;
- the migration tool (`rusqlite_migration` or `refinery`, both verified to exist).

## Update by difference

Re-indexing a file computes its new set of nodes and edges, compares it with the currently
valid set for that file, and then:

- closes the intervals of what disappeared;
- opens intervals for what appeared;
- leaves the rest untouched.

Re-applying an identical file must produce **zero row changes**. Test it.

## Writing a SQLite migration

1. Never edit a committed migration: add a new one.
2. One numbered file per migration, `NNNN_short_description.sql`. The location and runner are
   fixed by the Phase 2 ADR.
3. One migration is one transaction, and migrations only move forward. The database is a
   rebuildable cache: an incompatible change may drop tables and trigger a full reindex, as
   long as the migration says so.
4. Every table holding graph facts has `valid_from` / `valid_to`, plus indexes that make the
   snapshot-diff queries cheap.
5. Add a test that applies all migrations to an empty database and to a fixture of the
   previous schema, then runs the diff queries.
6. If a concept of the schema changes, update this skill in the same change.

## Test checklist for any graph change

- insertion;
- update by difference, including the zero-change reapplication;
- snapshot diff;
- file move and symbol move detection;
- fusion precedence;
- strict-tree invariant (property test with `proptest`);
- serde round-trip of every public type.
