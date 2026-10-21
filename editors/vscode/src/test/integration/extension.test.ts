import * as assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";

import * as vscode from "vscode";

import type { TimonierApi } from "../../extension";

const EXTENSION_ID = "NicolasMichel.timonier";

async function api(): Promise<TimonierApi> {
  const extension = vscode.extensions.getExtension<TimonierApi>(EXTENSION_ID);
  assert.ok(extension, `${EXTENSION_ID} is not installed`);
  return extension.activate();
}

function isMapTab(tab: vscode.Tab | undefined): boolean {
  return tab?.input instanceof vscode.TabInputWebview && tab.input.viewType.endsWith("timonier.map");
}

function mapTabs(): vscode.Tab[] {
  return vscode.window.tabGroups.all.flatMap((group) => group.tabs).filter(isMapTab);
}

function whiteboardTabs(): vscode.Tab[] {
  return vscode.window.tabGroups.all
    .flatMap((group) => group.tabs)
    .filter((tab) => tab.input instanceof vscode.TabInputWebview && tab.input.viewType.endsWith("timonier.whiteboard"));
}

function performanceTabs(): vscode.Tab[] {
  return vscode.window.tabGroups.all
    .flatMap((group) => group.tabs)
    .filter(
      (tab) => tab.input instanceof vscode.TabInputWebview && tab.input.viewType.endsWith("timonier.performance"),
    );
}

/** Rejects when `promise` takes longer than `ms`. */
function inTime<T>(promise: Promise<T>, what: string, ms = 10_000): Promise<T> {
  return Promise.race([promise, new Promise<T>((_, reject) => setTimeout(() => reject(new Error(what)), ms))]);
}

