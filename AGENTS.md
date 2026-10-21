<!-- timonier:begin -->
## Timonier

This project is steered with Timonier. Its MCP server, `timonier`, holds the project's plan, the todo of the session in progress and the journal of the sessions; the developer follows them live in the IDE.

- Before changing code, open a session: `session_start` with the objective of the work, and the plan task it serves when there is one.
- Split the work into todo items with `todo_add`, and mark each one done with `todo_complete` as you go.
- Keep the plan current: `plan_add_task` for new work, `plan_update_task` when a task starts, is blocked, done or dropped.
- When the work is done, call `session_end` with a summary of what was done and what is left.
- Before changing the structure of the code, read the architecture the developer wants with `get_intended_architecture` (their whiteboard, with an image of the sketch); after, check `get_drift`: forbidden edges crossed, required edges and drawn components missing.
- Never edit `.timonier/` by hand: the tools write `plan.json`, `todo.json` and `sessions/`, and `intent.json`, the whiteboard, belongs to the developer.
- Instrument the code as `.timonier/conventions/<language>/instrumentation.md` says (`get_instrumentation_conventions`); after a run of `timonier test`, check `get_instrumentation_gaps` and `get_budget_violations` (the whiteboard's performance budgets); for performance work, run `timonier test --runs auto` and read `get_regressions`.
- Every tool result starts with a mailbox header: read the messages it announces.
<!-- timonier:end -->
