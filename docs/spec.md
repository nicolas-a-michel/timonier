# Timonier — specification of the plugin for steering AI-driven development

*14 Oct 2026 · @Nicolas*

## Contents

1. [Vision and problem](#vision-and-problem)
2. [Technical architecture](#technical-architecture)
3. [Indexing and graph model](#indexing-and-graph-model)
4. [Whiteboard](#whiteboard)
5. [Architecture visualisation](#architecture-visualisation)
6. [Performance](#performance)
7. [Steering](#steering)
8. [Collaboration between models](#collaboration-between-models)
9. [Development process](#development-process)
10. [Technology inventory and component health](#technology-inventory-and-component-health)
11. [Public surface and security surface](#public-surface-and-security-surface)
12. [Initialisation and AI configuration](#initialisation-and-ai-configuration)
13. [Experience and design](#experience-and-design)
14. [MCP interface](#mcp-interface)
15. [MVP / v2 / v3 scope](#mvp--v2--v3-scope)
16. [Technical decisions](#technical-decisions)

## Vision and problem

The plugin lets the developer steer an AI agent without reading all the code it produces. When
the AI writes the code, the bottleneck is no longer production but understanding: the code
changes faster than the developer's mental model of it, and the developer builds up a
comprehension debt.

The developer becomes architect and reviewer. They need four answers at all times:

- **What I want**: the intended architecture, drawn freely.
- **What exists**: the real architecture, the technologies, the health of the code.
- **How it behaves**: execution time and resources, measured during the tests.
- **What is happening**: the plan, the live todo, what the agent did and where to look.

**Target**: any language, solo use first, then teams.

**Name**: Timonier. The *timonier* (helmsman) holds the wheel and keeps the course while the crew,
here the agents, moves the ship. CLI command `timonier`, short alias `tim`.

Availability checked on 14 October 2026:

- crate `timonier` on crates.io and package `timonier` on npm: free, to be reserved;
- VSCode Marketplace: no extension found, to be confirmed;
- GitHub organisation `timonier`: taken (Docker images inactive for more than five years), plan
  for `timonier-dev` or similar;
- minor namesake: an Emacs package for managing Kubernetes;
- domains (.dev, .io, .fr): to be checked with a registrar.

## Technical architecture

A local Rust daemon, one per project, holds the whole model; the front-ends stay thin and
interchangeable.

The unified graph is the single source of truth: the IDE and the agent read the same model, the
first through JSON-RPC, the second through MCP.

- **VSCode**: main front-end of the MVP, thanks to the webview API (map, tldraw whiteboard, plan,
  timelines).
- **Zed**: out of scope for now, to be reconsidered later.
- **AI agent**: any MCP-compatible agent, configured by `timonier init`.

## Indexing and graph model

No analyser is written per language: three generic sources are merged into a unified graph, with a
provenance and a confidence per edge.

| Source | Contribution | Limit |
| --- | --- | --- |
| Tree-sitter | Symbols, imports, definitions, ~200 languages | Approximate call resolution |
| LSP (already launched by the IDE) | Precise definitions, references, call hierarchy | Queried on the fly, costly in bulk |
| SCIP/LSIF when available | Complete and precise graph | Batch, not for every language |

**Schema.**

- **Nodes**: Repo, Component, Package, File, Symbol (module, type, trait, function, method, field,
  constant), with a name, a language, a span and a parent.
- **Edges**: Imports, Calls, References, Implements, Inherits, DependsOn, with a provenance
  (TreeSitter, Lsp, Scip, Manifest, Inferred, Intended), a confidence and a weight.
- The hierarchy (parent) is a strict tree; the relations form a free graph. Edges between
  components are aggregated from symbol edges, never stored by hand.

**Stable identity.** The SCIP symbol format when it exists, otherwise `lang:qualified_name`. Moves
are detected at diff time by similarity (signature + hash of the normalised body), so that a
refactoring does not turn into « everything deleted, everything recreated ».

**Fusion.** An edge seen by several sources keeps the most reliable provenance; a tree-sitter edge
contradicted by the LSP is removed.

**Components.** Aggregation by structure (crates, packages, folders), then graph clustering
(Louvain/Leiden). The LLM names and summarises the clusters; it does not compute them. Each
component gets an architectural role (server or entry point, data model, persistence, interface,
background task, group of external libraries, utility), deduced from the frameworks and APIs used
(a `TcpListener`, an HTTP router, an ORM, serialised types) and then confirmed by the LLM.
External dependencies are grouped by use (network, serialisation, database, async). The map thus
opens on a very high-level view, in the manner of the C4 model: a few boxes named by their role and
linked together. Clicking or zooming on a box opens its contents, themselves made of boxes, and so
on down to files and symbols.

**Storage.** SQLite, updated continuously. Each node and edge carries a `valid_from`/`valid_to`
interval of snapshots; a table `snapshot(seq, commit_sha, timestamp, kind)` with
`kind ∈ {commit, review_mark, working_tree}`. The diff between two snapshots is a query.
`.timonier/actual.json` is regenerated at each commit by a hook.

## Whiteboard

The whiteboard is one-way: the developer draws, the AI reads. Nothing comes back onto it: no
ghosts, no proposals, no computed statuses. The AI's work shows in the architecture visualisation.

**Mixed mode.** Free drawing everywhere, interpreted by a vision model and by the text. A shape can
be typed when it must count as a constraint:

- box → intended component (name, role);
- arrow → edge with a policy `allowed`, `forbidden` or `required`;
- zone → layer or boundary (domain, infra, UI); an edge between layers applies to all their
  contents;
- note → constraint in text, attached to an element.

**Performance budgets.** A note typed *budget* sets a measurable limit on a component or a path:
latency (p50, p95), maximum memory, allocations, number of calls. Example: « Parser: p95 < 5 ms,
< 50 MB ». The agent reads it as a constraint; the daemon checks it at each run of the tests.

**File `.timonier/intent.json`.** A single author, the developer. Semantics and geometry kept apart
(`shape` only serves the rendering), stable local IDs (`C-…`, `E-…`, `L-…`, `N-…`), versioned in
the repository.

```json
{
  "version": 1,
  "layers": [{ "id": "L-domain", "name": "Domain" }],
  "components": [{ "id": "C-parser", "name": "Parser", "layer": "L-domain",
                    "role": "Turns the tokens into an AST" }],
  "edges": [{ "id": "E-2", "src": "L-domain", "dst": "L-infra",
               "kind": "depends_on", "policy": "forbidden" }],
  "notes": [{ "id": "N-1", "text": "No allocation in the hot loop",
               "attached_to": "C-parser" }],
  "sketch": { }
}
```


**Implementation.** tldraw in the webview (custom shapes, clean JSON export), rather than
Excalidraw, which suits typed shapes less well.

## Architecture visualisation

The visualisation is generated from the real code only: that is where the AI's work shows.

- **Component map** with zoom component → files → symbols, and a click through to the code. Edges
  are drawn thicker according to the measured traffic.
- **Diff since my last review**: components, dependencies and cycles added, components whose
  fan-in/fan-out jumped, a « modified » badge on the boxes an internal refactoring touched. A
  « mark as reviewed » button creates a `review_mark` snapshot.
- **Compare with the intent** (display option): drawn components highlighted, missing components
  listed, forbidden edges crossed flagged. The anchoring drawn box ↔ real component is computed
  here, by name and place, without writing to the intent file.
- **Time travel**: a slider over the snapshots to replay the evolution.
- **Layers** to switch on over the map: performance, test coverage, review debt, health, AI cost.

**Displayed precision**: an edge from tree-sitter alone is drawn differently from an edge confirmed
by LSP or SCIP.

**Data-flow animation.** Particles move along the map's edges to show how data travels through the
software.

- **Real source**: the OpenTelemetry traces of the tests. The animated flow is what really
  happened, not a hypothesis.
- **Encoding**: density and speed of the particles by call throughput, size by the volume carried,
  hue by the type of data (from the LSP signatures). The attributes `timonier.payload.bytes` and
  `timonier.payload.type` join the instrumentation conventions for internal calls.
- **Two modes**: aggregated view (the overall traffic of a test run) and replay of a single trace
  (the path of one request or test, step by step).
- **Controls**: play, pause, speed, a time slider synchronised with the performance timeline; a
  click on a particle opens the matching span and code.
- **Landmarks**: bottlenecks visible as piles of particles, errors flagged by a brief flash on the
  node concerned.

True to the design principles: slow and soft motion, muted colours, animation off by default for
users who turned on reduced motion, with an equivalent static view (thicker edges, throughput
figures).

## Performance

The performance graphs fill in by themselves during the tests, with explicit OpenTelemetry
instrumentation (level 3) from the MVP on. No proprietary library: if the plugin disappears, the
instrumentation stays valid.

| Level | Mechanism | AI cooperation |
| --- | --- | --- |
| 1 | `timonier test` wraps the runner: duration, CPU, maximum memory, I/O through cgroups v2 / procfs; duration per test through JUnit XML | None |
| 2 | Sampling profiling (perf/eBPF, py-spy, async-profiler), symbols attached to the graph | None |
| 3 | Local OTLP receiver (gRPC 4317 / HTTP 4318), spans and metrics through the language's OTel SDK | Required |

Level 1 always runs in the background as the reference; level 2 is the fallback for a language
without a mature OTel SDK.

**Wiring.** `timonier test` injects `OTEL_EXPORTER_OTLP_ENDPOINT` and
`OTEL_RESOURCE_ATTRIBUTES=timonier.run_id=…,timonier.snapshot=…`. Each test becomes a root span.

**Conventions imposed on the AI.**

- One span per component boundary (public function of a module).
- Required attributes `code.function.name`, `code.file.path`, `code.line.number`, to attach the
  span to the graph.
- No spans in hot loops: histograms and counters instead.
- Instrumentation that can be switched off at compile time or by configuration (in Rust: `tracing`
  + `tracing-opentelemetry` behind a feature).

**Check after each run.** Public functions never seen in the traces, spans without `code.*`
attributes, overhead estimated against level 1. The gaps are reported and readable by the agent
through MCP.

**Budgets.** Each budget note of the whiteboard is compared with the run's measurements (median
over several runs). Exceeding one shows in red on the map and in the session's journal, and the
agent gets it through `get_budget_violations`.

**Views.** Timeline per test (nested spans, the process's resources below), annotated map
(cumulative time, number of calls), evolution by commit (median over several runs, a regression
beyond a threshold).

## Steering

The plan and the todo are the common thread: always visible, they link each task to the code, the
tests and the performance it produced.

- **Plan**: milestones → tasks, each task attached to components of the whiteboard. A permanent
  side panel, progress computed.
- **Todo**: the tasks of the session in progress, which fill in when the agent splits the work and
  empty as it runs, live.
- **Session journal**: for each agent session, the objective, the tasks, the components touched,
  the structural diff, the tests and the effect on performance. It answers « why does this module
  exist? » weeks later.
- **Review debt**: heat per component according to the amount of code the AI wrote and nobody
  read again. Changes made outside a session are marked « untracked ».
- **Test coverage** per component, as a layer over the map.
- **Impact analysis**: select a component → everything that depends on it, to frame a prompt
  before launching the agent.
- **AI cost**: tokens and money per session and per component, to spot where the agent goes round
  in circles.

The agent never edits `plan.json`, `todo.json` or the journal by hand: it goes through the MCP
tools, which validate every write.

**Multi-agent.** Several agents can work in parallel on the same project.

- One lane per agent in the plan and the todo, each session identified by its agent.
- Component reservation: an agent reserves those it will change (`claim_components`); an overlap is
  refused or flagged before it codes.
- Isolation by a git worktree per agent, controlled merge at the end of the session.
- Detection of structural conflicts: two agents changing the same public surface or creating
  contradictory edges.
- A real-time overview on the map: which agent works on which component.

## Collaboration between models

Several models, possibly from different vendors, work in parallel following a protocol the plugin
sets in advance: they share out the tasks, help and check each other. The developer sees at all
times who does what and what they tell each other.

**Predefined topologies.**

| Topology | Roles | Use |
| --- | --- | --- |
| Pair | One implementer, one reviewer who validates each task | Continuous quality control |
| Orchestrator | A planner splits and assigns, workers deliver | Large parallelisable milestones |
| Swarm | Equal agents who take free tasks, with component reservation | Maximum throughput on independent tasks |
| Second opinion | Two models implement the same task, a third arbitrates | Critical decisions, choice of algorithm |

The roles (planner, implementer, reviewer, tester, arbiter) are given per model in
`.timonier/team.json`, in a few clicks from a preset.

**Protocol.** Exchanges are typed messages, never free text alone: `task_assign`, `question`,
`answer`, `review_request`, `review_result`, `handoff`, `blocker`, `status`, `escalate`. Each task
follows a state machine (assigned, in progress, in review, accepted, rejected, escalated) that the
daemon enforces: an implementer cannot accept their own task, a rejected review sends the task back
with its reasons.

**Channels.** One channel per task, one per component, a general channel and direct messages. The
daemon stores them as an append-only journal, linked to the sessions, the commits and the pull
requests.

**Notifications without active watching.** A model cannot be interrupted in the middle of a
generation; the plugin does guarantee that it learns of a message's arrival as early as possible,
without spending tokens on watching:

1. **Mailbox header**: every response of a Timonier MCP tool carries the number of unread
   messages and their priority. Any MCP-compatible agent sees it at its next tool call.
2. **Hooks**, when the agent offers them: messages injected after each tool call, and a refusal to
   end a session while a blocking message is waiting.
3. **Wake on message**: an idle agent is relaunched by the daemon in non-interactive mode, its
   session resumed and the message as input.

**Supervision.**

- A view of the channels, like a team messenger, with filters by model, task and message type.
- Swimlanes per model over time: task in progress, waiting, review, blocked.
- A distribution board: which model holds which tasks and which components, with each one's load
  and cost.
- Human intervention: post in any channel, pause a model, reassign a task, settle an escalation.

**Safeguards.** Loop detection (review ping-pong), a message and token budget per task, automatic
escalation to the human after a set number of disagreements.

## Development process

The developer chooses in a few clicks how the agent works, as one sets a team's process: branches,
pull requests, tickets, review, continuous integration.

| Preset | How it works | For whom |
| --- | --- | --- |
| Solo, fast | Direct commits on the main branch, one session = a series of commits | Prototype, exploration |
| Solo, disciplined | A branch and a PR per task, merged automatically if tests, budgets and rules pass | A developer alone on a lasting project |
| Team | Ticket → branch → PR → required human review → merge, blocking CI | Shared project, code in production |

Each preset is a starting point: every setting stays changeable on its own in a settings panel.

**Available settings.**

- **Tickets**: a ticket created automatically for each task of the plan, its state kept in sync
  (open, in progress, in review, closed); GitHub Issues, GitLab, Jira, Linear.
- **Branches**: naming convention (`tim/<ticket>-<slug>`), one worktree per agent with several
  agents.
- **Commits**: convention (Conventional Commits or free), reference to the ticket, granularity.
- **Pull requests**: opened automatically at the end of the session, with a description generated
  from the task, the structural diff, the tests, the effect on performance and any violations;
  draft or ready for review.
- **Review and merge**: human approval required or not, automatic merge under conditions, strategy
  (squash, merge, rebase).
- **CI**: generation of the workflow (GitHub Actions, GitLab CI) that replays the tests, the budgets
  and the architecture rules.

**File `.timonier/workflow.json`.** Versioned in the repository, it describes the chosen process.
The managed block of the agent's instructions derives from it, and the MCP tools apply it:
`task_start` creates the ticket and the branch, `session_end` opens the PR. The agent does not need
to know the process; it follows it by construction.

Access to the forges goes through their CLIs (`gh`, `glab`) or their APIs, with the tokens stored
in the IDE's keychain.

## Technology inventory and component health

**Inventory.** Built from the manifests (`Cargo.toml`, `package.json`, `pyproject.toml`,
`go.mod`…) as a CycloneDX SBOM: languages, frameworks, dependencies with versions, licences, known
CVEs. It flags first the functional duplicates (two libraries for the same thing) and the
dependencies the agent added since the last review.

**Health per component.**

- churn (frequency of change);
- complexity;
- duplication, a frequent flaw of generated code;
- dead code and orphan symbols.

These indicators feed a layer of the map and are exposed to the agent.

## Public surface and security surface

Two views derived from the graph make visible changes that the AI often brings in without saying
so.

**Public surface.** For each component, the exported symbols (functions, types, endpoints,
schemas, CLI commands) and their signatures, through LSP/SCIP and the OpenAPI, protobuf or GraphQL
files when they exist. The diff since the last review classifies each change: addition, compatible
change, break. Breaks are flagged on the map and in the session's journal.

**Security surface.** A map of the points where the code touches the outside:

- entry points (HTTP endpoints, CLI, message handlers);
- untrusted inputs and their propagation down to sensitive sinks (SQL, shell, file system,
  deserialisation);
- outgoing network access, files, processes launched;
- use of secrets and environment variables;
- dependencies with known CVEs, from the inventory.

Any new surface entry the agent adds is flagged, even without a violation. The flow analysis starts
with heuristics per language (tree-sitter queries), then relies on Semgrep or CodeQL when they are
available.

## Initialisation and AI configuration

`timonier init` sets up the tracking files as soon as the project is created (or when an existing
project is adopted) and configures the agent.

```text
.timonier/
  intent.json           # whiteboard (author: the developer)
  plan.json             # milestones and tasks
  todo.json             # active tasks
  sessions/             # journal, one file per agent session
  conventions/          # instrumentation and rules, per language
  actual.json           # real state, regenerated at each commit
AGENTS.md               # block managed by the plugin
.mcp.json, .vscode/mcp.json   # MCP server declared
```

**Automatic configuration.** Two entry points: the « New steered project » wizard in VSCode, and
the detection, when a repository that is not configured is opened, which offers `timonier init`.
The developer only answers the questions the detection cannot settle.

1. **Detection**: languages, package manager, test runner, agents installed (Claude Code, Copilot,
   Cursor, Codex…), git state.
2. **Skeleton** (new project): the plugin calls the official generator of the chosen stack
   (`cargo new`, `npm create`, `uv init`…) rather than home-made templates.
3. **Instrumentation**: the language's OpenTelemetry SDK added, an initialisation module behind a
   feature or a configuration variable, the test runner set to produce JUnit XML.
4. **Agents**: a managed block in the instruction file of each agent detected, the MCP server
   declared in their format, hooks when they exist.
5. **Git**: a commit hook that regenerates `actual.json`, the SQLite database ignored, the
   `.timonier/` files versioned.
6. **Tracking**: an empty whiteboard, a plan with an « Initialisation » milestone, a first session
   recorded.
7. **End-to-end proof**: `timonier doctor`, then an instrumented smoke test that must produce a
   first trace attached to the graph. The chain is validated before the first task.

**Language packs.** Everything that depends on the language (instrumentation conventions, adding
the SDK, configuring the runner, security-surface queries) lives in a versioned, extensible pack.
Adding a language means writing a pack, not changing the daemon.

**Safety.** `timonier init --dry-run` shows the diff of every file touched; the operation is
idempotent, and `timonier uninit` removes everything the plugin added.

The managed block of `AGENTS.md` (and its equivalents `CLAUDE.md`, Cursor rules, Copilot
instructions, depending on the agents detected) is delimited by markers: the plugin rewrites it,
the rest of the file belongs to the developer.

**Enforcing the protocol.** An agent cannot be constrained 100 %; the gap is made visible and
costly:

1. **Instructions**: open a session, split into a todo, instrument according to the conventions.
2. **Hooks**, when the agent offers them: refusal to close a session if the todo is not up to date
   or the journal is empty.
3. **Detection by the daemon**: any change of code outside a session is marked « untracked » and
   counts in the review debt.
4. **`timonier doctor`**: checks and repairs the configuration (MCP declared, managed block up to
   date, conventions present).

## Experience and design

Timonier must be restful to look at for hours and understood at a glance. These principles apply
from the MVP on, to every view.

**Calm by default.**

- A desaturated palette, neutral backgrounds; vivid colour is reserved for meaning (red =
  violation, orange = attention, green = conforming) and only appears when there is something to
  say.
- Integration with the IDE's theme through its CSS variables, light as well as dark, so that
  Timonier feels native.
- Short, soft animations, only to explain a change (a component appearing, an edge getting
  thicker), never decorative.
- Discreet notifications: badges and summaries rather than a shower of pop-ups; only blocking
  alerts interrupt.

**Clear before complete.**

- A home view, the bridge, which answers three questions in a few seconds: where the plan stands,
  what happened since my last review, where I should look.
- Progressive disclosure: overview first, detail on hover, depth on click.
- Semantic zoom on the map: zooming changes the level of detail (components → files → symbols)
  instead of drawing everything smaller.
- Stable layout: the map keeps the elements' positions from one update to the next (incremental,
  constrained layout), so that the eye keeps its landmarks.
- A single visual language shared by all views: the same shapes, colours and icons for a
  component, an agent, a violation.

**Accessible and fluid.**

- WCAG AA contrast at least, a colour-blind-safe palette, and never colour alone to carry
  information (an icon or a pattern as well).
- Full keyboard navigation, a command palette entry for each action.
- Fluid on large graphs: level-of-detail rendering, automatic aggregation beyond a threshold of
  nodes.
- Adjustable density (comfortable or compact), teaching empty states that say what to do next.

**Method.** A small design system defined before the first view (colour tokens, typography,
spacing, components), mock-ups tested with a few developers, and a simple test for each screen:
does a user who discovers it understand in five seconds what it shows?

## MCP interface

The same model of the project serves the human and the agent: the agent consults the intent and
the rules before coding, and reads its own results afterwards.

| Domain | Tools |
| --- | --- |
| Intent | `get_intended_architecture`, `get_drift`, `get_budgets` |
| Graph | `query_graph`, `get_component`, `get_impact` |
| Surfaces | `get_public_api_diff`, `get_security_surface` |
| Performance | `get_instrumentation_conventions(lang)`, `get_perf(component)`, `get_instrumentation_gaps`, `get_budget_violations` |
| Tracking | `session_start`, `session_end`, `plan_add_task`, `plan_update_task`, `todo_add`, `todo_complete` |
| Process | `get_workflow`, `task_start`, `open_pull_request`, `link_ticket` |
| Multi-agent | `claim_components`, `release_components`, `list_active_agents` |
| Communication | `post_message`, `read_inbox`, `ack_message`, `request_review`, `submit_review`, `handoff_task`, `escalate` |
| Quality | `get_tech_inventory`, `get_component_health`, `get_coverage` |

The names are provisional; the write tools only accept structured, validated operations.

## MVP / v2 / v3 scope

The MVP aims at the complete loop for a developer alone in VSCode: I draw, the agent produces
following the protocol, I see the result, its performance and where to look.

| Phase | Content | Exit criterion |
| --- | --- | --- |
| MVP (solo, VSCode) | Rust daemon, tree-sitter + LSP indexing, components, map and review diff, mixed whiteboard and comparison with the intent, performance levels 1 and 3, performance budgets, `timonier init` / `timonier doctor`, plan, todo and journal through MCP, development process (presets, GitHub), review debt, technology inventory | A real project steered end to end without reading the code line by line |
| v2 (depth) | SCIP, level 2 profiling, public surface, security surface, multi-agent (lanes, reservation, worktrees), collaboration between models (topologies, protocol, channels, notifications, supervision), coverage per component, health, impact analysis, time travel, data-flow animation, AI cost, GitLab, Jira and Linear | All steering views available on at least two languages, two agents in parallel without conflict |
| v3 (team) | Snapshot and structural diff computed in CI per PR, automatic PR comment, rules and intent reviewed in PR, shared journal and review debt | A team of 3 to 5 people uses it on one repository |

The MVP stays ambitious; if a tighter scope is needed, the technology inventory and the comparison
with the intent are the first candidates to be postponed.

## Technical decisions

- **Default granularity of Calls edges on a large repository**: function → function, or cut down
  to the file for languages without a reliable LSP?
  **Decided**: function → function edges stored only when they come from the LSP or SCIP,
  otherwise weighted file → file edges; display always aggregated at the zoom level, starting from
  the high-level view by roles; lazy LSP refinement when a file is opened; compaction beyond about
  50,000 symbol edges.
- **Incremental indexing strategy**: invalidation, order of merging the sources, cost of LSP
  queries in bulk.
  **Decided**: three speeds. Immediate on save (debounce ~300 ms, file hash, incremental
  tree-sitter reparse, graph update by difference); background (dependent files marked if the
  public signature changes, LSP refinement in a low-priority queue starting with what is
  displayed); at commit (SCIP when available, full re-clustering seeded by the previous partition,
  frozen snapshot). The most reliable source wins, each file remembers « verified by LSP at hash
  X ». A single SQLite writer in WAL mode, parallel initial indexing, persistent cache.
- **Graph library for the webview** (Cytoscape + ELK, or another) and how it scales to several
  thousand nodes.
  **Decided**: React in the webview (consistent with tldraw), React Flow (xyflow) for the map,
  elkjs for the hierarchical layout with the previous positions as constraints, a canvas/WebGL
  layer (PixiJS) for the flow animation, uPlot for the timelines and performance curves. Semantic
  zoom keeps the display to a few hundred nodes.
- **Protocol between the daemon and the webview** (JSON-RPC over a local socket, or reusing the MCP
  server).
  **Decided**: JSON-RPC 2.0 over a local socket (Unix or named pipe) between the daemon and the
  extension, relayed by `postMessage` to the webview; subscriptions per view with versioned deltas
  and a full snapshot on desynchronisation; MCP apart, a thin proxy to the same daemon; JSON first,
  MessagePack in reserve for the traces; the same protocol over a local WebSocket with a token for
  Zed or a standalone app.
- **Performance regression threshold**, and the number of runs needed for it to be reliable.
  **Decided**: detection first on the number of CPU instructions (hardware counters) when
  available, wall time displayed; 5 executions, up to 15 if the variance is high, median and MAD;
  reference = median of the last 5 snapshots of the main branch; regression flagged if
  Mann-Whitney p < 0.01 and the gap ≥ max(5 %, 3 × MAD); « probable », then « confirmed » after a
  re-run; reference reset if the machine's fingerprint changes.
- **Real capabilities of Zed extensions** (UI panels) at the time of development.
  **Decided**: Zed out of scope for now, to be reconsidered later.
- **Measuring the AI's cost**: which agents expose their token consumption, and how to get it.
  **Decided**: 1) native OpenTelemetry telemetry when the agent exports it (Claude Code: tokens by
  type and estimated cost), pointed at the daemon's OTLP receiver by `timonier init`; 2) a local
  counting proxy for the agents using an API key; 3) otherwise « not measured », never an invented
  estimate. Exact attribution per session and per task, estimated per component in proportion to
  the changes. Tokens are the reference measure, the cost in dollars an indication.
