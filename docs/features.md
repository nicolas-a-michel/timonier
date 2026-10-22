# Timonier: what works today

The full list of what Timonier does on 2026-10-21 (Phase 8 of the MVP under way), with the figures
measured on Timonier itself. The [README](../README.md) gives the overview.

## Code graph

- **Indexing with tree-sitter** for Rust, TypeScript (including TSX), JavaScript (including JSX), Python, Go, Java, Kotlin, Scala, C, C++, C#, Swift and PHP, through versioned
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
  external libraries. External dependencies are grouped by use. In a folder of several
  projects, each top folder with its own git repository is a project box, with the roles inside
  it; developer tools (`xtask`, `tools/`, `vendor/`…) are never entry points.
- **History**: the graph lives in SQLite with a snapshot per commit and a working-tree head. A
  diff between two snapshots is a query, and it detects moved symbols instead of reporting
  « deleted, then added ». A commit's snapshot describes the committed tree, its packages
  included, even when the working tree differs.

## Daemon and CLI

- One daemon per project, found through a lock and a Unix socket in a private runtime directory
  (a named pipe on Windows). The home folder and `/` are never a project.
- JSON-RPC 2.0 protocol: views subscribed to by clients, versioned deltas, a full snapshot when a
  client misses one. The daemon stops on its own after 15 minutes without clients.
