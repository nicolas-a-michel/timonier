# Changelog

## 0.1.1 — pre-release

- A Python project without `pyproject.toml` or `requirements.txt`, only its `.py` files, is now
  recognised by « Set up », and its tests run with pytest.
- « Run the tests » no longer asks to set up a project that is set up but has no test runner.
- The six platforms are published one after another, each once the Marketplace serves the
  previous one: 0.1.0 first reached only macOS on Apple silicon.

## 0.1.0 — pre-release

The first public version, to try Timonier on real projects. Linux (x64, arm64), macOS (Intel,
Apple silicon) and Windows (x64, arm64); projects in Rust, TypeScript, JavaScript, Python, Go,
Java, Kotlin, Scala, C, C++, C#, Swift and PHP.

- The architecture map: boxes named by role, opened down to files and symbols; links from
  syntax drawn apart from links confirmed by a manifest or a language server; a stable layout.
- The review diff since « Mark as reviewed », and the review debt per box.
- The whiteboard of the intended architecture, with typed shapes, rules and performance budgets,
  and the map compared with it.
- Plan, todo and session journal, written by the agents through MCP and shown live; the
  bridge, which says where the plan stands and where to look.
- Set up from the bridge for Claude Code, GitHub Copilot, Cursor and Codex: instructions, MCP
  server, hooks; `timonier doctor` checks and repairs.
- `timonier test`: the tests measured, their traces attached to the map, budgets checked,
  regressions detected; the Performances panel, whose « Run the tests » starts the first run
  and installs a missing test runner (cargo-nextest, gotestsum).
- For each language: its test runner set to write JUnit XML, and the OpenTelemetry SDK with a
  smoke test that `timonier doctor` runs.
- The technology inventory, with known vulnerabilities and a CycloneDX SBOM.
- The development process: Solo fast, Solo disciplined or Team, with GitHub issues, branches
  and pull requests through `gh`.
- The AI cost of each session and task, from Claude Code's telemetry.
- The `timonier` program shipped in the package, and put on the agents' PATH.
