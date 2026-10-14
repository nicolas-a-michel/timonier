// Builds the extension: bundles src/extension.ts with esbuild, then copies the webview build
// and the licence into the package. Run `npm run build -w webview` first.
import { cpSync, existsSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { build } from "esbuild";

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = path.resolve(packageRoot, "..", "..");
const webviewDist = path.join(repoRoot, "webview", "dist");
const production = process.argv.includes("--production");

if (!existsSync(path.join(webviewDist, ".vite", "manifest.json"))) {
  console.error("The webview is not built: run `npm run build -w webview` first.");
  process.exit(1);
}

rmSync(path.join(packageRoot, "dist"), { recursive: true, force: true });
await build({
  entryPoints: [path.join(packageRoot, "src", "extension.ts")],
  outfile: path.join(packageRoot, "dist", "extension.js"),
  bundle: true,
  platform: "node",
  format: "cjs",
  target: "node24",
  external: ["vscode"],
  sourcemap: !production,
  minify: production,
  logLevel: "warning",
});
cpSync(webviewDist, path.join(packageRoot, "dist", "webview"), { recursive: true });
cpSync(path.join(repoRoot, "LICENSE"), path.join(packageRoot, "LICENSE"));
