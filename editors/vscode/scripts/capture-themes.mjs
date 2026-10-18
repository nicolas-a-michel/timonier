// Captures the theme variables a Timonier webview receives in VSCode's built-in themes, for the
// contrast tests of the design system (PLAN.md 4.6, ADR 0020). Run it again when VSCode changes
// its themes:
//
//   npm run build && node editors/vscode/scripts/capture-themes.mjs [theme file]
//
// It starts the VSCode of the integration tests (editors/vscode/.vscode-test) once per theme,
// sets the theme in the profile's settings once the window has settled, opens the map, and reads
// the webview's <html> style and <body> class over the DevTools protocol.
// Output: webview/test/themes/<theme file>.json.
import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const extension = path.resolve(here, "..");
const repoRoot = path.resolve(extension, "..", "..");
const out = path.join(repoRoot, "webview", "test", "themes");

// The defaults of VSCode 1.142, the previous ones, and both high-contrast themes. `name` is the
// theme's id (theme-defaults/package.json); `kind`, the class the workbench gets.
const THEMES = [
  { file: "light-2026", name: "Light 2026", kind: "vs" },
  { file: "dark-2026", name: "Dark 2026", kind: "vs-dark" },
  { file: "light-modern", name: "Light Modern", kind: "vs" },
  { file: "dark-modern", name: "Dark Modern", kind: "vs-dark" },
  { file: "high-contrast", name: "Default High Contrast", kind: "hc-black" },
  { file: "high-contrast-light", name: "Default High Contrast Light", kind: "hc-light" },
];

function vscodeBinary() {
  const root = path.join(extension, ".vscode-test");
  // The newest of those @vscode/test-electron downloaded: their names end with the version.
  const version = (d) => (d.split("-").at(-1) ?? "").split(".").map(Number);
  const newer = (a, b) => {
    const [x, y] = [version(a), version(b)];
    for (let i = 0; i < Math.max(x.length, y.length); i++) {
      if ((x[i] ?? 0) !== (y[i] ?? 0)) return (y[i] ?? 0) - (x[i] ?? 0);
    }
    return 0;
  };
  const install = readdirSync(root)
    .filter((d) => d.startsWith("vscode-"))
    .sort(newer)[0];
  if (!install) throw new Error("No VSCode in .vscode-test: run `npm run test:integration` once.");
  const dir = path.join(root, install);
  const code =
    process.platform === "darwin"
      ? path.join(dir, "Visual Studio Code.app", "Contents", "MacOS", "Electron")
      : path.join(dir, "code");
  return { code, version: install.split("-").at(-1) };
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/** A DevTools session. Nothing waits forever: a target may go away under a call. */
function connect(url) {
  const ws = new WebSocket(url);
  let id = 0;
  const waiting = new Map();
  ws.onmessage = (event) => {
    const message = JSON.parse(event.data);
    const call = waiting.get(message.id);
    if (!call) return;
    waiting.delete(message.id);
    if (message.error) call.reject(new Error(JSON.stringify(message.error)));
    else call.resolve(message.result);
  };
  const send = (method, params = {}) =>
    new Promise((resolve, reject) => {
      const call = ++id;
      const timer = setTimeout(() => {
        waiting.delete(call);
        reject(new Error(`${method}: no answer`));
      }, 15_000);
      waiting.set(call, {
        resolve: (value) => (clearTimeout(timer), resolve(value)),
        reject: (error) => (clearTimeout(timer), reject(error)),
      });
      ws.send(JSON.stringify({ id: call, method, params }));
    });
  const evaluate = async (expression) =>
    (await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true })).result.value;
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`cannot connect to ${url}`)), 10_000);
    ws.onopen = () => (clearTimeout(timer), resolve({ send, evaluate, close: () => ws.close() }));
    ws.onerror = () => (clearTimeout(timer), reject(new Error(`cannot connect to ${url}`)));
  });
}

