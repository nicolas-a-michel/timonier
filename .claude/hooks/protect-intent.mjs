#!/usr/bin/env node
// PreToolUse hook: denies any modification of .timonier/intent.json.
// The whiteboard has a single author, the human (docs/spec.md, « Tableau blanc »).
// File tools are checked exactly; Bash commands with a best-effort heuristic.
// The permission deny rule in .claude/settings.json is the second safety net.

import { emit, isIntentFile, isMain, readHookInput, runHook, toPosix } from "./lib.mjs";

const REASON =
  ".timonier/intent.json is the whiteboard: its only author is the human developer. " +
  "Claude never writes it, even partially. Read it (or use the MCP tool get_intended_architecture " +
  "once it exists) and ask the human to change the drawing if the intent must evolve.";

const MENTIONS_INTENT = /\.timonier[\\/]+intent\.json/;
// Redirection or tee into the file.
const REDIRECT_INTO = /(>>?|\btee\b(\s+-a)?)\s*["']?[^\s"';|&]*\.timonier[\\/]+intent\.json/;
// In-place editors and file-mutating commands (deliberately broad: a false positive costs
// a question to the human, a false negative breaks the single-author rule).
const MUTATING_COMMAND =
  /\b(sed|perl|ruby)\b[^|;&]*\s-i|\b(mv|cp|rm|truncate|dd|install|ln|touch|chmod|chown|patch|sponge)\b|\bgit\s+(checkout|restore|reset|apply|mv|rm|stash)\b|\bwriteFile(Sync)?\b|\bopen\([^)]*["'][wa]/;

// Each simple command is judged on its own, so that `cat intent.json; rm other` stays allowed.
const COMMAND_SEPARATOR = /\|\||&&|[;|&\n]/;

function segmentWritesIntent(segment) {
  return MENTIONS_INTENT.test(segment) && (REDIRECT_INTO.test(segment) || MUTATING_COMMAND.test(segment));
}

/** Returns the denial reason when the tool call would modify the whiteboard, else null. */
export function intentViolation(toolName, toolInput = {}) {
  if (toolName === "Bash" || toolName === "PowerShell") {
    const segments = String(toolInput.command ?? "").split(COMMAND_SEPARATOR);
    return segments.some(segmentWritesIntent) ? REASON : null;
  }
  const target = toolInput.file_path ?? toolInput.notebook_path;
  return target && isIntentFile(toPosix(target)) ? REASON : null;
}

async function main() {
  const input = await readHookInput();
  const reason = intentViolation(input.tool_name, input.tool_input);
  if (!reason) return;
  emit({
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: "deny",
      permissionDecisionReason: reason,
    },
  });
}

if (isMain(import.meta.url)) runHook("protect-intent", main);
