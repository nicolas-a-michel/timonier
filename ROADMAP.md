# Roadmap

**English** · [Français](ROADMAP.fr.md)

What Timonier will do next, in order. Timonier puts its effort on two things: the architecture
of the code, and how it performs. Dates are not promised: each step ends when its acceptance
criterion is checked on a real project ([PLAN.md](PLAN.md), in French, holds the criteria).
Last updated: 23 October 2026. Done since the last update: a map you can trust (Phase 11:
SCIP indexes, how sure each link is, renaming boxes, each language in one place).

## The end of the MVP

- **Full review of the MVP** against the specification ([docs/spec.md](docs/spec.md)).
- **Exit criterion**: a real project steered end to end without reading its code line by line.

## Next: performance in depth

- **Memory per component.** The instrumentation module Timonier adds counts the bytes each span
  allocates, and the map's Performances layer shows each component's share of them, as it shows
  its share of the time. First Rust, Java, Kotlin, Scala, C# and Python, whose runtimes count
  allocations per thread; the other languages' boxes say their memory is not measured.
- **Tests and performance in the session journal**: each agent session says which tests ran and
  how the performance moved, beside the budgets it already shows.
- **Level 2 profiling**: where the time goes inside a test, beyond its spans.
- **Changed but not tested**: in the diff since the last review, the symbols no test ran.

## Then: the architecture over time

- **What a commit changed**: choose a commit in the history and see on the map the components,
  files and links it added, modified or removed, as the review diff shows them since « Mark as
  reviewed ».
- **Growth of each component**: a small curve on each box, of its lines, symbols and
  dependencies over the commits.
- **Shortcuts taken by the agents**: disabled tests, weakened assertions, silenced warnings
  (`#[allow]`, `@ts-ignore`), stubs and `TODO`s left behind, empty `catch` blocks, shown as
  badges on the map's boxes with the commit and the session that brought them.
- **Duplicated code**: a function written again where one with the same body already exists,
  drawn as a link between the two places.
- **Dead code added**: new symbols that nothing calls or references, where a language server
  confirmed the links.
- **Public and security surfaces**: what a component exposes, and where input from outside
  reaches it.
- **Coverage, health and impact** per component, and **time travel** through the snapshots of
  the map.
- **Data-flow animation** on the map.

## Later

- **Skills manager**: see, install and update the skills and instructions each agent uses in a
  project (Claude Code skills, GitHub Copilot and Cursor instructions, Codex), from the IDE, and
  share them between projects.
- **Work outside a task's scope**: each plan task names the components it should change; the
  map marks the components a session touched outside them.
- **AI cost per component**: a map layer that shows where the agents' tokens went.
- **Several agents at once**: lanes, reservation of components, git worktrees.
- **Collaboration between models**: topologies, a protocol, channels, notifications and
  supervision.
- **GitLab, Jira and Linear** for the development process, beside GitHub.
- **Teams**: the map's snapshot and structural diff computed in CI for each pull request, with
  an automatic comment; rules and intent reviewed in pull requests; a shared session journal
  and review debt, for a team of 3 to 5 people on one repository.

## Not planned for now

- **Zed**: its extensions do not offer the panels Timonier needs yet; to be looked at again.
