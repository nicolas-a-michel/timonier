// Builds the extension: bundles src/extension.ts with esbuild, then copies the webview build
// and the licence into the package. Run `npm run build -w webview` first.
import { cpSync, existsSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
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
const result = await build({
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
  metafile: true,
  // The metafile's paths are relative to it.
  absWorkingDir: packageRoot,
});
writeNotices(result.metafile, path.join(packageRoot, "dist", "THIRD-PARTY-NOTICES.md"));
cpSync(webviewDist, path.join(packageRoot, "dist", "webview"), { recursive: true });
withSources(path.join(packageRoot, "dist", "webview", "THIRD-PARTY-NOTICES.md"));
withSources(path.join(packageRoot, "dist", "webview", "tracking", "THIRD-PARTY-NOTICES.md"));
withSources(path.join(packageRoot, "dist", "webview", "whiteboard", "THIRD-PARTY-NOTICES.md"));
withSources(path.join(packageRoot, "dist", "webview", "performance", "THIRD-PARTY-NOTICES.md"));
cpSync(path.join(repoRoot, "LICENSE"), path.join(packageRoot, "LICENSE"));

/** The licences of the packages bundled into extension.js, as the webview's build does for its own. */
function writeNotices(metafile, file) {
  const packages = new Map();
  for (const input of Object.keys(metafile.inputs)) {
    const match = /(?:^|\/)node_modules\/((?:@[^/]+\/)?[^/]+)\//.exec(input);
    if (!match) continue;
    const dir = path.resolve(packageRoot, input.slice(0, match.index + match[0].length));
    packages.set(match[1], dir);
  }
  const sections = [...packages.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([name, dir]) => {
      const manifest = JSON.parse(readFileSync(path.join(dir, "package.json"), "utf8"));
      const licence = readdirSync(dir).find((f) => /^licen[cs]e/i.test(f));
      if (!licence) throw new Error(`${name} ships no licence file: check it before bundling it`);
      const text = readFileSync(path.join(dir, licence), "utf8").trim();
      return `## ${name} ${manifest.version}\n\nLicence: ${manifest.license}\n\n${text}\n`;
    });
  const header = "# Third-party notices of the Timonier extension\n\nPackages bundled into `dist/extension.js`.\n";
  writeFileSync(file, [header, ...sections].join("\n"));
}

/**
 * Where to obtain the source of the bundled packages under a copyleft licence: EPL-2.0 (§ 3.2)
 * and the MPL ask for it with the object code. Read from each package's manifest.
 */
function withSources(file) {
  const notices = readFileSync(file, "utf8");
  const heading = /^## (\S+) - (\S+) \(([^)]*)\)$/gm;
  const lines = [];
  for (const [, name, version, licence] of notices.matchAll(heading)) {
    if (!/EPL|MPL|GPL/.test(licence)) continue;
    const manifest = JSON.parse(readFileSync(path.join(repoRoot, "node_modules", name, "package.json"), "utf8"));
    const repository = typeof manifest.repository === "string" ? manifest.repository : manifest.repository?.url;
    if (!repository) throw new Error(`${name} names no repository: its source must be findable (${licence})`);
    const url = repository.replace(/^git\+/, "").replace(/\.git$/, "");
    // The exact version, as npm distributes it: the repository alone does not say which.
    const tarball = `https://registry.npmjs.org/${name}/-/${name.split("/").at(-1)}-${version}.tgz`;
    lines.push(`- ${name} ${version} (${licence}): ${tarball}, from ${url}`);
  }
  if (lines.length === 0) return;
  writeFileSync(
    file,
    `${notices.trimEnd()}\n\n## Source code\n\nThe source of these packages, distributed unmodified:\n\n${lines.join("\n")}\n`,
  );
}
