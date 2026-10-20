// Tests for the Claude Code hooks. Run: node --test .claude/hooks/
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { after, describe, test } from "node:test";
import { fileURLToPath } from "node:url";

import { formatterFor } from "./format-file.mjs";
import { currentPhase, isIntentFile, parsePlan } from "./lib.mjs";
import { intentViolation } from "./protect-intent.mjs";
import { summarisePlan } from "./session-start.mjs";
import { classify, decide, parsePorcelainZ } from "./stop-checks.mjs";
import { openTurn } from "./turn-start.mjs";

const HOOKS_DIR = path.dirname(fileURLToPath(import.meta.url));
const temps = [];
after(() => temps.forEach((dir) => rmSync(dir, { recursive: true, force: true })));

function tempProject() {
  const dir = mkdtempSync(path.join(os.tmpdir(), "timonier-hooks-"));
  temps.push(dir);
  return dir;
}

function runScript(name, payload, root) {
  const result = spawnSync("node", [path.join(HOOKS_DIR, name)], {
    input: JSON.stringify(payload),
    encoding: "utf8",
    env: { ...process.env, CLAUDE_PROJECT_DIR: root },
  });
  return { ...result, json: result.stdout ? JSON.parse(result.stdout) : null };
}

const PLAN = `# PLAN

## Phase 0 — Environnement
Statut : terminée
- [x] 0.1 Lire la spec

## Phase 1 — Squelette
Statut : en cours
- [x] 1.1 Licence
- [ ] 1.2 Workspace Cargo — critère : cargo test
  passe sur Linux
  - [ ] indented detail, not a task
- [ ] 1.3 Extension vide

## Phase 2 — Graphe
Statut : à faire
- [ ] 2.1 Types

## Questions ouvertes
- [ ] Q1 not a task of any phase
`;

describe("PLAN.md parsing", () => {
  test("parses phases, statuses and tasks", () => {
    const phases = parsePlan(PLAN);
    assert.equal(phases.length, 3);
    assert.deepEqual(
      phases.map((p) => [p.number, p.status, p.tasks.length]),
      [
        [0, "terminée", 1],
        [1, "en cours", 3],
        [2, "à faire", 1],
      ],
    );
  });

  test("the phase marked « en cours » is current", () => {
    assert.equal(currentPhase(parsePlan(PLAN)).number, 1);
  });

  test("without « en cours », the first unfinished phase is current", () => {
    const plan = PLAN.replace("Statut : en cours", "Statut : à faire");
    assert.equal(currentPhase(parsePlan(plan)).number, 1);
  });

  test("a task without its box is a task too (ADR 0035)", () => {
    const plain = PLAN.replace("- [ ] 1.3 Extension vide", "- 1.3 Extension vide");
    assert.equal(parsePlan(plain)[1].tasks.length, 3);
    assert.equal(parsePlan(plain)[1].tasks[2].text, "1.3 Extension vide");
  });

  test("summary lists only the remaining tasks of the current phase", () => {
    const { line, context } = summarisePlan(PLAN);
    assert.match(line, /Phase 1 — Squelette \(en cours\) · 1\/3 tâches faites, 2 restantes/);
    assert.match(context, /1\.2 Workspace Cargo — critère : cargo test passe sur Linux\n/);
    assert.doesNotMatch(context, /indented detail/);
    assert.doesNotMatch(context, /1\.1 Licence/);
    assert.doesNotMatch(context, /Q1/);
  });
});

