/**
 * The Performances panel's « Run the tests »: the commands `.timonier/tests.json` records, shown
 * before they run, since this machine approves them by this click (Q84); then `timonier test`,
 * which installs a runner's missing program first, in a terminal where its output shows.
 */
import { readFileSync } from "node:fs";
import path from "node:path";

/** The commands of the recorded runners; `null` when the project is not set up. */
export function recordedCommands(root: string): string[] | null {
  let text: string;
  try {
    text = readFileSync(path.join(root, ".timonier", "tests.json"), "utf8");
  } catch {
    return null;
  }
  try {
    const tests = JSON.parse(text) as { runners?: { command?: unknown }[] };
    return (tests.runners ?? [])
      .map((runner) => runner.command)
      .filter((command): command is string => typeof command === "string" && command !== "");
  } catch {
    return null;
  }
}

/** What the terminal runs: the program itself, no shell to quote for. */
export const TEST_ARGS = ["test", "--approve", "--install"];
