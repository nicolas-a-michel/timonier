---
name: mcp-tools
description: Designing and implementing a Timonier MCP tool in timonier-mcp (a thin proxy to the daemon) — naming, validated input schema, the mailbox header carried by every response, structured-only write operations, error shape, tests and documentation. Use when adding, renaming or changing any MCP tool, or whenever the agent-facing contract of Timonier changes.
---

# MCP tools

## Principles (spec, « Interface MCP », « Pilotage », « Collaboration entre modèles »)

- The agent and the human use the same project model. The agent reads the intent and the
  rules before coding, and reads its own results afterwards.
- The MCP server is a **thin proxy** to the same daemon the IDE talks to over JSON-RPC. It holds
  no business logic and no storage access.
- **Write tools accept only structured, validated operations.**
- The agent never edits `plan.json`, `todo.json` or the session log by hand. It goes through
  the tools, which validate every write.
- **Every response carries the mailbox header**: the number of unread messages and their
  priority. In the solo MVP there are no messages yet, but the header is always present.
- Tool names are provisional in the spec. A renamed or new tool gets an ADR.

## Catalogue (spec)

| Domain         | Tools                                                                                                 |
| -------------- | ----------------------------------------------------------------------------------------------------- |
| Intention      | `get_intended_architecture`, `get_drift`, `get_budgets`                                               |
| Graphe         | `query_graph`, `get_component`, `get_impact`                                                          |
| Surfaces       | `get_public_api_diff`, `get_security_surface`                                                         |
| Performances   | `get_instrumentation_conventions(lang)`, `get_perf(component)`, `get_instrumentation_gaps`, `get_budget_violations` |
| Suivi          | `session_start`, `session_end`, `plan_add_task`, `plan_update_task`, `todo_add`, `todo_complete`      |
| Processus      | `get_workflow`, `task_start`, `open_pull_request`, `link_ticket`                                      |
| Multi-agents   | `claim_components`, `release_components`, `list_active_agents`                                        |
| Communication  | `post_message`, `read_inbox`, `ack_message`, `request_review`, `submit_review`, `handoff_task`, `escalate` |
| Qualité        | `get_tech_inventory`, `get_component_health`, `get_coverage`                                          |

Which tools belong to which phase is in `PLAN.md`. Multi-agents, communication and surfaces are v2.

## Checklist for a tool

1. **Name**: snake_case, verb_object, taken from the table above when it is listed there.
2. **Description**, written for an agent: what the tool does, when to call it, and what it
   returns, in a sentence or two.
3. **Input schema**:
   - strict JSON Schema: `additionalProperties: false`, explicit `required`, enums for closed
     sets, bounded strings and arrays;
   - generated from the Rust type, so there is a single source (mechanism fixed by ADR);
   - validated **in the daemon**, which is the authority, and not only in the proxy.
4. **Output**:
   - structured JSON plus a one-line text summary;
   - stable field names;
   - lists are paginated (cursor and limit) and size-capped.
5. **Mailbox header** on every response, errors included: unread count and highest priority.
   The exact shape is fixed by ADR in Phase 6.
6. **Writes** are operations, never documents: `plan_update_task { task_id, status, note }`,
   never "replace plan.json".
   - Validate the state transition.
   - Make the operation idempotent where possible.
   - Attribute every write to a session and timestamp it.
   - The daemon is the only writer of `.timonier/plan.json`, `todo.json` and `sessions/`.
7. **Errors**: an MCP tool error result with a stable code and an actionable message, such as
   "no open session: call session_start first". Never a stack trace.
8. **No logic in `timonier-mcp`**: it maps MCP to JSON-RPC and back, nothing more.
9. **Tests**:
   - schema validation, with valid and invalid inputs;
   - an integration test running an MCP client → `timonier-mcp` → daemon on a temporary project;
   - a snapshot test of the tool list (names and schemas) that catches accidental contract
     changes.
10. **Documentation**:
    - the user docs (French) describe the tool;
    - the template of the managed block in `AGENTS.md` / `CLAUDE.md` mentions it if agents
      must use it.

## How it is built (ADR 0027, 0028, 0029)

- SDK: `rmcp` 3.5.1 (locked in `Cargo.lock`), stdio only. `timonier mcp` (CLI) finds or starts
  the daemon quietly (stdout carries MCP), then `timonier-mcp` relays MCP `tools/list` and
  `tools/call` to the daemon's JSON-RPC methods of the same names (protocol version 4).
- A tool is:
  1. its input type in `timonier_core::tracking` (or the module of its domain): serde with
     `deny_unknown_fields`, `schemars` bounds (`length`, `pattern`);
  2. its entry in `timonier_core::tools::catalogue()` (name, description, read-only hint);
  3. its handling in the daemon (`timonier_daemon::tracking::Tracker::run`), which validates
     again and writes;
  4. the snapshot `crates/timonier-core/tests/tools.json`, refreshed with
     `TIMONIER_UPDATE_TOOLS=1 cargo test -p timonier-core` after checking the change.
- Results are MCP `CallToolResult`s built by `tools::success` / `tools::failure`: the mailbox
  header (`{unread, priority}`) in the structured content and as the first text line.
- Error codes in use: `invalid_argument`, `unknown_tool`, `unknown_task`, `unknown_milestone`,
  `unknown_item`, `unknown_session`, `forbidden_transition`, `no_open_session`,
  `session_already_open`, `invalid_file`, `io_error`, `internal`.
- Tests: core unit tests for the operations, daemon unit tests for the files, and
  `crates/timonier-cli/tests/mcp.rs` for a real MCP client → `tim mcp` → a real daemon.