describe("intent.json protection", () => {
  const intent = "/repo/.timonier/intent.json";
  test("matches the whiteboard file on every OS", () => {
    assert.ok(isIntentFile(intent));
    assert.ok(isIntentFile("C:\\repo\\.timonier\\intent.json"));
    assert.ok(!isIntentFile("/repo/.timonier/plan.json"));
    assert.ok(!isIntentFile("/repo/docs/intent.json"));
  });

  test("denies file tools on the whiteboard", () => {
    assert.ok(intentViolation("Write", { file_path: intent }));
    assert.ok(intentViolation("Edit", { file_path: intent }));
    assert.ok(intentViolation("NotebookEdit", { notebook_path: intent }));
    assert.equal(intentViolation("Edit", { file_path: "/repo/src/main.rs" }), null);
  });

  test("denies Bash commands that write it, allows reads", () => {
    for (const command of [
      "echo '{}' > .timonier/intent.json",
      "jq . x.json | tee .timonier/intent.json",
      "sed -i 's/a/b/' .timonier/intent.json",
      "rm .timonier/intent.json",
      "git checkout -- .timonier/intent.json",
      "node -e \"require('fs').writeFileSync('.timonier/intent.json', '{}')\"",
    ]) {
      assert.ok(intentViolation("Bash", { command }), command);
    }
    for (const command of [
      "cat .timonier/intent.json",
      "jq .components .timonier/intent.json",
      "git diff .timonier/intent.json",
      "ls src",
      // Mentions the file and mutates something else, in separate commands: allowed.
      'echo \'{"tool_input":{"file_path":".timonier/intent.json"}}\' | node hook.mjs; rm scratch.rs && touch a.mjs',
    ]) {
      assert.equal(intentViolation("Bash", { command }), null, command);
    }
  });

  test("the script prints a PreToolUse deny decision", () => {
    const root = tempProject();
    const out = runScript(
      "protect-intent.mjs",
      { tool_name: "Write", tool_input: { file_path: path.join(root, ".timonier/intent.json") } },
      root,
    );
    assert.equal(out.status, 0);
    assert.equal(out.json.hookSpecificOutput.permissionDecision, "deny");
    const allowed = runScript(
      "protect-intent.mjs",
      { tool_name: "Write", tool_input: { file_path: path.join(root, "a.rs") } },
      root,
    );
    assert.equal(allowed.stdout, "");
  });
});

describe("formatting", () => {
  test("rustfmt uses edition 2024 unless a rustfmt.toml exists", () => {
    const root = tempProject();
    const file = path.join(root, "crates/x/src/lib.rs");
    assert.deepEqual(formatterFor(file, root).args, ["--edition", "2024", file]);
    writeFileSync(path.join(root, "rustfmt.toml"), 'edition = "2024"\n');
    assert.deepEqual(formatterFor(file, root).args, [file]);
  });

  test("prettier is skipped when not installed, used when present", () => {
    const root = tempProject();
    const file = path.join(root, "webview/src/App.tsx");
    assert.equal(formatterFor(file, root), null);
    mkdirSync(path.join(root, "node_modules/.bin"), { recursive: true });
    writeFileSync(path.join(root, "node_modules/.bin/prettier"), "");
    assert.equal(formatterFor(file, root).cmd, path.join(root, "node_modules/.bin/prettier"));
  });

  test("prettier covers what the CI checks, Markdown excluded", () => {
    const root = tempProject();
    mkdirSync(path.join(root, "node_modules/.bin"), { recursive: true });
    writeFileSync(path.join(root, "node_modules/.bin/prettier"), "");
    for (const rel of ["a.mjs", "b.js", ".github/workflows/ci.yml", "webview/index.html", "c.json", "d.css"]) {
      assert.ok(formatterFor(path.join(root, rel), root), rel);
    }
    assert.equal(formatterFor(path.join(root, "PLAN.md"), root), null);
  });

  test("never formats dependencies, build output, lockfiles or files outside the project", () => {
    const root = tempProject();
    for (const rel of [
      "node_modules/a/index.ts",
      "target/debug/build.rs",
      "package-lock.json",
      ".timonier/plan.json",
      "../elsewhere.rs",
    ]) {
      assert.equal(formatterFor(path.join(root, rel), root), null, rel);
    }
  });

  test("the script formats a Rust file in place", () => {
    const root = tempProject();
    const file = path.join(root, "lib.rs");
    writeFileSync(file, "fn  main( ){let x=1;}\n");
    const out = runScript("format-file.mjs", { tool_name: "Write", tool_input: { file_path: file } }, root);
    assert.equal(out.status, 0, out.stderr);
    assert.equal(readFileSync(file, "utf8"), "fn main() {\n    let x = 1;\n}\n");
  });

  test("a syntax error is reported to Claude as context", () => {
    const root = tempProject();
    const file = path.join(root, "broken.rs");
    writeFileSync(file, "fn main( {\n");
    const out = runScript("format-file.mjs", { tool_name: "Write", tool_input: { file_path: file } }, root);
    assert.equal(out.status, 0);
    assert.match(out.json.hookSpecificOutput.additionalContext, /rustfmt could not format broken\.rs/);
  });
});

