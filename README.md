# Timonier

Timonier lets a developer steer AI coding agents without reading all the code they produce.
When an agent writes the code, the bottleneck is no longer writing it but understanding it: the
code changes faster than the developer's mental model of it. Timonier keeps that model for you,
and shows it.

It aims to answer four questions at any time:

- **What I want**: the intended architecture, drawn freely on a whiteboard.
- **What exists**: the real architecture, the technologies, the health of the code.
- **How it behaves**: execution time and resources, measured while the tests run.
- **What is happening**: the plan, the live todo, what the agent did and where to look.

A local Rust daemon, one per project, keeps a unified graph of the code. The VSCode extension
shows it as a living architecture map. Agents will read and write the same model through MCP.

The name: the *timonier* is the helmsman, who holds the wheel and keeps the course while the
crew, here the agents, move the ship.

## Status

**Early development, not released.** The MVP is planned in nine phases ([PLAN.md](PLAN.md)):
phases 0 to 5 are done (up to the review diff on the map). Phase 6 is under way: the MCP tools,
the plan, todo and session journal, the review debt and the bridge work; `timonier init` and
`timonier doctor` come next. Nothing is published on crates.io, npm or the VSCode Marketplace yet. It runs on Linux and macOS; Windows
is not supported yet.

## What works today

### Code graph

- **Indexing with tree-sitter** for Rust and TypeScript (including TSX), through versioned
  language packs. Symbols, imports, calls and references, with a stable identity for each symbol
  (crate or package, path, qualified name) and a hash of its normalised body.
- **Incremental updates**: a file watcher with a 300 ms debounce re-indexes what changed, with
  an extraction cache. On Timonier itself: under half a second from scratch, about 10 ms (p95) per
  save after the debounce.
- **Calls and usages verified by the IDE's language servers**: tree-sitter resolution is
  approximate, so the extension relays VSCode's language servers (rust-analyzer, TypeScript…)
  to the daemon, for the files shown on the map or open in the editor: the call hierarchy of
  each function, the definition of each type or name used from another file, and the users of
  a symbol whose signature changed. One question per path, not per site: on Timonier, about
  970 for 6,400 usage sites, one round trip per file. A doubtful answer replaces nothing, and
  nothing is verified from unsaved changes: the language servers answer about the open
  buffers, so the daemon waits until they match the files on disk. Each edge keeps its
  provenance and confidence.
- **Components and roles**: packages are split into components by directory, then by graph
  clustering (Leiden) when a level is too large. Each package and component gets an
  architectural role: entry point, interface, data model, persistence, background task, utility,
  external libraries. External dependencies are grouped by use.
- **History**: the graph lives in SQLite with a snapshot per commit and a working-tree head. A
  diff between two snapshots is a query, and it detects moved symbols instead of reporting
  « deleted, then added ». A commit's snapshot describes the committed tree, its packages
  included, even when the working tree differs.

### Daemon and CLI

- One daemon per project, found through a lock and a Unix socket in a private runtime directory.
- JSON-RPC 2.0 protocol: views subscribed to by clients, versioned deltas, a full snapshot when a
  client misses one. The daemon stops on its own after 15 minutes without clients.
- `timonier daemon start | stop | status | run` (also installed as `tim`).

### MCP tools for agents

- `timonier mcp` (or `tim mcp`) is an MCP server on stdio for coding agents (Claude Code, Codex,
  Cursor…). It starts the project's daemon if needed and relays its tools; the daemon validates
  every call and is the only writer of `.timonier/`.
- Tracking tools: `session_start`, `session_end`, `plan_add_task`, `plan_update_task`,
  `todo_add`, `todo_complete`, and `get_plan`, `get_todo`, `get_session` to read them back.
  Writes are structured operations, never whole files: a task follows its states (todo, in
  progress, blocked, done, dropped) and a forbidden transition is refused with what to do
  instead.
- The plan, the todo and the session journal are versioned JSON files in `.timonier/`
  (`plan.json`, `todo.json`, `sessions/`), their schemas in [docs/schemas](docs/schemas). Each
  session freezes the code graph at its start and end.
- Every result starts with a mailbox header (unread messages and their priority), empty until
  multi-agent messaging arrives.
- When a session ends, its journal keeps what it changed in the code: the components it touched
  and the dependencies it added or removed, computed as the review diff between its two
  snapshots.

### VSCode extension

- Launches the project's daemon, and launches it again if it dies (backoff from 1 s to 30 s).
  Its log is in the « Timonier » output channel.
