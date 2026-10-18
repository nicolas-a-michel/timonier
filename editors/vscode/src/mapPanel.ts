import { readFileSync } from "node:fs";

import * as vscode from "vscode";

import { webviewEntry, type WebviewEntry } from "./viteManifest";
import { isWebviewMessage } from "./messages";
import { ViewRelay, type DaemonSource, type PinStore, type Rendered } from "./viewRelay";
import type { MapCommand, Span } from "./protocol/generated";
import { createNonce, renderWebviewHtml } from "./webviewHtml";

export const MAP_VIEW_TYPE = "timonier.map";

/** The architecture map panel: one per window, revealed when it is already open. */
export class MapPanel {
  private static current: MapPanel | undefined;

  /** Opens the map, or reveals it. Throws if the webview build is missing from the extension. */
  static show(
    extensionUri: vscode.Uri,
    daemon: DaemonSource,
    log: vscode.LogOutputChannel,
    openCode?: (file: string, span: Span | null) => Promise<unknown>,
    pins?: PinStore,
  ): void {
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
    MapPanel.current = new MapPanel(panel, webviewRoot, entry, daemon, log, openCode, pins);
  }

  /** Resolves once the open map's webview has rendered; undefined when no map is open. */
  static whenReady(): Promise<void> | undefined {
    return MapPanel.current?.ready;
  }

  /** What the open map's webview last said it shows. */
  static rendered(): Rendered | undefined {
    return MapPanel.current?.relay.rendered;
  }

  /** A command of the palette for the open map; `false` when no map is open. */
  static command(command: MapCommand): Thenable<boolean> {
    const current = MapPanel.current;
    if (!current) return Promise.resolve(false);
    // A command acts on the map: show it, the command waits for its page if it reloads.
    if (!current.panel.visible) current.panel.reveal();
    return current.relay.command(command);
  }

  /** Each time the open map's webview has applied a snapshot or a delta. */
  static onDidRender(listener: (rendered: Rendered) => void): vscode.Disposable {
    return MapPanel.current?.relay.onDidRender(listener) ?? new vscode.Disposable(() => undefined);
  }

  private readonly ready: Promise<void>;
  private readonly relay: ViewRelay;

  private constructor(
    private readonly panel: vscode.WebviewPanel,
    webviewRoot: vscode.Uri,
    entry: WebviewEntry,
    daemon: DaemonSource,
    log: vscode.LogOutputChannel,
    openCode?: (file: string, span: Span | null) => Promise<unknown>,
    pins?: PinStore,
  ) {
    const webview = panel.webview;
    this.relay = new ViewRelay(daemon, (message) => webview.postMessage(message), log, openCode, pins);
    let announceReady: () => void = () => undefined;
    this.ready = new Promise((resolve) => (announceReady = resolve));
    const messages = webview.onDidReceiveMessage((message: unknown) => {
      if (!isWebviewMessage(message)) return;
      if (message.type === "ready") announceReady();
      void this.relay.receive(message);
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
    // Hidden, the panel loses its page (no `retainContextWhenHidden`): it reloads when shown.
    const visibility = panel.onDidChangeViewState(({ webviewPanel }) => {
      if (!webviewPanel.visible) this.relay.unloaded();
    });
    panel.onDidDispose(() => {
      visibility.dispose();
      messages.dispose();
      void this.relay.dispose();
      MapPanel.current = undefined;
    });
  }
}