describe("end-of-turn checks", () => {
  test("parses porcelain -z output, skipping rename sources", () => {
    assert.deepEqual(parsePorcelainZ(" M a.rs\0R  new.ts\0old.ts\0?? docs/x.md\0"), ["a.rs", "new.ts", "docs/x.md"]);
  });

  test("classifies changed files", () => {
    const c = classify(["crates/a/src/lib.rs", "webview/src/App.tsx", "docs/spec.md", "PLAN.md"]);
    assert.equal(c.rust, true);
    assert.equal(c.ts, true);
    assert.deepEqual(c.code, ["crates/a/src/lib.rs", "webview/src/App.tsx"]);
  });

  const fresh = { open: true, startedAt: 0, checkBlocks: 0, planReminded: false };
  const failure = { label: "cargo check", failed: true, output: "error[E0425]" };

  test("a failing check blocks once, then is only reported to the human", () => {
    const first = decide(fresh, { failures: [failure], planStale: false, changed: [] });
    assert.equal(first.output.decision, "block");
    assert.match(first.output.reason, /E0425/);
    const second = decide(first.turn, { failures: [failure], planStale: false, changed: [] });
    assert.equal(second.output.decision, undefined);
    assert.match(second.output.systemMessage, /still failing/);
    assert.equal(second.turn.open, false);
  });

  test("a stale PLAN.md triggers a single non-blocking reminder", () => {
    const first = decide(fresh, { failures: [], planStale: true, changed: ["a.rs"] });
    assert.equal(first.output.hookSpecificOutput.hookEventName, "Stop");
    assert.match(first.output.hookSpecificOutput.additionalContext, /PLAN\.md was not modified/);
    const second = decide(first.turn, { failures: [], planStale: true, changed: ["a.rs"] });
    assert.equal(second.output, null);
    assert.equal(second.turn.open, false);
  });

  test("a turn stays open across injected prompts", () => {
    const opened = openTurn({}, 100);
    assert.equal(openTurn(opened, 200).turn.startedAt, 100);
    assert.equal(openTurn({ turn: { open: false } }, 300).turn.startedAt, 300);
  });

  test("the script reminds about PLAN.md when code changed during the turn", () => {
    const root = tempProject();
    spawnSync("git", ["init", "-q"], { cwd: root });
    writeFileSync(path.join(root, "PLAN.md"), PLAN);
    const old = new Date(Date.now() - 3600_000);
    utimesSync(path.join(root, "PLAN.md"), old, old);
    const session = { session_id: "s1", cwd: root };
    runScript("turn-start.mjs", { ...session, hook_event_name: "UserPromptSubmit" }, root);
    writeFileSync(path.join(root, "tool.mjs"), "export {};\n");
    const out = runScript("stop-checks.mjs", { ...session, hook_event_name: "Stop", stop_hook_active: false }, root);
    assert.equal(out.status, 0, out.stderr);
    assert.match(out.json.hookSpecificOutput.additionalContext, /tool\.mjs/);
    const again = runScript("stop-checks.mjs", { ...session, hook_event_name: "Stop", stop_hook_active: true }, root);
    assert.equal(again.stdout, "");
  });

  const hasCargo = spawnSync("cargo", ["--version"]).status === 0;
  test("the script blocks once on a Rust compile error", { skip: !hasCargo && "cargo not installed" }, () => {
    const root = tempProject();
    spawnSync("git", ["init", "-q"], { cwd: root });
    writeFileSync(path.join(root, "PLAN.md"), PLAN);
    const session = { session_id: "s5", cwd: root };
    runScript("turn-start.mjs", session, root);
    writeFileSync(path.join(root, "Cargo.toml"), '[package]\nname = "probe"\nversion = "0.0.0"\nedition = "2024"\n');
    mkdirSync(path.join(root, "src"));
    writeFileSync(path.join(root, "src/lib.rs"), "pub fn f() -> u32 { undefined_name }\n");
    const out = runScript("stop-checks.mjs", { ...session, stop_hook_active: false }, root);
    assert.equal(out.status, 0, out.stderr);
    assert.equal(out.json.decision, "block");
    assert.match(out.json.reason, /cargo check/);
    assert.match(out.json.reason, /undefined_name/);
  });

  test("the script stays silent when nothing changed", () => {
    const root = tempProject();
    spawnSync("git", ["init", "-q"], { cwd: root });
    const session = { session_id: "s2", cwd: root };
    runScript("turn-start.mjs", session, root);
    const out = runScript("stop-checks.mjs", { ...session, stop_hook_active: false }, root);
    assert.equal(out.status, 0, out.stderr);
    assert.equal(out.stdout, "");
  });
});

