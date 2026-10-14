#!/usr/bin/env node
// PostToolUse hook (Edit|Write): formats the file Claude just touched.
// - .rs                      → rustfmt (edition 2024 unless a rustfmt.toml decides)
// - every type `prettier --check .` covers in CI (TS/JS, JSON, CSS, HTML, YAML)
//                            → prettier from the nearest node_modules (skipped if not installed;
//                              .prettierignore still applies, so Markdown is left alone)
// A formatter failure (usually a syntax error) is reported to Claude as context, never blocking.

import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { emit, isMain, projectDir, readHookInput, runHook, toPosix } from "./lib.mjs";

const PRETTIER_EXT = new Set([
  ".ts",
  ".tsx",
  ".mts",
  ".cts",
  ".js",
  ".jsx",
  ".mjs",
  ".cjs",
  ".json",
  ".css",
  ".html",
  ".yml",
  ".yaml",
]);
const SKIPPED = /(^|\/)(node_modules|target|dist|out|\.git|\.timonier|\.claude\/state)\//;
const LOCKFILES = new Set(["package-lock.json", "Cargo.lock", "pnpm-lock.yaml"]);
const TIMEOUT_MS = 20_000;

/** Walks up from `dir` to `root` (inclusive) and returns the first existing candidate. */
function findUp(dir, root, candidates) {
  let current = dir;
  while (true) {
    for (const candidate of candidates) {
      const full = path.join(current, candidate);
      if (existsSync(full)) return full;
    }
    if (current === root || path.dirname(current) === current) return null;
    current = path.dirname(current);
  }
}

/** Decides which formatter applies; returns { cmd, args } or null. */
export function formatterFor(file, root) {
  const posix = toPosix(path.relative(root, file));
  if (posix.startsWith("..") || SKIPPED.test(`/${posix}`) || LOCKFILES.has(path.basename(file))) return null;
  const ext = path.extname(file);
  if (ext === ".rs") {
    const config = findUp(path.dirname(file), root, ["rustfmt.toml", ".rustfmt.toml"]);
    return { cmd: "rustfmt", args: config ? [file] : ["--edition", "2024", file] };
  }
  if (PRETTIER_EXT.has(ext)) {
    const bin = findUp(path.dirname(file), root, [path.join("node_modules", ".bin", "prettier")]);
    return bin ? { cmd: bin, args: ["--write", "--log-level", "warn", file] } : null;
  }
  return null;
}

async function main() {
  const input = await readHookInput();
  const file = input.tool_input?.file_path;
  if (!file || !existsSync(file)) return;
  const root = projectDir(input);
  const formatter = formatterFor(path.resolve(file), root);
  if (!formatter) return;

  const result = spawnSync(formatter.cmd, formatter.args, { cwd: root, encoding: "utf8", timeout: TIMEOUT_MS });
  if (result.error?.code === "ENOENT") return; // Formatter not installed: nothing to do.
  if (result.status === 0) return;

  const details = (result.stderr || result.stdout || String(result.error ?? ""))
    .trim()
    .split("\n")
    .slice(0, 20)
    .join("\n");
  emit({
    hookSpecificOutput: {
      hookEventName: "PostToolUse",
      additionalContext: `${path.basename(formatter.cmd)} could not format ${path.relative(root, file)} (the file is unchanged by the formatter):\n${details}`,
    },
  });
}

if (isMain(import.meta.url)) runHook("format-file", main);
