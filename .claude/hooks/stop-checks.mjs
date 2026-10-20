#!/usr/bin/env node
// Stop hook: quick checks at the end of each turn.
// 1. Files changed during the turn are found from `git status` + mtime ≥ turn start
//    (so edits made through Bash count too).
// 2. Rust changed → `cargo check`; TypeScript changed → `npm run typecheck`.
//    A failure blocks the stop once so Claude fixes it; a second failure is only
//    reported to the human (no loop).
// 3. Code changed but PLAN.md untouched → one non-blocking reminder to Claude.

import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { emit, isMain, loadState, projectDir, readHookInput, runHook, saveState, toPosix } from "./lib.mjs";

const RUST_RE = /\.rs$|(^|\/)Cargo\.toml$/;
const TS_RE = /\.(ts|tsx|mts|cts)$|(^|\/)(package\.json|tsconfig[^/]*\.json)$/;
const CODE_RE = /\.(rs|ts|tsx|mts|cts|js|jsx|mjs|cjs|css|html|scm|sql|sh|toml|ya?ml)$|(^|\/)package\.json$/;
const WALK_SKIP = new Set([".git", "node_modules", "target", "dist", "out"]);
const WALK_SKIP_REL = new Set([".claude/state"]);
const OUTPUT_MAX = 6000;
const CARGO_TIMEOUT_MS = 240_000;
const TSC_TIMEOUT_MS = 180_000;

// File timestamps come from a coarse kernel clock that can lag Date.now() by a few ms.
const CLOCK_SLACK_MS = 1000;

function mtimeAtLeast(file, since) {
  try {
    return statSync(file).mtimeMs >= since - CLOCK_SLACK_MS;
  } catch {
    return false; // Deleted or unreadable: not counted.
  }
}

/** Parses `git status --porcelain=v1 -z` output into paths (rename sources skipped). */
export function parsePorcelainZ(output) {
  const items = output.split("\0").filter(Boolean);
  const paths = [];
  for (let i = 0; i < items.length; i++) {
    const entry = items[i];
    const xy = entry.slice(0, 2);
    paths.push(entry.slice(3));
    if (/[RC]/.test(xy)) i++; // The next item is the original path of a rename/copy.
  }
  return paths;
}

function walk(root, dir = root, out = []) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (WALK_SKIP.has(entry.name)) continue;
    const full = path.join(dir, entry.name);
    const rel = toPosix(path.relative(root, full));
    if (entry.isDirectory()) {
      if (!WALK_SKIP_REL.has(rel)) walk(root, full, out);
    } else if (out.length < 20_000) out.push(rel);
  }
  return out;
}

/** Files (relative, POSIX) modified at or after `since`. */
export function changedFiles(root, since) {
  const git = spawnSync("git", ["status", "--porcelain=v1", "-z", "--untracked-files=all"], {
    cwd: root,
    encoding: "utf8",
  });
  const candidates = git.status === 0 ? parsePorcelainZ(git.stdout) : walk(root);
  return candidates.filter((rel) => mtimeAtLeast(path.join(root, rel), since));
}

export function classify(files) {
  const code = files.filter((f) => CODE_RE.test(f));
  return { rust: code.some((f) => RUST_RE.test(f)), ts: code.some((f) => TS_RE.test(f)), code };
}

function tail(text) {
  const trimmed = text.trim();
  return trimmed.length <= OUTPUT_MAX ? trimmed : `…\n${trimmed.slice(-OUTPUT_MAX)}`;
}

function run(label, cmd, args, root, timeout) {
  const result = spawnSync(cmd, args, { cwd: root, encoding: "utf8", timeout });
  if (result.error?.code === "ENOENT") return { label, skipped: `${cmd} not found` };
  if (result.error?.code === "ETIMEDOUT") return { label, failed: true, output: `timed out after ${timeout / 1000}s` };
  if (result.status === 0) return { label, failed: false };
  return { label, failed: true, output: tail(`${result.stdout}\n${result.stderr}`) };
}

function cargoCheck(root) {
  if (!existsSync(path.join(root, "Cargo.toml"))) return { label: "cargo check", skipped: "no Cargo.toml" };
  return run(
    "cargo check",
    "cargo",
    ["check", "--workspace", "--all-targets", "--quiet", "--message-format=short"],
    root,
    CARGO_TIMEOUT_MS,
  );
}

function typecheck(root) {
  const manifest = path.join(root, "package.json");
  if (!existsSync(manifest)) return { label: "typecheck", skipped: "no root package.json" };
  const scripts = JSON.parse(readFileSync(manifest, "utf8")).scripts ?? {};
  if (!scripts.typecheck) return { label: "typecheck", skipped: "no root `typecheck` script" };
  if (!existsSync(path.join(root, "node_modules")))
    return { label: "typecheck", skipped: "node_modules missing (npm install)" };
  return run("typecheck", "npm", ["run", "typecheck"], root, TSC_TIMEOUT_MS);
}

/**
 * Pure decision: given the turn state and the check results, returns
 * { output (JSON for Claude Code or null), turn (new turn state) }.
 */
export function decide(turn, { failures, planStale, changed }) {
  if (failures.length > 0) {
    const report = failures.map((f) => `### ${f.label}\n${f.output}`).join("\n\n");
    if (turn.checkBlocks < 1) {
      return {
        output: {
          decision: "block",
          reason:
            `The end-of-turn checks failed after this turn's changes:\n\n${report}\n\n` +
            "Fix these errors before ending the turn. This check blocks once per turn; a second failure is only reported to the human.",
        },
        turn: { ...turn, checkBlocks: turn.checkBlocks + 1 },
      };
    }
    return {
      output: {
        systemMessage: `Timonier · end-of-turn checks still failing (${failures.map((f) => f.label).join(", ")}).`,
      },
      turn: { open: false },
    };
  }
  if (planStale && !turn.planReminded) {
    const listed = changed.slice(0, 15).join(", ") + (changed.length > 15 ? ", …" : "");
    return {
      output: {
        hookSpecificOutput: {
          hookEventName: "Stop",
          additionalContext:
            `Code files changed during this turn (${changed.length}): ${listed}. PLAN.md was not modified during this turn. ` +
            "Project rule: task states go through Timonier's plan_update_task (.timonier/plan.json, ADR 0035); PLAN.md records each task's detail, deviations and decisions. " +
            "If a task changed state, update PLAN.md now; if none did, no update is needed and a one-line note is enough.",
        },
      },
      turn: { ...turn, planReminded: true },
    };
  }
  return { output: null, turn: { open: false } };
}

async function main() {
  const input = await readHookInput();
  const root = projectDir(input);
  const state = loadState(root, input.session_id);
  const turn = state.turn?.open
    ? state.turn
    : state.sessionStartedAt
      ? { open: true, startedAt: state.sessionStartedAt, checkBlocks: 0, planReminded: false }
      : null;
  if (!turn) return; // Hooks were installed mid-session: nothing reliable to compare against.

  const changed = changedFiles(root, turn.startedAt);
  const { rust, ts, code } = classify(changed);
  const results = [rust ? cargoCheck(root) : null, ts ? typecheck(root) : null].filter(Boolean);
  const failures = results.filter((r) => r.failed);
  const planStale = code.length > 0 && !mtimeAtLeast(path.join(root, "PLAN.md"), turn.startedAt);

  const decision = decide(turn, { failures, planStale, changed: code });
  saveState(root, input.session_id, { ...state, sessionStartedAt: undefined, turn: decision.turn });
  if (decision.output) emit(decision.output);
}

if (isMain(import.meta.url)) runHook("stop-checks", main);
