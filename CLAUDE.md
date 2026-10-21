# Timonier

## Vision

Timonier lets a developer steer AI coding agents without reading all the code they produce.
A local Rust daemon, one per project, maintains a unified code graph (tree-sitter + LSP, SCIP later) as the single source of truth.
A VSCode extension shows the living architecture map, the whiteboard of intended architecture, performance measured during tests, and the live plan and todo.
Agents read and write the same model through MCP tools: sessions, plan, todo, intent, budgets.
The developer becomes architect and reviewer: what I want, what exists, how it behaves, what is happening.

## Sources of authority

- **[docs/spec.md](docs/spec.md)** is authoritative. When in doubt, the spec wins. If the spec contradicts itself or is
  silent, **stop and ask the human**; never invent.
- **The human's answers recorded in `PLAN.md`** (« Questions ouvertes », marked ✅) override the spec on the points
  they cover. Example: Q3 rules out tldraw, although the spec names it. Do not edit `docs/spec.md`: it is the human's
  document.
- **`.timonier/plan.json`**: since the switch (ADR 0035, Q11), the milestones (one per phase) and the tasks with
  their state, kept only through Timonier's MCP tools and shown in the IDE. Never edit it by hand.
- **[PLAN.md](PLAN.md)**: phases, each task's acceptance criterion and detail (no state), open questions (Q…), working
  assumptions (H…), verified dependency versions. The SessionStart hook shows the current phase.
- **[docs/adr/](docs/adr/README.md)**: every technical decision not covered by the spec.

## Repository map

| Path                         | Content                                                          | Status   |
| ---------------------------- | ---------------------------------------------------------------- | -------- |
| `docs/spec.md`               | Product specification (French)                                   | exists   |
| `docs/adr/`                  | Architecture decision records (French), index in `README.md`     | exists   |
| `PLAN.md`                    | Plan and progress (French); task states are in `.timonier/plan.json` | exists   |
| `AGENTS.md`, `.mcp.json`, `.timonier/` | Timonier's own setup by `timonier init` (ADR 0033): the agents' managed block, the MCP server, the tracking (plan, todo, sessions), conventions | exists |
| `.claude/settings.json`      | Project hooks and permission rules (shared)                      | exists   |
| `.claude/hooks/`             | Hook scripts (Node.js, no dependencies) and their tests          | exists   |
| `.claude/skills/<name>/`     | Project skills                                                   | exists   |
| `.claude/agents/`            | Project subagents                                                | exists   |
| `Cargo.toml`, `crates/`      | Cargo workspace: `timonier-{core,store,index,daemon,otlp,mcp,cli}`; dependency graph in ADR 0003. `timonier-core` (graph model, fusion, diff, aggregation, roles; protocol types and JSON-RPC framing since Phase 4; in-memory graph and review diff since Phase 5; tracking formats and the MCP tool catalogue since Phase 6, snapshot in `tests/tools.json`; the runs' types (`perf.rs`), the instrumentation gaps (`gaps.rs`, ADR 0049), the budgets checked on the runs (`budgets.rs`, ADR 0052) and the regressions (`regression.rs`, ADR 0053) since Phase 8; the whiteboard's `intent.json` format since Phase 7, ADR 0039) and `timonier-store` (SQLite history, single writer, extraction cache, refinement debt; `migrations/`, `examples/bench.rs`) since Phase 2; `timonier-index` (language packs in `packs/`, extraction, resolution, components, LSP refinement) and `timonier-daemon` (pipeline, watcher, commit consolidation; `examples/index_bench.rs`, `examples/index_dump.rs`) since Phase 3, plus its JSON-RPC server on a Unix socket, views, LSP relay and blocking client (Phase 4, ADR 0019, 0022; `examples/view_bench.rs`, `examples/view_snapshot.rs`, `examples/lsp_cost.rs`, `examples/commit_bench.rs`; `examples/support/synthetic.rs` writes the benches' synthetic workspaces); the daemon also runs the MCP tools and writes `.timonier/` (`tracking.rs`, ADR 0027 to 0029); `mcp` is the stdio proxy (rmcp); `cli` has `timonier daemon start|stop|status|run`, `timonier mcp`, `timonier init|uninit|doctor`, `timonier hook stop|pre-commit` (`src/init/`, ADR 0033, 0034; the packs' `[[tests.runners]]` and `conventions/*.md`) and `timonier test` with its measuring relay `__measure` (`src/test.rs`, `src/measure.rs`, ADR 0045, 0046), its own telemetry behind the feature `otel` (`src/telemetry.rs`, ADR 0044), the packs' `[instrumentation]` and smoke test written by `init` and run by `doctor` (ADR 0050); the daemon keeps the runs in `.timonier/perf/perf.sqlite` (`runs.rs`, `run/start` and `run/finish`; `timonier-store`'s `perf` module and `migrations/perf/`, ADR 0043, 0047); `otlp` is the OTLP receiver (gRPC and HTTP on 4317/4318 or free ports, `decode.rs`, `receiver.rs`, ADR 0048), started by the daemon at the first run, whose spans the daemon attaches to tests and nodes (`attach.rs`) | in progress |
| `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml` | Toolchain (stable), formatting (edition 2024), clippy settings (ADR 0004) | exists |
| `package.json`, `tsconfig.base.json` | npm workspaces root, shared TS settings, lint/format tooling (ADR 0005) | exists |
| `eslint.config.mjs`, `.prettierrc.json` | Typed ESLint (TS 6.0.x), Prettier (Markdown excluded)        | exists   |
| `webview/`                   | React + Vite webview, shared by future front-ends. `src/tokens.css` = design tokens, `src/design/` = components (ADR 0020), `src/map/` = architecture map with React Flow and elkjs (ADR 0021), `src/tracking/` + `tracking.html` = panels of the plan, todo and sessions, a second page built apart into `dist/tracking/` (ADR 0031), `src/whiteboard/` + `whiteboard.html` = the whiteboard (React Flow + perfect-freehand), a third page built into `dist/whiteboard/` (ADR 0036, 0037), `src/performance/` + `performance.html` = the Performances panel (timeline, call tree, evolution; uPlot), a fourth page built into `dist/performance/`, and `src/map/perf.ts` the map's Performances layer (ADR 0051), `demo.html` = development page (`?tracking=all&width=…`); `test/a11y` (Playwright + axe-core), `test/production` (the whiteboard's production build under the webview CSP, built by Playwright's global setup), `test/themes` (captured VSCode themes), `test/views` (Timonier's views from `view_snapshot`) | in progress |
| `editors/vscode/`            | VSCode extension: activity-bar container with the « Overview » view, the bridge (ADR 0007, 0032, icon in `media/`), daemon supervisor and JSON-RPC client, view relay, LSP relay that answers about saved content only, map panel (ADR 0019, 0022), whiteboard panel (ADR 0037), Performances panel and the map's layer feed (ADR 0051), a « Plan, todo and sessions » webview view in a secondary-side-bar container, fed by one tracking subscription (ADR 0031), esbuild bundle, CSP webview panel, `node:test` unit + `@vscode/test-cli` integration tests | in progress |
| `.vscode/`                   | `launch.json`: « Run Extension » (F5) and « Extension Tests » (needs the recommended Extension Test Runner); `tasks.json`; `extensions.json` | exists |
| `LICENSE`, `README.md`, `README.fr.md`, `docs/features.md`, `docs/images/` | MIT licence (Nicolas Michel); README in English with its French translation (purpose, tour with screenshots, status, how to try it), the full feature list, and the screenshots `npm run screenshots -w webview` takes (`webview/test/screenshots/`); skill `readme` | exists   |
| `.github/workflows/ci.yml`   | CI on ubuntu-latest and macos-latest                             | exists   |