- **Architecture map**:
  - it opens on a few boxes named by role, each with a title bar in the role's colour.
    Timonier itself fits in six boxes;
  - a click or Enter opens a role on its packages, then a package on its components, files and
    symbols. An open box becomes a frame around its contents;
  - edges are dependencies and calls. As in a node editor, they leave a box on its right and
    enter the other on its left. Edges known only from tree-sitter are dashed. Edges declared
    in a manifest (`Cargo.toml`, `package.json`) or confirmed by a language server are solid.
    Pointing at a box colours its edges;
  - the layout stays stable when the code changes: on Timonier, unchanged boxes move less than
    5 px on average when a file is added, and not at all when a call is. Opening a box makes
    room to its right, and closing it puts everything back;
  - the layout is computed, but any box can be moved by hand, with the mouse or Alt and an
    arrow key. It stays where it was left; « Reset the layout » puts it back;
  - beyond 500 boxes or 600 edges, the deeper levels are folded, so that large maps stay fluid.
    A synthetic map of 6,000 nodes pans at 60 frames per second in headless Chromium;
  - **Go to the code** (Shift+Enter, double click) opens the file beside the map, at the symbol;
  - **review diff**: « Mark as reviewed » freezes the map as you reviewed it. From then on, a
    banner above the map says what changed since: new, modified and removed boxes, dependencies
    added and removed, cycles added, fan-in or fan-out jumps; « Details » lists them. On the
    map, a new box has a star and the word « New », a modified one a dot and « Modified », a
    closed role says « 2 modified »; new links and links in a new cycle are marked, and the
    legend explains the marks. « Highlight the changes » dims the rest. Only code changes
    count: the edges themselves are compared, so a link that only a language server's check
    made appear or disappear, or files that Timonier merely regrouped into other components,
    do not. A box is new or removed only when code comes or goes with it, a crate with its
    manifest; a moved file, a renamed folder or crate is modified, and its dependencies follow
    it;
  - **review debt**: a toolbar toggle shows, on each box, how many of its files changed since
    your last review and are still to read again (« 9 files to read again »), in three levels of
    the attention tone; files changed with no agent session open are hatched and marked
    « untracked »;
  - **density**: the `timonier.map.density` setting makes the map comfortable (the default) or
    compact, with smaller boxes and tighter gaps;
  - keyboard navigation (Tab, arrow keys, Enter, Home, + and - to zoom) and a command-palette
    command for each action.
- **The bridge**, the home view on the left, answers three questions at a glance: where the plan
  stands (current milestone, open session), what happened since your last review (boxes,
  dependencies, agent sessions, files to read again), and where to look first: what needs a
  decision (a blocked task, an unreadable file, a cycle added), then the boxes to read again,
  untracked ones first. A click opens the map on the box.
- **Plan, todo and sessions** in one view of the secondary side bar, on the right (two glued
  columns when it is wide enough), updated live as the agent calls
  its tools: milestones with their computed progress and tasks by state; the open session's
  todo; the journal of sessions with their objective, tasks, components touched (live while the
  session is open), todo and summary. A file of `.timonier/` edited by hand shows too, and one
  that no longer reads is named.
- Follows the IDE theme, light, dark and high contrast. WCAG AA contrast is checked automatically
  with axe-core in six built-in VSCode themes. The interface is in English and French.

## Planned

From the [specification](docs/spec.md) and [PLAN.md](PLAN.md), in order:

- **Project setup** (phase 6, in progress): `timonier init` and `timonier doctor` configure a
  project and its agent.
- **Whiteboard of the intended architecture** (phase 7): free drawing, with shapes that can be
  typed as constraints, compared to the real architecture.
- **Performance** (phase 8): `timonier test` wraps the test runner, collects OpenTelemetry traces
  and resource use, and checks performance budgets written on the whiteboard.
- **End of the MVP** (phase 9): a real project steered end to end without reading the code line
  by line.

Later versions add SCIP indexing, more languages, multi-agent work and team use.

## Try it

Requirements: Rust stable (`rustup`), Node.js 24 or later, VSCode 1.140 or later.

```sh
npm ci
npm run build                          # the webview, then the extension
cargo install --path crates/timonier-cli   # puts timonier and tim on the PATH
```

Then open this repository in VSCode and press F5 (« Run Extension »). In the window that opens,
open a Rust or TypeScript project, click the Timonier icon (a ship's wheel) in the activity bar,
then **Open Map**. The command **Timonier: Open Map** in the palette does the same.

Instead of installing the CLI, you can build it with `cargo build -p timonier-cli` and set
`timonier.daemon.path` to `target/debug/timonier`.

The daemon writes its database to `.timonier/graph.sqlite` in the project, with a `.gitignore`
that keeps it out of git. From a terminal:

```sh
tim daemon start     # in the project, or with --root <dir>
tim daemon status
tim daemon stop
```

## Develop

```sh
# Rust
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all

# Webview and extension
npm test                   # unit tests: webview, extension, Claude Code hooks
npm run test:integration   # builds the daemon, then runs the extension in a real VSCode
npm run test:a11y          # WCAG AA in every captured VSCode theme (npx playwright install chromium)
npm run lint
npm run typecheck
npm run package            # editors/vscode/timonier.vsix
```

The specification, the plan and the architecture decision records are in French:
[docs/spec.md](docs/spec.md), [PLAN.md](PLAN.md), [docs/adr/](docs/adr/README.md).

## License

[MIT](LICENSE) © 2026 Nicolas Michel. Third-party notices ship with the extension.
