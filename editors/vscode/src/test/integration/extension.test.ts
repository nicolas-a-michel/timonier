import * as assert from "node:assert/strict";

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
});
