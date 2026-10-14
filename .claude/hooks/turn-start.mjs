#!/usr/bin/env node
// UserPromptSubmit hook: records when the current turn started, so that the Stop hook
// can tell which files changed during the turn (including edits made through Bash).
// A turn stays open until the Stop hook closes it: prompts injected mid-turn
// (background agents reporting back, scheduled tasks) do not move its start.

import { isMain, loadState, projectDir, readHookInput, runHook, saveState } from "./lib.mjs";

export function openTurn(state, now = Date.now()) {
  if (state.turn?.open) return state;
  return { ...state, turn: { open: true, startedAt: now, checkBlocks: 0, planReminded: false } };
}

async function main() {
  const input = await readHookInput();
  const root = projectDir(input);
  saveState(root, input.session_id, openTurn(loadState(root, input.session_id)));
}

if (isMain(import.meta.url)) runHook("turn-start", main);
