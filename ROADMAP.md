# Roadmap

**English** · [Français](ROADMAP.fr.md)

What Timonier will do next, in order. Dates are not promised: each step ends when its
acceptance criterion is checked on a real project ([PLAN.md](PLAN.md), in French, holds the
criteria). Last updated: 22 October 2026.

## Before the pre-release

The MVP's phases 0 to 8 are done, and phase 10 has given Timonier its thirteen languages. What
remains before a first public version:

- **Windows on a real machine.** The tests pass on Windows in CI; a try on a real machine
  confirms the extension and the daemon there.
- **Pre-release on the VSCode Marketplace**, for Linux, macOS and Windows (x64 and arm64).

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
- **Data-flow animation** on the map.
- **GitLab, Jira and Linear** for the development process, beside GitHub.

## Later (v3: teams)

- The map's snapshot and structural diff computed in CI for each pull request, with an automatic
  comment.
- Rules and intent reviewed in pull requests.
- A shared session journal and review debt, for a team of 3 to 5 people on one repository.

## Not planned for now

- **Zed**: its extensions do not offer the panels Timonier needs yet; to be looked at again.