## Commands

Run from the repository root.

```sh
# Rust
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all                    # CI runs --check

# Node (webview + extension)
npm ci
npm run build                      # webview (Vite) then extension (esbuild)
npm test                           # unit tests: webview (vitest), extension (node:test), hooks (node:test)
npm run test:integration           # builds the daemon (cargo) and the extension, then runs it inside a real VSCode against a real daemon (needs a display; CI uses xvfb-run)
npm run test:a11y                  # WCAG AA of the design system (webview/demo.html) in every captured VSCode theme, and the whiteboard's production build under the CSP; needs `npx playwright install chromium`
node editors/vscode/scripts/capture-themes.mjs   # captures VSCode's theme variables into webview/test/themes (after `npm run build`)
npm run lint                       # ESLint, type-aware
npm run typecheck                  # tsc --noEmit for each workspace; the Stop hook runs it
npm run format                     # Prettier (format:check in CI)
npm run package                    # build + vsce package → editors/vscode/timonier.vsix

# CLI
cargo run --bin tim -- --version   # `timonier` and `tim` are the same program

# Baseline timings (record changes in PLAN.md)
cargo run --release -p timonier-store --example bench [packages] [files] [symbols]
cargo run --release -p timonier-daemon --example lsp_cost [repository]   # questions refinement asks the language servers (ADR 0022)
npm run bench:map -w webview       # map frames per second on synthetic repositories (PLAN.md 4.10)
npm run bench:whiteboard -w webview   # whiteboard with 1,000 and 3,000 strokes: loading, a stroke, an undo (ADR 0037)
npm run screenshots -w webview     # the README's images in docs/images/ (English, Dark Modern; after npm run build)
cargo run --release -p timonier-store --example debt_bench   # cost of a refinement debt write (ADR 0022)
cargo run --release -p timonier-daemon --example view_bench [repository|synthetic:CxDxF] [saves]   # RUST_LOG=timonier_daemon=debug for logs
cargo run --release -p timonier-daemon --example commit_bench [CxDxF] [runs]   # commit snapshot, same or other packages (ADR 0025)
cargo run -p timonier-daemon --example sketch [intent.json] [out.png]   # the whiteboard as the agent sees it (ADR 0040)

# Protocol types → TypeScript (a test fails when webview/src/protocol/generated.ts is stale)
TIMONIER_UPDATE_TS=1 cargo test -p timonier-core
# MCP tool catalogue snapshot and .timonier/ file schemas (docs/schemas/), checked the same way
TIMONIER_UPDATE_TOOLS=1 cargo test -p timonier-core

# MCP server for an agent, in a project (starts the daemon if needed)
cargo run --bin tim -- mcp [--root DIR]

# Run the tests measured (level 1, ADR 0045): the runners of .timonier/tests.json without a command
cargo run --bin tim -- test [--runs N|auto] [--without-traces] [--junit FILE] [--root DIR] [-- <command>]   # TIMONIER_MEASURE=rusage: no systemd scope

# Set a project up for its agents (ADR 0033), and take it back; never on a real home in tests
cargo run --bin tim -- init [--dry-run] [--yes] [--agents claude,copilot,cursor,codex] [--root DIR]
cargo run --bin tim -- uninit [--dry-run] [--purge [--yes]] [--root DIR]   # keeps the tracking unless --purge (Q54)
cargo run --bin tim -- doctor [--fix | --check] [--no-smoke] [--root DIR]

# Indexing timings on a repository (default: this one, copied to a temp dir), and its graph
cargo run --release -p timonier-daemon --example index_bench [repository] [saves]
cargo run --release -p timonier-daemon --example index_dump [repository]
cargo test --release -p timonier-daemon --test compaction -- --ignored   # load test
```

