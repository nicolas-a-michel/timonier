// Shared helpers for the Claude Code hooks of this repository.
// Hooks are plain Node.js ES modules with no dependencies (see docs/adr/0001).

import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, unlinkSync, writeFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

/** True when the module is the entry point (not imported by the tests). */
export function isMain(importMetaUrl) {
  return Boolean(process.argv[1]) && importMetaUrl === pathToFileURL(process.argv[1]).href;
}

/** Runs a hook entry point; a crash is a non-blocking error (exit 1), never a silent pass. */
export function runHook(name, main) {
  main().catch((error) => {
    process.stderr.write(`${name} hook failed: ${error?.stack ?? error}\n`);
    process.exit(1);
  });
}

/** Reads the hook JSON payload from stdin. Returns {} on empty input. */
export async function readHookInput() {
  const chunks = [];
  for await (const chunk of process.stdin) chunks.push(chunk);
  const raw = Buffer.concat(chunks).toString("utf8").trim();
  return raw ? JSON.parse(raw) : {};
}

/** Project root: CLAUDE_PROJECT_DIR when Claude Code sets it, else the payload cwd. */
export function projectDir(input) {
  return process.env.CLAUDE_PROJECT_DIR || input.cwd || process.cwd();
}

/** Prints a single JSON object on stdout, as Claude Code expects. */
export function emit(output) {
  process.stdout.write(JSON.stringify(output));
}

/** Normalises a path to forward slashes so the same regexes work on every OS. */
export function toPosix(p) {
  return String(p ?? "").replaceAll("\\", "/");
}

/** True when the path designates the whiteboard file, whose only author is the human. */
export function isIntentFile(p) {
  return /(^|\/)\.timonier\/intent\.json$/.test(toPosix(p));
}

// ---------------------------------------------------------------------------
// Per-session state (gitignored: .claude/state/<session>.json)
// ---------------------------------------------------------------------------

const STATE_MAX_AGE_MS = 7 * 24 * 3600 * 1000;

function stateDir(root) {
  return path.join(root, ".claude", "state");
}

function statePath(root, sessionId) {
  const safe = String(sessionId || "unknown").replace(/[^A-Za-z0-9_-]/g, "_");
  return path.join(stateDir(root), `${safe}.json`);
}

export function loadState(root, sessionId) {
  try {
    return JSON.parse(readFileSync(statePath(root, sessionId), "utf8"));
  } catch {
    return {};
  }
}

export function saveState(root, sessionId, state) {
  mkdirSync(stateDir(root), { recursive: true });
  writeFileSync(statePath(root, sessionId), JSON.stringify(state, null, 2));
}

/** Removes state files of sessions older than a week. */
export function pruneState(root, now = Date.now()) {
  const dir = stateDir(root);
  if (!existsSync(dir)) return;
  for (const name of readdirSync(dir)) {
    const file = path.join(dir, name);
    try {
      if (now - statSync(file).mtimeMs > STATE_MAX_AGE_MS) unlinkSync(file);
    } catch {
      // Another session may have removed it concurrently.
    }
  }
}

// ---------------------------------------------------------------------------
// PLAN.md parsing (format defined in docs/adr/0002-format-de-plan-md.md)
// ---------------------------------------------------------------------------

const PHASE_RE = /^##\s+Phase\s+(\d+)\s*[—–-]\s*(.+?)\s*$/;
const STATUS_RE = /^Statut\s*:\s*(.+?)\s*$/i;
// Only top-level checkboxes are tasks; indented lines are details.
const TASK_RE = /^-\s+\[([ xX])\]\s+(.+?)\s*$/;
const DONE_STATUS_RE = /termin|valid/i;
const ACTIVE_STATUS_RE = /en cours/i;

/**
 * Parses PLAN.md into phases: { number, title, status, tasks: [{ done, text }] }.
 * Only top-level "## Phase N — Title" sections are considered.
 */
export function parsePlan(markdown) {
  const phases = [];
  let current = null;
  let lastTask = null;
  for (const line of markdown.split(/\r?\n/)) {
    const heading = PHASE_RE.exec(line);
    if (heading) {
      current = { number: Number(heading[1]), title: heading[2], status: "", tasks: [] };
      phases.push(current);
      lastTask = null;
      continue;
    }
    if (/^##\s/.test(line)) {
      current = null;
      lastTask = null;
      continue;
    }
    if (!current) continue;
    // An indented line that is not a bullet continues the task's first paragraph.
    if (lastTask && /^\s{2,}\S/.test(line) && !/^\s*-\s/.test(line)) {
      lastTask.text += ` ${line.trim()}`;
      continue;
    }
    lastTask = null;
    const status = STATUS_RE.exec(line);
    if (status && !current.status) {
      current.status = status[1];
      continue;
    }
    const task = TASK_RE.exec(line);
    if (task) {
      lastTask = { done: task[1] !== " ", text: task[2] };
      current.tasks.push(lastTask);
    }
  }
  return phases;
}

/** Current phase: the one marked "en cours", else the first one not finished. */
export function currentPhase(phases) {
  return (
    phases.find((p) => ACTIVE_STATUS_RE.test(p.status)) ??
    phases.find((p) => !DONE_STATUS_RE.test(p.status) && p.tasks.some((t) => !t.done)) ??
    null
  );
}

export function truncate(text, max) {
  return text.length <= max ? text : `${text.slice(0, max - 1)}…`;
}