- `timonier daemon start | stop | status | run` (also installed as `tim`).
- `timonier init` sets a project up for Timonier and its agents (Claude Code, GitHub Copilot in
  VSCode, Cursor, Codex), the ones it finds on the machine or that `--agents` names:
  - a managed block in `AGENTS.md`, which the four agents read, and an `@AGENTS.md` import in an
    existing `CLAUDE.md`;
  - the MCP server `timonier` in each agent's own file (`.mcp.json`, `.vscode/mcp.json`,
    `.cursor/mcp.json`, `.codex/config.toml`);
  - a stop hook (Claude Code, Copilot, Codex): an agent that wants to stop with its session out
    of order (todo empty or not done, code changed with no session open) is held once, with the
    reason;
  - a git `pre-commit` hook that writes `.timonier/actual.json`, the boxes and dependencies of the
    code, into each commit;
  - a plan with an « Initialisation » milestone and a first session;
  - the test runner, set to write JUnit XML (cargo-nextest for Rust; vitest, `node --test` or
    jest for TypeScript and JavaScript; pytest for Python; gotestsum for Go; Maven or Gradle, their wrappers first, for Java; Gradle for Kotlin; sbt for Scala; CMake and CTest for C and C++; `dotnet test` for C#; `swift test` for Swift; PHPUnit for PHP), and the instrumentation conventions of each language in
    `.timonier/conventions/`.

  `--dry-run` shows the exact diff and changes nothing; a second run changes nothing.
  `timonier uninit` takes the setup back: init then uninit gives the original tree. Once work
  has been tracked, the plan, the todo and the journal stay (they are the project's);
  `--purge` removes them too, once confirmed.
- `timonier doctor` checks that setup (MCP server declared, managed block current, hooks and
  conventions present, `timonier` on the PATH, the daemon, the tracking files) and repairs what
  it can; `--check` only says it, by its exit status, for CI. It then runs the instrumented
  smoke test `init` wrote, under `timonier test`, and says whether its trace came back attached
  to the graph (`--no-smoke` skips it).
- For instrumentation, `init` adds the OpenTelemetry SDK between markers: optional dependencies
  and an `otel` feature in the root `Cargo.toml`, or development dependencies in
  `package.json`; it writes an initialisation module (`src/timonier_otel.rs`,
  `timonier.otel.ts`) and a smoke test. Timonier instruments itself the same way, behind its own
  `otel` feature (on by default).
- `timonier test -- <command>` runs the tests and measures them: wall time, CPU, peak memory
  and block I/O of the command and all its processes, and the duration of each test from the
  runner's JUnit XML. On Linux, a transient systemd user scope gives the memory of the whole
  process tree and a resource curve; on macOS, the largest process; on Windows, the wall time
  only (CPU and memory « not measured »), and runs are compared on it.
  Without a command, it runs the runners `timonier init` recorded; `--install` first installs
  a program a runner needs and the machine lacks (`cargo install cargo-nextest --locked` for
  Rust, `go install gotest.tools/gotestsum@v1.13.0` for Go). Before any run, the Performances panel offers « Run the tests »: it shows the commands,
  then runs them in a terminal. The tests get
  `OTEL_EXPORTER_OTLP_ENDPOINT` and `OTEL_RESOURCE_ATTRIBUTES` (`timonier.run_id`,
  `timonier.snapshot`), and each test of the JUnit XML becomes a root span of the run. The
  daemon receives their OpenTelemetry traces and metrics (OTLP over gRPC or HTTP, on 4317 and
  4318 or free ports) and attaches each span to its test and to its node of the graph by its
  `code.*` attributes. It keeps each run in `.timonier/perf/`, out of git.

## MCP tools for agents

- Development process (phase 9, under way): `.timonier/workflow.json` holds the process, from
  three presets (solo fast, the default; solo disciplined; team) or set one by one in
  « Timonier: Development Process ». With a branch per task, `task_start` opens the task's GitHub
  issue and branch `tim/<ticket>-<slug>` through `gh`; `open_pull_request`, or `session_end`,
  pushes the branch and opens its pull request with a written description (the task, the
  session's structural changes, the last run, regressions, budgets and forbidden edges), and asks
  GitHub's auto-merge when the process says so; the issue closes with the task. Choosing GitHub
  Actions writes `.github/workflows/timonier.yml`, which runs `timonier test` then
  `timonier check` (fails on an exceeded budget or a broken architecture rule).
- Technology inventory (phase 9, under way): `timonier inventory` and the tool
  `get_tech_inventory` list the languages, frameworks and dependencies with their resolved
  versions and licences (`cargo metadata`, `Cargo.lock`, `package-lock.json`), the duplicates by
  function (two HTTP clients), the dependencies added since the last review, and the known
  vulnerabilities from OSV.dev, asked on demand only and kept a day. `--sbom FILE` writes a
  CycloneDX 1.5 SBOM, which passes the official schema on Timonier.

- `timonier mcp` (or `tim mcp`) is an MCP server on stdio for coding agents (Claude Code, Codex,
  Cursor…). It starts the project's daemon if needed and relays its tools; the daemon validates
  every call and is the only writer of `.timonier/`.
- Tracking tools: `session_start`, `session_end`, `plan_add_task`, `plan_update_task`,
  `todo_add`, `todo_complete`, and `get_plan`, `get_todo`, `get_session` to read them back.
  Writes are structured operations, never whole files: a task follows its states (todo, in
  progress, blocked, done, dropped) and a forbidden transition is refused with what to do
  instead.
- Intent tools: `get_intended_architecture` gives the architecture the developer drew (layers,
  components, edges with their policy, notes, budgets, where each shape and stroke is) and an
  image of the sketch for the agent's vision; `get_drift` compares the code with it: each drawn
  component and the real package or component it stands for (by name, then place), the ones
  missing, the forbidden edges the code crosses and the required ones it lacks. The agent reads
  the whiteboard, never writes it.
- Budget and performance tools: `get_budgets` checks the performance budgets drawn on the
  whiteboard against the median of the last runs (latency p50 and p95 of each incoming call,
  peak memory, calls), `get_budget_violations` lists those exceeded, and `get_perf` gives what a
  component cost in a run.
- Instrumentation tools: `get_instrumentation_conventions` gives how the project instruments a
  language; `get_instrumentation_gaps` checks the last run of `timonier test`: the public
  functions no span was seen in, the spans that lack their `code.*` attributes or a node, and
  the overhead of the traces against a run made with `timonier test --without-traces`.
- The plan, the todo and the session journal are versioned JSON files in `.timonier/`
  (`plan.json`, `todo.json`, `sessions/`), their schemas in [docs/schemas](docs/schemas). Each
  session freezes the code graph at its start and end.
- Every result starts with a mailbox header (unread messages and their priority), empty until
  multi-agent messaging arrives.
- When a session ends, its journal keeps what it changed in the code: the components it touched
  and the dependencies it added or removed, computed as the review diff between its two
  snapshots.
- The AI cost of each session and task: the tokens Claude Code reports in its own OpenTelemetry
  telemetry (input, output, prompt cache, requests), counted while the session was open; the
  dollars are the agent's estimate. `timonier init` turns that telemetry on in
  `~/.claude/settings.json` when you agree (Claude Code reads it only from there), and sends it
  to `127.0.0.1:4319`. Without it, the session says « not measured ». Other agents are not
  measured yet.

## VSCode extension

- Launches the project's daemon, and launches it again if it dies (backoff from 1 s to 30 s).
  Its log is in the « Timonier » output channel.
- Ships the `timonier` program for its platform (phase 9, under way: one package each for
  Linux, macOS and Windows, x64 and arm64). « Timonier: Install the Command Line for the
  Agents », and **Set up** when `timonier` is missing, puts a copy on the agents' PATH
  (`~/.local/bin`, or the user's PATH on Windows) that follows the extension's updates.
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
  - **compare with the intent**: a toolbar toggle compares the map with the whiteboard. A banner
    says how many drawn components the code has and lacks, the forbidden edges it crosses (with
    the dependencies that cross them) and the required edges it lacks; a box that a drawn
    component stands for carries a ringed tick in its title bar (a dashed ring when it holds one
    deeper), its words in its tooltip, the details panel and for screen readers; a link that
    crosses a forbidden edge takes the violation tone and the word « forbidden ». Drawn boxes are
    matched to the code by name (case, spaces and dashes aside; a package whose name ends with
    it), then by place;
  - keyboard navigation (Tab, arrow keys, Enter, Home, + and - to zoom) and a command-palette
    command for each action.
- **The bridge**, the home view on the left, offers « Set up » (`timonier init` in a terminal)
  in a project not set up for your agents, with a badge on the Timonier icon, and answers three
  questions at a glance: where the plan
  stands (current milestone, open session), what happened since your last review (boxes,
  dependencies, agent sessions, files to read again), and where to look first: what needs a
  decision (a blocked task, an unreadable file, a cycle added), then the boxes to read again,
  untracked ones first. A click opens the map on the box.
- **Plan, todo and sessions** in one view of the secondary side bar, on the right (two glued
  columns when it is wide enough), updated live as the agent calls
  its tools: milestones with their computed progress and tasks by state; the open session's
  todo; the journal of sessions with their objective, tasks, components touched (live while the
  session is open), todo, AI cost and summary. A file of `.timonier/` edited by hand shows too, and one
  that no longer reads is named.
- **The whiteboard** (« Timonier: Open the Whiteboard »), in its own editor tab: freehand
  drawing with the mouse or a pen and an eraser, and typed shapes that count as constraints: a
  box is an intended component (a name and what it does), a zone a layer (the boxes drawn
  inside it are in it), an arrow a dependency that is allowed, forbidden or required, a note a
  constraint in words, a budget note a limit (latency p50 and p95, peak memory, allocations,
  calls). A properties panel sets what each shape says; everything works from the keyboard too
  (V, P, E, B, Z, A, N and M pick the tools; Ctrl+Z and Ctrl+Shift+Z undo and redo), and every
  action is a command of the palette. Every gesture is saved on its own to
  `.timonier/intent.json`, the file the agents read (the meaning apart from the geometry, in a
  stable order for readable git diffs); a change of that file outside the whiteboard, such as a
  `git checkout`, shows on it. `timonier init` puts an empty whiteboard.
- **Performances** (phase 8, under way). « Timonier: Show the Map's Performances (On or Off) » turns on or
  off a layer of the map: the time spent in each box and in what it called during the last run
  with traces (its own time and calls in its tooltip), a bar of each box's share of the run's
  own time (what it holds included, so that the shares of a level make 100%), and
  the links drawn thicker as more calls go through them. « Timonier: Open the Performances Panel »
  opens a panel: a run and its figures, the timeline of each test (its spans by depth, the
  memory and CPU of the process below), the call tree to unfold, with total time, own time,
  calls and the process's resources at each level, and the evolution of the runs by commit.
  A budget of the whiteboard that the last runs exceed marks its box on the map in red, always,
  and the journal of the session that ends keeps it. `timonier test --runs auto` runs the tests
  5 to 15 times and compares them with the main branch, as the specification decides: a
  measure slower beyond doubt (Mann-Whitney p < 0.01) and by at least max(5 %, 3 MAD) is a
  probable regression, confirmed by the next batch (`get_regressions`, the Evolution tab).
- Follows the IDE theme, light, dark and high contrast. WCAG AA contrast is checked automatically
  with axe-core in six built-in VSCode themes. The interface is in English and French.
