/**
 * Builds the whiteboard for production before its tests (PLAN.md 7.1): what the extension
 * ships, not the development server. Into node_modules/.cache, away from `dist/`.
 */
import { execFileSync } from "node:child_process";
import path from "node:path";

export const PRODUCTION = path.join(
  import.meta.dirname,
  "..",
  "..",
  "node_modules",
  ".cache",
  "timonier",
  "whiteboard",
);

export default function build(): void {
  execFileSync(
    "npx",
    ["vite", "build", "--mode", "whiteboard", "--outDir", PRODUCTION, "--emptyOutDir", "--logLevel", "warn"],
    {
      cwd: path.join(import.meta.dirname, "..", ".."),
      stdio: "inherit",
    },
  );
}
