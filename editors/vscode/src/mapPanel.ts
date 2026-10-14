import { readFileSync } from "node:fs";

import * as vscode from "vscode";

import { webviewEntry, type WebviewEntry } from "./viteManifest";
import { createNonce, renderWebviewHtml } from "./webviewHtml";

export const MAP_VIEW_TYPE = "timonier.map";

/** The webview's handshake (webview/src/protocol.ts); the shared protocol comes in Phase 4. */
function isReadyMessage(message: unknown): boolean {
  return typeof message === "object" && message !== null && (message as { type?: unknown }).type === "ready";
}

/** The architecture map panel: one per window, revealed when it is already open. */
export class MapPanel {
  private static current: MapPanel | undefined;

  /** Opens the map, or reveals it. Throws if the webview build is missing from the extension. */
  static show(extensionUri: vscode.Uri): void {
    if (MapPanel.current) {
      MapPanel.current.panel.reveal();
      return;
    }
    const webviewRoot = vscode.Uri.joinPath(extensionUri, "dist", "webview");
    // Read the manifest first, so that a broken build fails before any panel appears.
    const entry = webviewEntry(readFileSync(vscode.Uri.joinPath(webviewRoot, ".vite", "manifest.json").fsPath, "utf8"));
    const panel = vscode.window.createWebviewPanel(
      MAP_VIEW_TYPE,
      vscode.l10n.t("Timonier map"),
      vscode.ViewColumn.Active,
      {
        enableScripts: true,
        localResourceRoots: [webviewRoot],
      },
    );
    MapPanel.current = new MapPanel(panel, webviewRoot, entry);
  }

  /** Resolves once the open map's webview has rendered; undefined when no map is open. */
  static whenReady(): Promise<void> | undefined {
    return MapPanel.current?.ready;
  }

  private readonly ready: Promise<void>;

  private constructor(
    private readonly panel: vscode.WebviewPanel,
    webviewRoot: vscode.Uri,
    entry: WebviewEntry,
  ) {
    const webview = panel.webview;
    this.ready = new Promise((resolve) => {
      const subscription = webview.onDidReceiveMessage((message: unknown) => {
        if (!isReadyMessage(message)) return;
        subscription.dispose();
        resolve();
      });
    });
    const asUri = (file: string) => webview.asWebviewUri(vscode.Uri.joinPath(webviewRoot, file)).toString();
    webview.html = renderWebviewHtml({
      cspSource: webview.cspSource,
      nonce: createNonce(),
      scriptUri: asUri(entry.script),
      styleUris: entry.styles.map(asUri),
      lang: vscode.env.language,
      title: vscode.l10n.t("Timonier map"),
    });
    panel.onDidDispose(() => {
      MapPanel.current = undefined;
    });
  }
}
