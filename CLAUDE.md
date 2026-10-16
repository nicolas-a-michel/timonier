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
- **[PLAN.md](PLAN.md)**: phases, tasks with acceptance criteria, progress, open questions (Q…), working
  assumptions (H…), verified dependency versions. The SessionStart hook shows the current phase.
- **[docs/adr/](docs/adr/README.md)**: every technical decision not covered by the spec.

## Repository map

| Path                         | Content                                                          | Status   |
| ---------------------------- | ---------------------------------------------------------------- | -------- |
| `docs/spec.md`               | Product specification (French)                                   | exists   |
| `docs/adr/`                  | Architecture decision records (French), index in `README.md`     | exists   |
| `PLAN.md`                    | Plan and progress (French)                                       | exists   |
| `.claude/settings.json`      | Project hooks and permission rules (shared)                      | exists   |
| `.claude/hooks/`             | Hook scripts (Node.js, no dependencies) and their tests          | exists   |
| `.claude/skills/<name>/`     | Project skills                                                   | exists   |
| `.claude/agents/`            | Project subagents                                                | exists   |
| `Cargo.toml`, `crates/`      | Cargo workspace: `timonier-{core,store,index,daemon,otlp,mcp,cli}`; dependency graph in ADR 0003. `timonier-core` (graph model, fusion, diff, aggregation, roles) and `timonier-store` (SQLite history, single writer, extraction cache; `migrations/`, `examples/bench.rs`) since Phase 2; `timonier-index` (language packs in `packs/`, extraction, resolution, components, LSP refinement) and `timonier-daemon` (pipeline, watcher, commit consolidation; `examples/index_bench.rs`, `examples/index_dump.rs`) since Phase 3; `otlp`, `mcp`, `cli` are skeletons | in progress |
| `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml` | Toolchain (stable), formatting (edition 2024), clippy settings (ADR 0004) | exists |
| `package.json`, `tsconfig.base.json` | npm workspaces root, shared TS settings, lint/format tooling (ADR 0005) | exists |
| `eslint.config.mjs`, `.prettierrc.json` | Typed ESLint (TS 6.0.x), Prettier (Markdown excluded)        | exists   |
| `webview/`                   | React + Vite webview, shared by future front-ends; `src/tokens.css` = design tokens | skeleton |
| `editors/vscode/`            | VSCode extension: activity-bar container with the « Overview » view (ADR 0007, icon in `media/`), esbuild bundle, CSP webview panel, `node:test` unit + `@vscode/test-cli` integration tests | skeleton |
| `.vscode/`                   | `launch.json`: « Run Extension » (F5) and « Extension Tests » (needs the recommended Extension Test Runner); `tasks.json`; `extensions.json` | exists |
| `LICENSE`, `README.md`       | MIT licence (Nicolas Michel); French README                      | exists   |
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
npm run test:integration           # builds, then runs the extension inside a real VSCode (needs a display; CI uses xvfb-run)
npm run lint                       # ESLint, type-aware
npm run typecheck                  # tsc --noEmit for each workspace; the Stop hook runs it
npm run format                     # Prettier (format:check in CI)
npm run package                    # build + vsce package → editors/vscode/timonier.vsix

# CLI
cargo run --bin tim -- --version   # `timonier` and `tim` are the same program

# Store baseline timings (record changes in PLAN.md)
cargo run --release -p timonier-store --example bench [packages] [files] [symbols]

# Indexing timings on a repository (default: this one, copied to a temp dir), and its graph
cargo run --release -p timonier-daemon --example index_bench [repository] [saves]
cargo run --release -p timonier-daemon --example index_dump [repository]
cargo test --release -p timonier-daemon --test compaction -- --ignored   # load test
```

## Working rules

1. **Plan first.** Work follows `PLAN.md`. A task is done when its acceptance criterion is verified.
2. **One phase at a time.** At the end of a phase:
   - tests are green;
   - `PLAN.md` is updated (tasks ticked, `Écarts :` and `Décisions :` filled);
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
   - Language: code, identifiers, comments and commit messages in English; user documentation in French. Files read
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

## Subagents

- `phase-reviewer`: independent, read-only review of a finished phase against the spec, `PLAN.md` and the ADRs. Run it
  before asking for the go-ahead. Its report is in French.

## Hooks

Configured in `.claude/settings.json`. Scripts are in `.claude/hooks/`, documented in
[ADR 0001](docs/adr/0001-hooks-claude-code-en-node.md). Per-session state lives in `.claude/state/` (gitignored).

| Event              | Script                | Behaviour                                                                                         |
| ------------------ | --------------------- | ------------------------------------------------------------------------------------------------- |
| `SessionStart`     | `session-start.mjs`   | One line for the human (current phase, tasks done/remaining); the remaining tasks for Claude.     |
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

1. Read the SessionStart line, then the current phase section of `PLAN.md`, its open questions and its assumptions.
2. Read the ADRs that the phase references, and the skills for the area you are about to touch.
3. If the phase is finished but not validated, do not start the next one: summarise and wait for the human.
