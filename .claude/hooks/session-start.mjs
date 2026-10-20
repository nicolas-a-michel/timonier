#!/usr/bin/env node
// SessionStart hook: shows the current milestone of .timonier/plan.json and its remaining tasks
// (ADR 0035); without plan.json, the current PLAN.md phase (ADR 0002).
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

/** The states of a task in plan.json (ADR 0029), as the human reads them. */
const STATES = { todo: "à faire", in_progress: "en cours", blocked: "bloquée", done: "faite", dropped: "abandonnée" };

/**
 * The milestone where the work is (review n° 2 of Phase 6): the one with a task in progress;
 * else the last one with a task done, unless it is finished, then the next one not finished;
 * else the first not finished. As the bridge picks it (webview/src/tracking/Bridge.tsx).
 */
function current(milestones, unfinished) {
  const underway = milestones.find((m) => m.tasks.some((t) => t.status === "in_progress"));
  if (underway) return underway;
  const last = milestones.findLastIndex((m) => m.tasks.some((t) => t.status === "done"));
  if (last >= 0) {
    if (unfinished(milestones[last])) return milestones[last];
    const next = milestones.slice(last + 1).find(unfinished);
    if (next) return next;
  }
  return milestones.find(unfinished);
}

/**
 * Since the switch (ADR 0035), plan.json holds the tasks: the milestone where the work is (see
 * `current`), and its tasks left. `null` when it does not read, or lacks a part.
 */
export function summariseTracking(json) {
  let plan;
  try {
    plan = JSON.parse(json);
  } catch {
    return null;
  }
  const milestones = Array.isArray(plan?.milestones) ? plan.milestones : null;
  // A file edited by hand may lack a part: PLAN.md then, as for a file that does not read.
  const wellFormed = (m) =>
    typeof m?.id === "string" &&
    typeof m?.title === "string" &&
    Array.isArray(m?.tasks) &&
    m.tasks.every((t) => typeof t?.id === "string" && typeof t?.title === "string" && typeof t?.status === "string");
  if (!milestones || !milestones.every(wellFormed)) return null;
  const counted = (m) => m.tasks.filter((t) => t.status !== "dropped");
  const unfinished = (m) => counted(m).some((t) => t.status !== "done");
  const milestone = current(milestones, unfinished);
  if (!milestone) {
    return {
      line: "Timonier · every milestone of .timonier/plan.json is done.",
      context: ".timonier/plan.json: every milestone is done. No remaining task.",
    };
  }
  const remaining = counted(milestone).filter((t) => t.status !== "done");
  const done = counted(milestone).length - remaining.length;
  const line =
    `Timonier · ${milestone.title} · ` +
    `${done}/${counted(milestone).length} tâches faites, ${remaining.length} restantes`;
  const listed = remaining
    .slice(0, MAX_TASKS_LISTED)
    .map((t) => `- ${t.id} ${truncate(t.title, 200)} (${STATES[t.status] ?? t.status})`);
  if (remaining.length > MAX_TASKS_LISTED)
    listed.push(`- … and ${remaining.length - MAX_TASKS_LISTED} more (get_plan)`);
  const context = [
    `.timonier/plan.json (the tasks since the switch, ADR 0035): the current milestone is ${milestone.id} ${milestone.title}.`,
    `${done} of ${counted(milestone).length} tasks are done.`,
    remaining.length
      ? "Remaining tasks (their criteria and detail are in PLAN.md):"
      : "No task left in this milestone; it awaits review and the human's go-ahead.",
    ...listed,
    "Work through Timonier's MCP tools: session_start for each task, todo_add / todo_complete as you go, plan_update_task when a task changes state, session_end with a summary.",
    "Project rules: one phase at a time; stop for the human's go-ahead at the end of each phase (see CLAUDE.md).",
  ].join("\n");
  return { line, context };
}

export function summarisePlan(markdown) {
  const phases = parsePlan(markdown);
  if (phases.length === 0) {
    return {
      line: "Timonier · PLAN.md has no « ## Phase N — … » section yet.",
      context:
        "PLAN.md exists but contains no phase section in the expected format (see docs/adr/0002-format-de-plan-md.md).",
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
  if (remaining.length > MAX_TASKS_LISTED)
    listed.push(`- … and ${remaining.length - MAX_TASKS_LISTED} more (see PLAN.md)`);
  const context = [
    `PLAN.md: the current phase is Phase ${phase.number} — ${phase.title} (status: ${status}).`,
    `${done} of ${phase.tasks.length} tasks are done.`,
    remaining.length
      ? "Remaining tasks:"
      : "No task left unchecked in this phase; it awaits review and the human's go-ahead.",
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
  const trackingFile = path.join(root, ".timonier", "plan.json");
  const tracked = existsSync(trackingFile) ? summariseTracking(readFileSync(trackingFile, "utf8")) : null;
  // plan.json there but unreadable: PLAN.md holds no task state since the switch (ADR 0035).
  const broken = !tracked && existsSync(trackingFile);
  const fromPlan = (markdown) => {
    const summary = summarisePlan(markdown);
    if (!broken) return summary;
    return {
      line: `Timonier · .timonier/plan.json does not read: task states unknown (PLAN.md shows the phase only) · ${summary.line}`,
      context: `.timonier/plan.json exists but does not read, or lacks a part: the task states are unknown. Since the switch (ADR 0035), PLAN.md holds no task state: every task below reads as not done. Run \`timonier doctor\` and repair plan.json before relying on this list.\n${summary.context}`,
    };
  };
  const { line, context } = tracked
    ? tracked
    : existsSync(planFile)
      ? fromPlan(readFileSync(planFile, "utf8"))
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