## Working rules

1. **Plan first.** Work follows the plan: `.timonier/plan.json` for the tasks and their state, `PLAN.md` for their
   criteria and detail. A task is done when its acceptance criterion is verified. Work through Timonier's MCP tools
   (the managed block of `AGENTS.md`): one session per task (`session_start` … `session_end`), its todo, and
   `plan_update_task` when a task changes state.
2. **One phase at a time.** At the end of a phase:
   - tests are green;
   - the tasks are `done` in `plan.json` (`plan_update_task`), and `PLAN.md` is updated (`Écarts :` and
     `Décisions :` filled, each task's detail);
   - the `phase-reviewer` subagent has reviewed the phase;
   - you have written a short summary: done, decided, remaining.

   Then **stop and wait for the human's go-ahead**.
3. **Decisions are recorded.** A technical decision not covered by the spec gets an ADR (skill `adr`). A decision that
   changes product behaviour is a question for the human, not an ADR.
4. **Invent nothing.** Before using a crate, an npm package or an API:
   - verify it exists, and check its current version and licence (crates.io, npm, official docs);
   - read the docs or the source for every signature you call.

   `PLAN.md` lists what was verified, and when.
5. **Quality.**
   - Language: code, identifiers, comments and commit messages in English; user documentation in French, except
     `README.md`, in English at the human's request (2026-10-16), with `README.fr.md` beside it (2026-10-21). Files read
     by the human to steer the project (`PLAN.md`, ADRs, review reports) are in French. Agent-facing files
     (`CLAUDE.md`, skills, subagents, hook messages) are in English.
   - Commits follow Conventional Commits. **Commit only when the human asks.**
   - Tests: unit tests for every module, integration tests for every boundary (daemon ↔ extension, daemon ↔ MCP).
   - Linters: `cargo clippy -- -D warnings` and the TypeScript lint are clean.
6. **Design.** Every UI follows « Expérience et design » of the spec: calm, clear, IDE theme, WCAG AA. Skill
   `vscode-webview-ui`.
7. **The whiteboard has a single author, the human.** Never write `.timonier/intent.json`. A hook and a permission rule
   enforce it.

## Skills

| Skill                  | Load it when                                                                          |
| ---------------------- | ------------------------------------------------------------------------------------- |
| `rust-daemon`          | Writing or reviewing any Rust code or `Cargo.toml`                                    |
| `graph-model`          | Touching graph types, the SQLite schema, migrations, snapshots, diffs, fusion         |
| `language-pack`        | Adding a language, changing extraction, role heuristics or per-language init          |
| `vscode-webview-ui`    | Any work in `editors/vscode/` or `webview/`, any view or visual decision              |
| `mcp-tools`            | Adding or changing an MCP tool                                                        |
| `otel-instrumentation` | Instrumenting code, `timonier-otlp`, `timonier test`, instrumentation gaps            |
| `adr`                  | Recording a technical decision                                                        |
| `readme`               | Writing or updating README.md, README.fr.md, docs/features.md or their screenshots    |

## Subagents

- `phase-reviewer`: independent, read-only review of a finished phase against the spec, `PLAN.md` and the ADRs. Run it
  before asking for the go-ahead. Its report is in French.

## Hooks

Configured in `.claude/settings.json`. Scripts are in `.claude/hooks/`, documented in
[ADR 0001](docs/adr/0001-hooks-claude-code-en-node.md). Per-session state lives in `.claude/state/` (gitignored).

| Event              | Script                | Behaviour                                                                                         |
| ------------------ | --------------------- | ------------------------------------------------------------------------------------------------- |
| `SessionStart`     | `session-start.mjs`   | One line for the human (current milestone of `.timonier/plan.json`, tasks done/remaining; `PLAN.md`'s phase without it); the remaining tasks for Claude. |
| `UserPromptSubmit` | `turn-start.mjs`      | Records when the turn started. Prompts injected mid-turn do not move it.                          |
| `PreToolUse`       | `protect-intent.mjs`  | Denies `Edit`/`Write`/`NotebookEdit` on `.timonier/intent.json`, and Bash commands that would write it (best effort). Fails closed. |
| `PostToolUse`      | `format-file.mjs`     | Formats the touched file: `rustfmt` for `.rs`; `prettier` from `node_modules` for every type the CI checks (TS/JS, JSON, CSS, HTML, YAML; Markdown is in `.prettierignore`). Skipped if `npm ci` has not run. |
| `Stop`             | `stop-checks.mjs`     | Finds the files changed during the turn (git status + mtime). Rust changed → `cargo check`; TS changed → `npm run typecheck`. A failure blocks the stop once. Code changed but `PLAN.md` untouched → one non-blocking reminder. |

Testing by hand: pipe the JSON that Claude Code would send and read stdout. Run these from the repository root, after
`npm ci`. Each block prints what its comment announces.

```sh
export CLAUDE_PROJECT_DIR="$PWD"

# Session start → {"systemMessage": "Timonier · Phase …", "hookSpecificOutput": {"additionalContext": …}}
echo '{"session_id":"manual","source":"startup"}' | node .claude/hooks/session-start.mjs

# Intent guard → a "deny" decision; a read-only Bash command prints nothing (allowed)
echo '{"tool_name":"Write","tool_input":{"file_path":"'"$PWD"'/.timonier/intent.json"}}' | node .claude/hooks/protect-intent.mjs
echo '{"tool_name":"Bash","tool_input":{"command":"cat .timonier/intent.json"}}' | node .claude/hooks/protect-intent.mjs

# Formatter → prints the formatted Rust, then the formatted TypeScript
printf 'fn  main( ){}\n' > scratch.rs && echo '{"tool_input":{"file_path":"'"$PWD"'/scratch.rs"}}' | node .claude/hooks/format-file.mjs; cat scratch.rs; rm scratch.rs
printf 'export const  x = {a:1}\n' > webview/src/scratch.ts && echo '{"tool_input":{"file_path":"'"$PWD"'/webview/src/scratch.ts"}}' | node .claude/hooks/format-file.mjs; cat webview/src/scratch.ts

# End of turn, type error → first stop: {"decision":"block", "reason": "…TS2322…"}; second stop: a systemMessage only
echo '{"session_id":"manual"}' | node .claude/hooks/turn-start.mjs; sleep 1
printf 'export const n: number = "x";\n' > webview/src/scratch.ts
echo '{"session_id":"manual","stop_hook_active":false}' | node .claude/hooks/stop-checks.mjs
echo '{"session_id":"manual","stop_hook_active":true}' | node .claude/hooks/stop-checks.mjs
rm webview/src/scratch.ts .claude/state/manual.json

# End of turn, code changed but PLAN.md untouched → {"hookSpecificOutput": {"additionalContext": "…PLAN.md was not modified…"}}
echo '{"session_id":"manual"}' | node .claude/hooks/turn-start.mjs; sleep 1; touch .claude/hooks/lib.mjs
echo '{"session_id":"manual","stop_hook_active":false}' | node .claude/hooks/stop-checks.mjs
rm .claude/state/manual.json
```

In a session: `/hooks` lists the registered hooks, `/skills` the skills, `/agents` the subagents. `claude --debug`
shows hook input and output.

## Resuming work

1. Read the SessionStart line, then the current phase section of `PLAN.md`, its open questions and its assumptions;
   `get_plan` gives the tasks' state.
2. Read the ADRs that the phase references, and the skills for the area you are about to touch.
3. If the phase is finished but not validated, do not start the next one: summarise and wait for the human.

<!-- timonier:begin -->
@AGENTS.md
<!-- timonier:end -->
