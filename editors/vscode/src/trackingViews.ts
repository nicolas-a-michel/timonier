import { readFileSync } from "node:fs";

import * as vscode from "vscode";

import { isWebviewMessage } from "./messages";
import type { TrackingRelay } from "./trackingRelay";
import { webviewEntry } from "./viteManifest";
import { createNonce, renderWebviewHtml } from "./webviewHtml";

/** The view of the secondary side bar that shows the tracking (ADR 0031). */
export const TRACKING_VIEW = "timonier.tracking";

/** The home view, the bridge, at the top of the Timonier side bar (ADR 0032, Q47). */
export const BRIDGE_VIEW = "timonier.home";

/** What a page shows: the plan and the todo beside the sessions, or the bridge (ADR 0032). */
export type Panel = "all" | "bridge";

/** What the bridge's page asks of the extension: the map on a box, the map, a review mark. */
export interface BridgeHost {
  reveal(target: string, path: string[]): unknown;
  openMap(): unknown;
  markReviewed(): unknown;
}

/** The tracking: a webview view of the secondary side bar, fed by the relay. */
class TrackingViewProvider implements vscode.WebviewViewProvider {
  constructor(
    private readonly extensionUri: vscode.Uri,
    private readonly panel: Panel,
    private readonly title: string,
    private readonly relay: TrackingRelay,
    private readonly log: vscode.LogOutputChannel,
    private readonly host?: BridgeHost,
  ) {}

  resolveWebviewView(view: vscode.WebviewView): void {
    const webview = view.webview;
    // Its own build, apart from the map's (webview/vite.config.ts).
    const webviewRoot = vscode.Uri.joinPath(this.extensionUri, "dist", "webview", "tracking");
    webview.options = { enableScripts: true, localResourceRoots: [webviewRoot] };
    let entry;
    try {
      entry = webviewEntry(
        readFileSync(vscode.Uri.joinPath(webviewRoot, ".vite", "manifest.json").fsPath, "utf8"),
        "tracking.html",
      );
    } catch (error) {
      this.log.error(`the ${this.panel} panel cannot load: ${String(error)}`);
      return;
    }
    // Each page that loads gets the state again: a hidden view loses its page.
    let page: { dispose(): void } | undefined;
    const messages = webview.onDidReceiveMessage((message: unknown) => {
      if (!isWebviewMessage(message)) return;
      if (message.type === "ready") {
        page?.dispose();
        page = this.relay.attach((m) => webview.postMessage(m));
        return;
      }
      switch (message.type) {
        case "reveal":
          this.host?.reveal(message.target, message.path);
          return;
        case "openMap":
          this.host?.openMap();
          return;
        case "markReviewed":
          this.host?.markReviewed();
          return;
        default:
          this.relay.receive(message);
      }
    });
    const asUri = (file: string) => webview.asWebviewUri(vscode.Uri.joinPath(webviewRoot, file)).toString();
    webview.html = renderWebviewHtml({
      cspSource: webview.cspSource,
      nonce: createNonce(),
      scriptUri: asUri(entry.script),
      styleUris: entry.styles.map(asUri),
      lang: vscode.env.language,
      title: this.title,
      panel: this.panel,
    });
    const visibility = view.onDidChangeVisibility(() => {
      if (!view.visible) {
        page?.dispose();
        page = undefined;
      }
    });
    view.onDidDispose(() => {
      page?.dispose();
      visibility.dispose();
      messages.dispose();
    });
  }
}

/** Registers the view of the tracking, and the bridge in place of the overview (ADR 0032). */
export function registerTrackingViews(
  extensionUri: vscode.Uri,
  relay: TrackingRelay,
  log: vscode.LogOutputChannel,
  host: BridgeHost,
): vscode.Disposable[] {
  return [
    vscode.window.registerWebviewViewProvider(
      TRACKING_VIEW,
      new TrackingViewProvider(extensionUri, "all", vscode.l10n.t("Plan, todo and sessions"), relay, log),
    ),
    vscode.window.registerWebviewViewProvider(
      BRIDGE_VIEW,
      new TrackingViewProvider(extensionUri, "bridge", vscode.l10n.t("Overview"), relay, log, host),
    ),
  ];
}
