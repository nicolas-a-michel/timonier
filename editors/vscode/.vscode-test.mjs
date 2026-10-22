import { existsSync, mkdirSync, mkdtempSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { defineConfig } from "@vscode/test-cli";

const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(here, "..", "..");

// The daemon the extension launches, built by `cargo build -p timonier-cli`
// (the root `npm run test:integration` does it first).
const daemon =
  process.env.TIMONIER_DAEMON_PATH ??
  path.join(repoRoot, "target", "debug", process.platform === "win32" ? "timonier.exe" : "timonier");
if (!existsSync(daemon)) {
  throw new Error(`No daemon at ${daemon}: run \`cargo build -p timonier-cli\` first.`);
}

// Projects left by a run cut short, or by a listing alone (an « Extension Tests » session
// stopped early, a test explorer looking for tests): gone after a day (review n° 11, m11-3).
const DAY_MS = 24 * 60 * 60 * 1000;
for (const name of readdirSync(os.tmpdir())) {
  if (!name.startsWith("timonier-it-")) continue;
  const old = path.join(os.tmpdir(), name);
  try {
    if (Date.now() - statSync(old).mtimeMs > DAY_MS) rmSync(old, { recursive: true, force: true });
  } catch {
    // Gone meanwhile, with another run.
  }
}

// A small TypeScript project for the daemon to index, whose call hierarchy VSCode's own
// TypeScript extension answers (PLAN.md 4.5), and a runtime directory of its own: the tests
// never touch the developer's daemons.
const temp = mkdtempSync(path.join(os.tmpdir(), "timonier-it-"));
// Gone with the run: the project, and the daemon's runtime directory (review n° 8, m8-6). The
// tests remove it themselves at the end (`cleanup.test.ts`), however VSCode was launched; this
// is for a VSCode that stops before. Not when this file is only listed (`--list-configuration`,
// what « Extension Tests » of launch.json does before it opens VSCode on the project): the
// project must outlive the listing (review n° 9, I9-1).
if (!process.argv.includes("--list-configuration")) {
  process.on("exit", () => {
    try {
      rmSync(temp, { recursive: true, force: true });
    } catch {
      // Windows keeps the files a process holds open (EPERM): the next run removes them (T-125).
    }
  });
}
const project = path.join(temp, "project");
mkdirSync(path.join(project, "src"), { recursive: true });
writeFileSync(path.join(project, "package.json"), '{ "name": "fixture", "version": "0.0.0" }\n');
writeFileSync(
  path.join(project, "src", "main.ts"),
  'import { helper } from "./util";\n\nexport function run(): void {\n  helper();\n}\n',
);
writeFileSync(path.join(project, "src", "util.ts"), "export function helper(): void {}\n");
// A type used by another file, without a call: its definition comes from VSCode (ADR 0022).
writeFileSync(path.join(project, "src", "shape.ts"), "export interface Shape {\n  size: number;\n}\n");
writeFileSync(
  path.join(project, "src", "area.ts"),
  'import type { Shape } from "./shape";\n\nexport function area(shape: Shape): number {\n  return shape.size;\n}\n',
);
// A dependency installed under the root, which the daemon does not index (git ignores it):
// VSCode answers calls into it (review n° 6, I6-1).
writeFileSync(path.join(project, ".gitignore"), "node_modules/\n");
mkdirSync(path.join(project, "node_modules", "lib"), { recursive: true });
writeFileSync(
  path.join(project, "node_modules", "lib", "package.json"),
  '{ "name": "lib", "version": "1.0.0", "types": "index.d.ts" }\n',
);
writeFileSync(path.join(project, "node_modules", "lib", "index.d.ts"), "export declare function shout(): void;\n");
writeFileSync(
  path.join(project, "src", "usePkg.ts"),
  'import { shout } from "lib";\nimport { helper } from "./util";\n\nexport function usePkg(): void {\n  shout();\n  helper();\n}\n',
);
// A note, which no language server reads (review n° 6, m6-2).
writeFileSync(path.join(project, "NOTES.md"), "# Notes\n");
// A file in Latin-1, which VSCode reads so (review n° 7, E8): the setting is for TSX files
// alone, the language of this one.
mkdirSync(path.join(project, ".vscode"));
writeFileSync(
  path.join(project, ".vscode", "settings.json"),
  '{ "[typescriptreact]": { "files.encoding": "iso88591" } }\n',
);
// A name after accents, in UTF-8: 20 « é » are 20 columns for VSCode, 40 bytes for the daemon
// (review n° 8, X6).
writeFileSync(
  path.join(project, "src", "wide.ts"),
  `import { helper } from "./util";\n/* ${"é".repeat(20)} */ export function wide(): void { helper(); }\n`,
);
writeFileSync(
  path.join(project, "src", "callsWide.ts"),
  'import { wide } from "./wide";\n\nexport function callsWide(): void {\n  wide();\n}\n',
);
writeFileSync(
  path.join(project, "src", "latin.tsx"),
  Buffer.concat([
    Buffer.from("// caf"),
    Buffer.from([0xe9]),
    Buffer.from('\nimport { helper } from "./util";\n\nexport function latin(): void {\n  helper();\n}\n'),
  ]),
);
// A Rust crate, which no language server of the test's VSCode reads (review n° 7, E11, E16).
mkdirSync(path.join(project, "tools", "src"), { recursive: true });
writeFileSync(path.join(project, "tools", "Cargo.toml"), '[package]\nname = "tools"\n');
writeFileSync(
  path.join(project, "tools", "src", "lib.rs"),
  "pub mod shapes;\npub mod wrap;\n\npub fn top() {\n    shapes::unit();\n}\n",
);
writeFileSync(path.join(project, "tools", "src", "shapes.rs"), "pub struct Unit;\n\npub fn unit() {}\n");
writeFileSync(path.join(project, "tools", "src", "wrap.rs"), "pub struct Wrap(pub Unit);\n");

export default defineConfig({
  files: "out/test/integration/**/*.test.js",
  workspaceFolder: project,
  // The project is new at each run, so never trusted: in restricted mode, VSCode keeps its
  // TypeScript features off. `npm run test:integration` adds this flag itself; « Extension
  // Tests » (F5) passes only these arguments (review n° 10, I10-1).
  launchArgs: ["--disable-workspace-trust"],
  env: {
    TIMONIER_DAEMON_PATH: daemon,
    TIMONIER_RUNTIME_DIR: path.join(temp, "run"),
    TIMONIER_FIXTURE: project,
  },
  mocha: { ui: "bdd", timeout: 30000 },
});
