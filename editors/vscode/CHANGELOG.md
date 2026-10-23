# Changelog

## 0.1.3 — pre-release

- The MCP server answers agents that speak the protocol's 2026-07-28 revision: their tool
  calls no longer fail with « missing required resultType ».
- SCIP indexes confirm the map's links: after each commit and at start, Timonier runs the
  indexer of each language that has one (rust-analyzer, scip-typescript, scip-python, scip-go,
  scip-java, scip-clang, scip-dotnet), in the background and at low priority. Timonier installs
  an indexer only when you approve it: « Install the SCIP Indexers » or `tim scip --install`.
  On six open-source projects, 96 to 100 % of the links between files are then confirmed.
- The map says how sure each link is: solid when confirmed, long dashes when partly, short
  dashes when known from syntax alone; the « Certainty » layer gives each box and the map
  their share of confirmed links.
- F2 or a double click renames a box and changes its role; the name is kept in
  `.timonier/names.json`, shared through git. Agents can propose names, roles and a short
  summary, which shows on the map; the names you give stay yours.
- Agents read the map as you see it, through the MCP tools `query_graph` and
  `get_component`.
- A C# `using` and a Go import of a whole package lead to the files they use, not to the
  first file of the namespace or package.

## 0.1.2 — pre-release

- The map's Performances layer draws a bar of each box's share of the run's own time, with the
  share in figures: the shares of one level make 100%.
- A folder of several repositories opens on one box per project, in turquoise with a thicker
  outline, with the roles inside each.
  A box's role now counts its libraries too, so that one binary among libraries no longer makes
  them an entry point; developer tools (`xtask`, `tools/`, `vendor/`…) are never entry points;
  a group named after its `mod.rs`, `index.ts` or `__init__.py` takes its folder's name.
- « Set up » waits for the list of agents instead of opening a terminal, and its boxes can be
  ticked as soon as the list comes.

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
