---
name: rust-daemon
description: Conventions for all Rust code in Timonier's Cargo workspace (crates/timonier-*) — crate layout and dependency direction, error handling with thiserror/anyhow, async with tokio, the single SQLite writer in WAL mode, logging with tracing, tests, lints and how to add a dependency. Use before writing, modifying or reviewing any .rs file or Cargo.toml in this repository.
---

# Rust daemon conventions

Sources of truth: `docs/spec.md` (« Architecture technique », « Indexation et modèle de graphe »,
« Décisions techniques ») and the ADRs in `docs/adr/`. This skill explains how to apply them.
If it disagrees with an ADR, the ADR wins and this file must be fixed in the same change.

## Workspace layout

One Cargo workspace, crates under `crates/`. Responsibilities and internal dependencies
(ADR 0003; a dependency outside this table needs a new ADR):

| Crate             | Responsibility                                                                 | Depends on (internal)          |
| ----------------- | ------------------------------------------------------------------------------ | ------------------------------ |
| `timonier-core`   | Graph model types, stable identity, fusion rules, protocol types. Pure: no I/O, no tokio. | —                   |
| `timonier-store`  | SQLite schema, migrations, snapshots, diff queries.                            | core                           |
| `timonier-index`  | tree-sitter extraction, language packs, LSP access, components and roles.      | core                           |
| `timonier-otlp`   | Local OTLP receiver (gRPC 4317 / HTTP 4318).                                   | core                           |
| `timonier-daemon` | File watcher, work queue, JSON-RPC 2.0 server; wires the other crates together.| core, store, index, otlp       |
| `timonier-mcp`    | MCP server: a thin proxy to the daemon over its socket, no business logic, no storage access. | core            |
| `timonier-cli`    | The `timonier` binary and its `tim` alias (two `[[bin]]` targets calling `timonier_cli::run`). | core, daemon, mcp |

Rules:

- Dependencies point towards `timonier-core`; no cycles.
- Only `timonier-daemon` (and the CLI that launches it) owns I/O orchestration.
- `timonier-core` stays synchronous and deterministic, so it can be unit- and property-tested.
- Shared versions live in `[workspace.dependencies]` of the root `Cargo.toml`; crates use
  `dep = { workspace = true }`. Edition 2024, `rust-version = 1.96` (ADR 0004), checked by the
  CI `msrv` job.
- Workspace lints: `unsafe_code` forbidden; clippy `unwrap_used`, `dbg_macro`, `todo` warn (errors
  under `-D warnings`); `unwrap`/`expect` allowed in `#[test]` code only (`clippy.toml`). Test
  helpers outside `#[test]` functions use `expect("why")`.

## Errors

- Library crates: one `Error` enum per crate, derived with `thiserror`, plus
  `pub type Result<T, E = Error> = std::result::Result<T, E>;`. Variants carry structured
  context (path, node id, snapshot seq), not pre-formatted strings.
- The binary and top-level task boundaries use `anyhow::Result` with `.context(...)` /
  `.with_context(|| ...)`.
- No `unwrap()` outside tests. `expect("…")` only for true invariants, with a message that
  says why the failure is impossible.
- Nothing panics across the JSON-RPC or MCP boundary: errors become protocol error objects
  with stable codes and an actionable message.

## Async with tokio

- One multi-threaded runtime, created in `main`.
- Never block the runtime. CPU-heavy work (tree-sitter parsing, clustering) and blocking I/O
  (rusqlite) run on `tokio::task::spawn_blocking` or on a dedicated thread.
- Bounded channels only (`tokio::sync::mpsc::channel(n)`): back-pressure is intended.
- Every long-lived task has a cancellation path and is awaited at shutdown.
- Never hold a `std::sync::Mutex` guard across an `.await`.

## SQLite: a single writer in WAL mode

- Exactly one writer: a dedicated thread owns the only read-write connection. Every write is a
  command sent to it over a channel and applied in a transaction (batched per file update or
  per indexing batch).
- Pragmas set at open: `journal_mode=WAL`, `foreign_keys=ON`, a `busy_timeout`; the
  `synchronous` level is fixed by the Phase 2 ADR.
- Readers use separate read-only connections and never write.
- Schema changes only through migrations (see the `graph-model` skill).
- The database is a rebuildable cache of the repository. User-owned data lives in
  `.timonier/*.json`, never only in SQLite.

## Logging

- `tracing` only. No `println!` / `eprintln!` in the daemon or the MCP server: stdout may carry
  a protocol (MCP over stdio).
- Structured fields, not interpolated messages:
  `tracing::info!(file = %path.display(), elapsed_ms, "file reindexed")`.
- Levels: `error` needs the human; `warn` means degraded mode; `info` is lifecycle; `debug` is
  per operation; `trace` is hot paths and off by default.
- Spans follow the `otel-instrumentation` skill (one span per component boundary, exporter
  behind a feature).

## Tests

- Unit tests next to the code (`#[cfg(test)] mod tests`); integration tests in
  `crates/<crate>/tests/`.
- Every boundary (daemon ↔ extension, daemon ↔ MCP) has integration tests that start a real
  daemon on a temporary socket and a temporary project.
- Tests use temporary directories and never touch the developer's `.timonier/`.
- A performance claim (for example "update in < 100 ms") ships with the measurement that
  backs it (a bench or a test that records timings), and the figure goes into `PLAN.md`.

## Done means

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Adding a dependency

1. Check crates.io: the crate exists, its current version, its licence (compatible with the
   project licence) and its maintenance. Read its docs for every function you call: no
   guessed signatures.
2. Add it to `[workspace.dependencies]`, with minimal features.
3. A structuring choice (framework, protocol, storage, parser) gets an ADR (`adr` skill).
