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

## Granularity of `Calls` edges (spec, « Technical decisions »)

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
- **Review diff** (ADR 0024, `timonier_core::review`): between the last `review_mark` and the
  head, at the level of boxes (components and packages), under the head's ids. The content
  hash of each file at the review is kept in `snapshot_content` (migration 0005), for the last
  review only, and for a session's snapshots until it ends (ADR 0031,
  `Store::freeze_keeping_contents`, `forget_contents`, read by `Reader::kept_contents`): a
  session's diff is the review diff between its two snapshots, written in its journal. `timonier_core::graph::Graph` is the in-memory snapshot both the views and the
  review diff read.
  - Renames: a file gone from one path and found at one other with the same content is moved,
    with its symbols (paired by their names from the file down and their span); so are the
    symbols of an unchanged file whose ids changed (it changed crate). A box gone and a new
    box of the same kind are one when each holds the majority of the other's files.
  - Boxes (Q30, Q33, Q34, Q35): only code changes count, never a regrouping. A box is added
    when it brings a new file or holds none (external packages); removed when one of its files
    was deleted or it held none; otherwise modified when it holds a changed or moved file, at
    the review or now. A package comes and goes with its manifest (Q35): new is added, gone is
    removed, unless renamed. A package renamed, or one that lost or gained all its files
    (internal ↔ external), is modified. A package new, gone or changed by its manifest
    (`ReviewDiff::by_manifest`) counts for the boxes above it, as a file does, and its files
    as moved ones for the boxes they leave or join (Q37).
  - `ReviewDiff::regrouped`: boxes a regrouping alone made or undid (new and not added, gone
    and not removed).
  - Dependencies between sibling boxes compare the edges themselves, under the head's ids
    (Q36, refining Q28): a dependency is added or removed when one of its edges appears or
    goes and touches an edited, new or deleted file (a move is no edit; manifest edges always
    count), or exists at both ends but carries another dependency because an end changed
    box: a file moved to another folder (one renamed in its folder does not move, Q40), a file
    of a package its manifest made, undid or turned internal ↔ external, or that package
    itself (a renamed package stays the same box). Each end counts for its own move (Q41),
    when, at the dependency's level, neither the box it leaves nor the one it joins is a
    regrouping (Q38), and, its package staying, they are not above that package
    (`Edges::end_moves`, `moved_dependencies`). A regrouping never changes a dependency. The
    map's new links follow the same rule. Known gap (Q39): a file regrouped at a commit and
    edited to call another file of the same box lists a dependency removed and one added.
  - Cycles added (Tarjan, no review cycle holds them), looked for in the head's boxes as they
    are, without what the language server alone brought, and counted only when a dependency
    added closes them; fan-in and fan-out jumps (+3 and +50 %), for boxes present at both
    ends, on the review's dependencies with those added and without those removed.
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

## Decisions of Phase 3 (ADR 0012 to 0018)

- **Who produces what** (ADR 0013): `timonier-index` turns a repository into scope updates.
  The structure scope holds the repo, components, packages (internal ones that own a file, and
  runtime dependencies), files, and the manifests' `DependsOn`. Each file scope holds its
  symbols and its tree-sitter edges: file → file `Imports`, `Calls`, `References`, and symbol →
  symbol `Implements`/`Inherits`.
- **Qualified names** (ADR 0013): `rust:<crate>::<module path>::<items>`, with target prefixes
  no module can take (`<crate>::@tests::<name>`, `<crate>::@bin::<name>`, `@examples`,
  `@benches`, `@build`); `typescript:<package>/<path>#<A.b>`. Only `timonier_index::project`
  builds them.
- **LSP** (ADR 0014): `Lsp` observations are function → function `Calls`, `verifying([Calls])`,
  `computed_on` the hash read at request time. Beyond 50 000 of them (ADR 0018), the oldest
  verified files keep `Lsp` file → file `Calls`, still verified.
- **Commit snapshots** (ADR 0015) describe the committed tree: the index is first brought in
  line with the disk, walk included (what an ignore file skips now goes); dirty files take their
  HEAD version for the freeze; then the working tree comes back in two steps (ADR 0022): first
  as the index held it, from memory, with the LSP verification the freeze dropped; then the
  disk, read as any change. The freeze also takes the commit's packages (ADR 0025): its
  manifests and Cargo libraries over the disk; every file placed again under them, its LSP
  verification out for the freeze and back after.
- **Components** (ADR 0016): keys `dir/<path>`, `cluster/<smallest member path>`, `external`,
  `external/<category>`. Clustering happens at a commit, at the first indexing, and when
  packages change; a restart takes the head's components back as they are. Leiden is
  idempotent, and only components that hang from the repo or an existing package are emitted.
- **Roles** (ADR 0017): `Node::role` on packages and components, versioned (a changed role is
  a modification). Migration 0003.
- **Extraction cache** (ADR 0013, migration 0002): table `extraction`, opaque JSON for the
  store, keyed by content hash, language, pack version and extractor version. Not graph facts:
  no `valid_from`/`valid_to`.
- **Refinement debt** (ADR 0022, migration 0004): tables `refinement_dependent`,
  `refinement_lookup` and `refinement_lookup_citer`, what the LSP refinement still owes (stale
  verified files, lookups of dependents), written by difference, with the batch that changes
  it, read back at start. Not graph facts either.

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