describe("session start", () => {
  test("prints the current phase for the human and the tasks for Claude", () => {
    const root = tempProject();
    writeFileSync(path.join(root, "PLAN.md"), PLAN);
    const out = runScript("session-start.mjs", { session_id: "s3", source: "startup", cwd: root }, root);
    assert.equal(out.status, 0, out.stderr);
    assert.match(out.json.systemMessage, /Phase 1 — Squelette/);
    assert.equal(out.json.hookSpecificOutput.hookEventName, "SessionStart");
    assert.match(out.json.hookSpecificOutput.additionalContext, /1\.3 Extension vide/);
  });

  test("reads plan.json once it is there (ADR 0035): the milestone under way and its tasks left", () => {
    const root = tempProject();
    writeFileSync(path.join(root, "PLAN.md"), PLAN);
    mkdirSync(path.join(root, ".timonier"), { recursive: true });
    const task = (id, title, status) => ({ id, title, status });
    writeFileSync(
      path.join(root, ".timonier", "plan.json"),
      JSON.stringify({
        version: 1,
        milestones: [
          { id: "M-1", title: "Phase 5 — Revue", tasks: [task("T-1", "5.1 Diff", "done")] },
          {
            id: "M-2",
            title: "Phase 6 — Suivi",
            tasks: [
              task("T-2", "6.1 ADR", "todo"),
              task("T-3", "6.2 Proxy", "done"),
              task("T-4", "6.3 Bascule", "in_progress"),
              task("T-5", "6.4 Zed", "dropped"),
            ],
          },
          { id: "M-3", title: "Phase 7 — Tableau", tasks: [task("T-6", "7.1 Dessin", "todo")] },
        ],
      }),
    );
    const out = runScript("session-start.mjs", { session_id: "s5", source: "startup", cwd: root }, root);
    assert.equal(out.status, 0, out.stderr);
    assert.match(out.json.systemMessage, /Phase 6 — Suivi · 1\/3 tâches faites, 2 restantes/);
    const context = out.json.hookSpecificOutput.additionalContext;
    assert.match(context, /- T-4 6\.3 Bascule \(en cours\)/);
    assert.match(context, /- T-2 6\.1 ADR \(à faire\)/);
    assert.doesNotMatch(context, /Zed|Dessin|5\.1/);
    // A plan.json that reads but lacks a part (edited by hand): PLAN.md, no crash.
    writeFileSync(path.join(root, ".timonier", "plan.json"), '{"milestones":[{"id":"M-1","title":"x"}]}');
    const partial = runScript("session-start.mjs", { session_id: "s8", source: "startup", cwd: root }, root);
    assert.equal(partial.status, 0, partial.stderr);
    assert.match(partial.json.systemMessage, /Phase 1 — Squelette/);
    // And it says the states are unknown (review n° 4 of Phase 6).
    assert.match(partial.json.systemMessage, /plan\.json does not read: task states unknown/);
    assert.match(partial.json.hookSpecificOutput.additionalContext, /PLAN\.md holds no task state/);
    // A plan.json that does not read: PLAN.md as before.
    writeFileSync(path.join(root, ".timonier", "plan.json"), "{ broken");
    const fallback = runScript("session-start.mjs", { session_id: "s6", source: "startup", cwd: root }, root);
    assert.match(fallback.json.systemMessage, /Phase 1 — Squelette/);
  });

  test("skips a closed milestone where a task was left to do (review n° 2 of Phase 6)", () => {
    const root = tempProject();
    mkdirSync(path.join(root, ".timonier"), { recursive: true });
    const task = (id, status) => ({ id, title: id, status });
    writeFileSync(
      path.join(root, ".timonier", "plan.json"),
      JSON.stringify({
        version: 1,
        milestones: [
          { id: "M-1", title: "Phase 0", tasks: [task("T-1", "done"), task("T-2", "todo")] },
          { id: "M-2", title: "Phase 6", tasks: [task("T-3", "done")] },
          { id: "M-3", title: "Phase 7", tasks: [task("T-4", "todo")] },
        ],
      }),
    );
    const out = runScript("session-start.mjs", { session_id: "s7", source: "startup", cwd: root }, root);
    assert.match(out.json.systemMessage, /Phase 7 · 0\/1 tâches faites/);
  });

  test("reports a missing PLAN.md", () => {
    const root = tempProject();
    const out = runScript("session-start.mjs", { session_id: "s4", source: "startup", cwd: root }, root);
    assert.match(out.json.systemMessage, /PLAN\.md is missing/);
  });
});