async function targets(port) {
  return (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
}

/** The workbench page once `ready` holds in it. A new profile reloads its window: look again. */
async function workbench(port, ready, what) {
  for (let i = 0; i < 120; i++) {
    await sleep(500);
    try {
      const page = (await targets(port)).find((t) => t.type === "page" && t.url.includes("workbench"));
      if (!page) continue;
      const session = await connect(page.webSocketDebuggerUrl);
      if (await session.evaluate(ready)) return session;
      session.close();
    } catch {
      // Starting or reloading: the next try.
    }
  }
  throw new Error(`timeout: ${what}`);
}

async function key(session, name, keyCode) {
  for (const type of ["keyDown", "keyUp"]) {
    await session.send("Input.dispatchKeyEvent", { type, key: name, code: name, windowsVirtualKeyCode: keyCode });
  }
}

const enter = (session) => key(session, "Enter", 13);

/** F1, `text`, Enter: a command, or an item of the quick pick that is open. */
async function palette(session, text, { open = true } = {}) {
  if (open) await key(session, "F1", 112);
  for (let i = 0; i < 40 && !(await session.evaluate("!!document.querySelector('.quick-input-widget input')")); i++) {
    await sleep(250);
  }
  await session.send("Input.insertText", { text });
  await sleep(1000);
  await enter(session);
}

/** The variables of the webview frame that holds #root. */
async function webviewVariables(port) {
  for (let i = 0; i < 60; i++) {
    await sleep(500);
    for (const target of await targets(port)) {
      if (!target.url.startsWith("vscode-webview://") || !target.webSocketDebuggerUrl) continue;
      let frame;
      try {
        frame = await connect(target.webSocketDebuggerUrl);
        const found = await frame.evaluate(`(() => {
          const frames = [window, ...[...document.querySelectorAll('iframe')].map(f => { try { return f.contentWindow; } catch { return null; } })];
          for (const w of frames) {
            const d = w && w.document;
            if (!d || !d.getElementById('root')) continue;
            const style = d.documentElement.style;
            const variables = {};
            for (let i = 0; i < style.length; i++) {
              const name = style[i];
              if (name.startsWith('--vscode-')) variables[name] = style.getPropertyValue(name).trim();
            }
            return { bodyClass: d.body.className, variables };
          }
          return null;
        })()`);
        if (found && Object.keys(found.variables).length > 0) return found;
      } catch {
        // A frame that does not answer (a service worker, the host page): the next one.
      } finally {
        frame?.close();
      }
    }
  }
  throw new Error("no Timonier webview found");
}

async function capture(theme, code) {
  const temp = mkdtempSync(path.join(os.tmpdir(), "timonier-themes-"));
  const userData = path.join(temp, "user");
  mkdirSync(path.join(userData, "User"), { recursive: true });
  const settings = path.join(userData, "User", "settings.json");
  writeFileSync(
    settings,
    JSON.stringify({
      "workbench.startupEditor": "none",
      "window.restoreWindows": "none",
      "workbench.enableExperiments": false,
      "telemetry.telemetryLevel": "off",
      "update.mode": "none",
      "extensions.autoCheckUpdates": false,
      "chat.disableAIFeatures": true,
      "workbench.tips.enabled": false,
    }),
  );
  const workspace = path.join(temp, "project");
  mkdirSync(workspace);
  const port = 9300 + Math.floor(Math.random() * 600);
  const child = spawn(
    code,
    [
      `--extensions-dir=${path.join(temp, "extensions")}`,
      `--user-data-dir=${userData}`,
      `--extensionDevelopmentPath=${extension}`,
      `--remote-debugging-port=${port}`,
      "--new-window",
      "--disable-workspace-trust",
      "--skip-welcome",
      "--skip-release-notes",
      "--disable-gpu",
      workspace,
    ],
    {
      stdio: "ignore",
      // A terminal of VSCode runs its tools as Node: the editor must start as itself.
      env: Object.fromEntries(
        Object.entries(process.env).filter(([k]) => !k.startsWith("ELECTRON_RUN_AS_NODE") && !k.startsWith("VSCODE_")),
      ),
    },
  );
  try {
    let session = await workbench(port, "!!document.querySelector('.activitybar')", "the workbench");
    // Settled: the first launch of a profile reloads its window once.
    await sleep(8000);
    session.close();
    session = await workbench(port, "!!document.querySelector('.activitybar')", "the workbench");
    // The theme, written once the workbench has settled: VSCode applies a changed settings file
    // live. (Its picker also lists Marketplace themes, which a typed name may match first.)
    writeFileSync(
      settings,
      JSON.stringify({ ...JSON.parse(readFileSync(settings, "utf8")), "workbench.colorTheme": theme.name }),
    );
    session.close();
    const kind = JSON.stringify(theme.kind);
    const hasKind = `!!document.querySelector('.monaco-workbench')?.className.split(/\\s+/).includes(${kind})`;
    session = await workbench(port, hasKind, `the theme ${theme.name}`);
    await sleep(1000);
    await palette(session, "Timonier: Open Map");
    session.close();
    return await webviewVariables(port);
  } finally {
    child.kill();
    await sleep(1500);
    rmSync(temp, { recursive: true, force: true });
  }
}

const { code, version } = vscodeBinary();
mkdirSync(out, { recursive: true });
const only = process.argv[2];
for (const theme of THEMES.filter((t) => !only || t.file === only)) {
  const found = await capture(theme, code);
  const variables = Object.fromEntries(Object.entries(found.variables).sort(([a], [b]) => a.localeCompare(b)));
  const file = path.join(out, `${theme.file}.json`);
  const content = { vscode: version, theme: theme.name, bodyClass: found.bodyClass, variables };
  writeFileSync(file, `${JSON.stringify(content, null, 2)}\n`);
  console.log(`${file}: ${Object.keys(variables).length} variables, body "${found.bodyClass}"`);
}
