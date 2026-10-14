#!/usr/bin/env node
// SessionStart hook: shows the current PLAN.md phase and its remaining tasks.
// - systemMessage: one line for the human;
// - additionalContext: the full list for Claude.

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import {
  currentPhase,
  emit,
  isMain,
  loadState,
  parsePlan,
  projectDir,
  pruneState,
  readHookInput,
  runHook,
  saveState,
  truncate,
} from "./lib.mjs";

const MAX_TASKS_LISTED = 25;

export function summarisePlan(markdown) {
  const phases = parsePlan(markdown);
  if (phases.length === 0) {
    return {
      line: "Timonier · PLAN.md has no « ## Phase N — … » section yet.",
      context: "PLAN.md exists but contains no phase section in the expected format (see docs/adr/0002-plan-md-format.md).",
    };
  }
  const phase = currentPhase(phases);
  if (!phase) {
    return {
      line: "Timonier · every phase in PLAN.md is finished.",
      context: "PLAN.md: every phase is marked finished. No remaining task.",
    };
  }
  const remaining = phase.tasks.filter((t) => !t.done);
  const done = phase.tasks.length - remaining.length;
  const status = phase.status || "statut non renseigné";
  const line =
    `Timonier · Phase ${phase.number} — ${phase.title} (${status}) · ` +
    `${done}/${phase.tasks.length} tâches faites, ${remaining.length} restantes`;
  const listed = remaining.slice(0, MAX_TASKS_LISTED).map((t) => `- ${truncate(t.text, 200)}`);
  if (remaining.length > MAX_TASKS_LISTED) listed.push(`- … and ${remaining.length - MAX_TASKS_LISTED} more (see PLAN.md)`);
  const context = [
    `PLAN.md: the current phase is Phase ${phase.number} — ${phase.title} (status: ${status}).`,
    `${done} of ${phase.tasks.length} tasks are done.`,
    remaining.length ? "Remaining tasks:" : "No task left unchecked in this phase; it awaits review and the human's go-ahead.",
    ...listed,
    "Project rules: one phase at a time; stop for the human's go-ahead at the end of each phase (see CLAUDE.md).",
  ].join("\n");
  return { line, context };
}

async function main() {
  const input = await readHookInput();
  const root = projectDir(input);

  pruneState(root);
  const state = loadState(root, input.session_id);
  if (!state.sessionStartedAt) state.sessionStartedAt = Date.now();
  saveState(root, input.session_id, state);

  const planFile = path.join(root, "PLAN.md");
  const { line, context } = existsSync(planFile)
    ? summarisePlan(readFileSync(planFile, "utf8"))
    : {
        line: "Timonier · PLAN.md is missing.",
        context: "PLAN.md does not exist yet. Phase 0 of the start-up prompt requires writing it.",
      };

  emit({
    systemMessage: line,
    hookSpecificOutput: { hookEventName: "SessionStart", additionalContext: context },
  });
}

if (isMain(import.meta.url)) runHook("session-start", main);