async function waitFor(condition: () => boolean, timeoutMs = 5000): Promise<void> {
  const start = Date.now();
  while (!condition()) {
    if (Date.now() - start > timeoutMs) throw new Error("condition not met in time");
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
}

describe("Timonier extension", () => {
  it("is installed and activates", async () => {
    const extension = vscode.extensions.getExtension(EXTENSION_ID);
    assert.ok(extension, `${EXTENSION_ID} is not installed`);
    await extension.activate();
    assert.equal(extension.isActive, true);
  });

  it("adds the Timonier container to the activity bar, with its overview view", async () => {
    const commands = await vscode.commands.getCommands(true);
    // VSCode generates these commands for each contributed container and view.
    assert.ok(commands.includes("workbench.view.extension.timonier"), "no Timonier view container");
    assert.ok(commands.includes("timonier.home.focus"), "no overview view");
    await vscode.commands.executeCommand("workbench.view.extension.timonier");
    await vscode.commands.executeCommand("timonier.home.focus");
  });

  it("contributes the Open Map command", async () => {
    const commands = await vscode.commands.getCommands(true);
    assert.ok(commands.includes("timonier.openMap"));
  });

  it("opens the map, whose bundle runs under the strict CSP", async () => {
    const timonier = await api();
    await vscode.commands.executeCommand("timonier.openMap");
    await waitFor(() => mapTabs().length === 1);
    const ready = timonier.whenMapReady();
    assert.ok(ready, "no map panel is tracked");
    // The webview only says "ready" if its script ran: a CSP violation would leave this pending.
    await Promise.race([
      ready,
      new Promise((_, reject) => setTimeout(() => reject(new Error("the webview never said ready")), 10_000)),
    ]);
  });

  it("reveals the open map instead of opening a second one", async () => {
    const document = await vscode.workspace.openTextDocument({ content: "" });
    await vscode.window.showTextDocument(document);
    await waitFor(() => !isMapTab(vscode.window.tabGroups.activeTabGroup.activeTab));
    await vscode.commands.executeCommand("timonier.openMap");
    await waitFor(() => isMapTab(vscode.window.tabGroups.activeTabGroup.activeTab));
    assert.equal(mapTabs().length, 1);
  });

  it("opens the Performances panel under the strict CSP, and the map's layer asks the daemon (ADR 0051)", async () => {
    const timonier = await api();
    await vscode.commands.executeCommand("timonier.openPerformance");
    await waitFor(() => performanceTabs().length === 1);
    const ready = timonier.whenPerformanceReady();
    assert.ok(ready, "no Performances panel is tracked");
    await inTime(ready, "the Performances panel never said ready");
    await vscode.commands.executeCommand("timonier.openPerformance");
    assert.equal(performanceTabs().length, 1, "revealed, not opened twice");
    // The map's layer: no run yet in the fixture, so a layer of no run, from the daemon itself.
    await vscode.commands.executeCommand("timonier.openMap");
    await waitFor(() => mapTabs().length >= 1);
    assert.equal(
      await inTime(Promise.resolve(timonier.mapCommand({ name: "togglePerformance" })), "no map command"),
      true,
    );
    await waitFor(() => timonier.mapPerformance() !== undefined, 20_000);
    assert.deepEqual(timonier.mapPerformance(), { run: null, nodes: [], links: [], ancestors: {} });
    await timonier.mapCommand({ name: "togglePerformance" });
  });

  it("shows a run's time and an exceeded budget on the map, from a real run (ADR 0051, 0052)", async () => {
    const timonier = await api();
    const project = process.env.TIMONIER_FIXTURE;
    const daemon = process.env.TIMONIER_DAEMON_PATH;
    assert.ok(project && daemon, "no fixture project or daemon");
    // A budget the human would draw: the fixture's package, under a microsecond at p95.
    const intent = path.join(project, ".timonier", "intent.json");
    const drawn = existsSync(intent) ? readFileSync(intent, "utf8") : null;
    mkdirSync(path.dirname(intent), { recursive: true });
    writeFileSync(
      intent,
      JSON.stringify({
        version: 1,
        components: [{ id: "C-1", name: "fixture", layer: null, role: "the fixture" }],
        budgets: [{ id: "N-1", attached_to: "C-1", p95_ms: 0.001 }],
      }),
    );
    // A test process that sends one span of `run` over OTLP/HTTP JSON, as an SDK would.
    const emit = path.join(project, "emit.mjs");
    writeFileSync(
      emit,
      `const run = (process.env.OTEL_RESOURCE_ATTRIBUTES ?? "").match(/timonier\\.run_id=([^,]+)/)?.[1];
const now = BigInt(Date.now()) * 1000000n;
const span = {
  traceId: "0af7651916cd43dd8448eb211c80319c", spanId: "b7ad6b7169203331", name: "run",
  startTimeUnixNano: String(now - 5000000n), endTimeUnixNano: String(now),
  attributes: [
    { key: "code.function.name", value: { stringValue: "run" } },
    { key: "code.file.path", value: { stringValue: "src/main.ts" } },
    { key: "code.line.number", value: { intValue: "3" } },
  ],
};
const body = { resourceSpans: [{
  resource: { attributes: [{ key: "timonier.run_id", value: { stringValue: run } }] },
  scopeSpans: [{ spans: [span] }],
}] };
const answer = await fetch(process.env.OTEL_EXPORTER_OTLP_ENDPOINT + "/v1/traces", {
  method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body),
});
process.exit(answer.ok ? 0 : 1);
`,
    );
    try {
      await vscode.commands.executeCommand("timonier.openMap");
      await waitFor(() => mapTabs().length >= 1);
      const ran = spawnSync(daemon, ["test", "--root", project, "--", process.execPath, emit], {
        encoding: "utf8",
        env: { ...process.env, ELECTRON_RUN_AS_NODE: "1" },
      });
      assert.equal(ran.status, 0, `${ran.stdout}\n${ran.stderr}`);
      // The run ended: the map's budgets follow, its box marked.
      await waitFor(() => timonier.mapBudgets()?.checks.some((c) => c.state === "exceeded") === true, 20_000).catch(
        (error: unknown) => {
          throw new Error(`${String(error)}: ${JSON.stringify(timonier.mapBudgets())}\n${ran.stderr}`);
        },
      );
      const check = timonier.mapBudgets()?.checks.find((c) => c.id === "N-1");
      assert.deepEqual(check?.boxes, ["pkg:npm/fixture"]);
      // The layer, turned on, gives the run's time to the function's node.
      assert.equal(await timonier.mapCommand({ name: "togglePerformance" }), true);
      await waitFor(() => (timonier.mapPerformance()?.nodes.length ?? 0) > 0, 20_000);
      const node = timonier.mapPerformance()?.nodes.find((n) => n.node === "typescript:fixture/src/main#run");
      assert.ok(node && node.selfNs > 0 && node.enteredNs > 0, JSON.stringify(timonier.mapPerformance()));
      await timonier.mapCommand({ name: "togglePerformance" });
    } finally {
      if (drawn === null) rmSync(intent, { force: true });
      else writeFileSync(intent, drawn);
      rmSync(emit, { force: true });
    }
  });

  it("opens the whiteboard, whose production bundle runs under the strict CSP (ADR 0036)", async () => {
    const timonier = await api();
    assert.equal(await timonier.whiteboardCommand({ name: "undo" }), false, "a command without a whiteboard");
    await vscode.commands.executeCommand("timonier.openWhiteboard");
    await waitFor(() => whiteboardTabs().length === 1);
    const ready = timonier.whenWhiteboardReady();
    assert.ok(ready, "no whiteboard panel is tracked");
    await inTime(ready, "the whiteboard never said ready");
    // The palette's commands reach its page.
    assert.equal(await inTime(timonier.whiteboardCommand({ name: "tool", tool: "eraser" }), "no command"), true);
    await vscode.commands.executeCommand("timonier.whiteboard.addBox");
    // The box reaches `.timonier/intent.json` on its own, through the daemon (Q66, ADR 0039).
    const project = process.env.TIMONIER_FIXTURE;
    assert.ok(project, "no fixture project");
    const file = path.join(project, ".timonier", "intent.json");
    const read = () => (existsSync(file) ? readFileSync(file, "utf8") : "");
    await waitFor(() => read().includes('"id": "C-1"'), 20_000);
    assert.match(read(), /^\{\n {2}"version": 1,\n/);
    await vscode.commands.executeCommand("timonier.whiteboard.undo");
    await waitFor(() => !read().includes('"id": "C-1"'), 20_000);
    await vscode.commands.executeCommand("timonier.openWhiteboard");
    assert.equal(whiteboardTabs().length, 1);
  });
});
