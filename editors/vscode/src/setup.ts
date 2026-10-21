/**
 * The bridge's setup, without a terminal (T-118): the agents `timonier init` would configure,
 * then init itself for those chosen, its output in Timonier's log.
 */
import { execFile } from "node:child_process";

import type { AgentChoice } from "./protocol/generated";

/** What `timonier init` takes at most: a project of any size is set up in seconds. */
const INIT_MS = 120_000;

interface Log {
  info(message: string): void;
  error(message: string): void;
}

function run(program: string, args: string[], root: string): Promise<{ code: number; out: string; err: string }> {
  return new Promise((resolve) => {
    execFile(program, args, { cwd: root, timeout: INIT_MS, maxBuffer: 4 << 20 }, (error, out, err) => {
      const code = error ? (typeof error.code === "number" ? error.code : 1) : 0;
      resolve({ code, out: String(out), err: String(err) || (error && !err ? error.message : "") });
    });
  });
}

/** `timonier init --detect`: every agent, whether this machine has it. */
export async function detectAgents(program: string, root: string): Promise<AgentChoice[]> {
  const { code, out } = await run(program, ["init", "--detect", "--root", root], root);
  if (code !== 0) return [];
  try {
    const parsed = JSON.parse(out) as { agents?: AgentChoice[] };
    return Array.isArray(parsed.agents) ? parsed.agents : [];
  } catch {
    return [];
  }
}

/** `timonier init --yes --agents …`: ok, or the last line it said. */
export async function setUp(
  program: string,
  root: string,
  agents: string[],
  log: Log,
): Promise<{ ok: boolean; detail: string | null }> {
  const args = ["init", "--yes", "--root", root, ...(agents.length > 0 ? ["--agents", agents.join(",")] : [])];
  log.info(`timonier ${args.join(" ")}`);
  const { code, out, err } = await run(program, args, root);
  for (const line of `${out}\n${err}`.split("\n").filter((l) => l.trim() !== "")) log.info(`init: ${line}`);
  if (code === 0) return { ok: true, detail: null };
  const last = err.trim().split("\n").pop() ?? null;
  log.error(`timonier init failed (exit ${code})`);
  return { ok: false, detail: last ? last.replace(/^timonier: /, "") : null };
}
