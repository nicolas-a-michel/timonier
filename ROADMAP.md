# Roadmap

**English** · [Français](ROADMAP.fr.md)

What Timonier will do next, in order. Dates are not promised: each step ends when its
acceptance criterion is checked on a real project ([PLAN.md](PLAN.md), in French, holds the
criteria). Last updated: 22 October 2026.

## The end of the MVP

- **« New steered project » assistant**: start a project from VSCode with its agents, its
  conventions and its whiteboard already set up.
- **Full review of the MVP** against the specification ([docs/spec.md](docs/spec.md)).
- **Exit criterion**: a real project steered end to end without reading its code line by line.

## Right after the pre-release

- **Each language in one place.** Today a language is its pack (grammar, queries, rules) plus
  code spread over the daemon, the command line and the core (manifests, inventory, the edits of
  `timonier init`, what is public). The rules that are data move into the pack; the rest goes
  into one module per ecosystem behind a single interface, so that adding a language never
  touches the daemon. No change for users.

## Next (v2: depth)

- **Skills manager**: see, install and update the skills and instructions each agent uses in a
  project (Claude Code skills, GitHub Copilot and Cursor instructions, Codex), from the IDE, and
  share them between projects.
- **SCIP indexing**, for exact references in the languages that have an indexer.
- **Level 2 profiling**: where the time goes inside a test, beyond its spans.
- **Public and security surfaces**: what a component exposes, and where input from outside
  reaches it.
- **Several agents at once**: lanes, reservation of components, git worktrees.
- **Collaboration between models**: topologies, a protocol, channels, notifications and
  supervision.
- **Coverage, health and impact** per component, and **time travel** through the snapshots of
  the map.
- **Renaming a box**: the developer gives a box of the map its own name (F2 or a double click),
  kept in `.timonier/` across reindexing and commits; the agents can propose names through an
  MCP tool, and the developer's name wins.
- **What a commit changed**: choose a commit in the history and see on the map the components,
  files and links it added, modified or removed, as the review diff shows them since « Mark as
  reviewed ».
- **Shortcuts taken by the agents**: disabled tests, weakened assertions, silenced warnings
  (`#[allow]`, `@ts-ignore`), stubs and `TODO`s left behind, empty `catch` blocks, shown as
  badges on the map's boxes with the commit and the session that brought them.
- **Work outside a task's scope**: each plan task names the components it should change; the
  map marks the components a session touched outside them.
- **Duplicated code**: a function written again where one with the same body already exists,
  drawn as a link between the two places.
- **Changed but not tested**: in the diff since the last review, the symbols no test ran.
- **Growth of each component**: a small curve on each box, of its lines, symbols and
  dependencies over the commits.
- **Dead code added**: new symbols that nothing calls or references, where a language server
  confirmed the links.
- **AI cost per component**: a map layer that shows where the agents' tokens went.
- **Data-flow animation** on the map.
- **GitLab, Jira and Linear** for the development process, beside GitHub.

## Later (v3: teams)

- The map's snapshot and structural diff computed in CI for each pull request, with an automatic
  comment.
- Rules and intent reviewed in pull requests.
- A shared session journal and review debt, for a team of 3 to 5 people on one repository.

## Not planned for now

- **Zed**: its extensions do not offer the panels Timonier needs yet; to be looked at again.
