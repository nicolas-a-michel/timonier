import { readFileSync } from "node:fs";

import * as vscode from "vscode";

import { IntentKeeper } from "./intentKeeper";
import { isWebviewMessage } from "./messages";
import { METHODS, type IntentRead, type IntentWritten, type WhiteboardCommand } from "./protocol/generated";
import type { DaemonSource } from "./viewRelay";
import { webviewEntry } from "./viteManifest";
import { createNonce, renderWebviewHtml } from "./webviewHtml";

export const WHITEBOARD_VIEW_TYPE = "timonier.whiteboard";

/**
 * The whiteboard panel (ADR 0036, 0037): one per window, revealed when it is already open. The
 * board is `.timonier/intent.json`, written by the daemon shortly after each gesture (Q66,
 * ADR 0039), and given back to the page when the file changes outside it.
 */
export class WhiteboardPanel {
  private static current: WhiteboardPanel | undefined;

  /** Opens the whiteboard, or reveals it. Throws if its build is missing from the extension. */
  static show(
    extensionUri: vscode.Uri,
    daemon: DaemonSource,
    root: string | undefined,
    log: vscode.LogOutputChannel,
  ): void {
    if (WhiteboardPanel.current) {
      WhiteboardPanel.current.panel.reveal();
      return;
    }
    const webviewRoot = vscode.Uri.joinPath(extensionUri, "dist", "webview", "whiteboard");
    // Read the manifest first, so that a broken build fails before any panel appears.
    const entry = webviewEntry(
      readFileSync(vscode.Uri.joinPath(webviewRoot, ".vite", "manifest.json").fsPath, "utf8"),
      "whiteboard.html",
    );
    const panel = vscode.window.createWebviewPanel(
      WHITEBOARD_VIEW_TYPE,
      vscode.l10n.t("Timonier whiteboard"),
      vscode.ViewColumn.Active,
      { enableScripts: true, localResourceRoots: [webviewRoot] },
    );
    const webview = panel.webview;
    const asUri = (file: string) => webview.asWebviewUri(vscode.Uri.joinPath(webviewRoot, file)).toString();
    webview.html = renderWebviewHtml({
      cspSource: webview.cspSource,
      nonce: createNonce(),
      scriptUri: asUri(entry.script),
      styleUris: entry.styles.map(asUri),
      lang: vscode.env.language,
      title: vscode.l10n.t("Timonier whiteboard"),
    });
    WhiteboardPanel.current = new WhiteboardPanel(panel, daemon, root, log);
  }

  /** Resolves once the open whiteboard's page has rendered; undefined when none is open. */
  static whenReady(): Promise<void> | undefined {
    return WhiteboardPanel.current?.ready;
  }

  /** A command of the palette for the open whiteboard; `false` when none is open. */
  static async command(command: WhiteboardCommand): Promise<boolean> {
    const current = WhiteboardPanel.current;
    if (!current) return false;
    // A hidden panel has no page (no `retainContextWhenHidden`): show it, and wait for it.
    if (!current.panel.visible) current.panel.reveal();
    await current.ready;
    return current.panel.webview.postMessage({ type: "whiteboard", command });
  }

  private ready: Promise<void>;
  private announceReady: () => void = () => undefined;

  private readonly keeper: IntentKeeper;

  private constructor(
    private readonly panel: vscode.WebviewPanel,
    daemon: DaemonSource,
    root: string | undefined,
    log: vscode.LogOutputChannel,
  ) {
    this.ready = this.nextPage();
    const client = () => {
      if (!daemon.client) throw new Error(vscode.l10n.t("the daemon is not running"));
      return daemon.client;
    };
    this.keeper = new IntentKeeper({
      read: () => client().sendRequest<IntentRead>(METHODS.INTENT_READ),
      write: (intent) => client().sendRequest<IntentWritten>(METHODS.INTENT_WRITE, { intent }),
      post: (message) => panel.webview.postMessage(message),
    });
    const messages = panel.webview.onDidReceiveMessage((message: unknown) => {
      if (!isWebviewMessage(message)) {
        log.warn("whiteboard: a message the extension does not know", message);
        return;
      }
      if (message.type === "ready") {
        this.announceReady();
        void this.keeper.opened();
      } else if (message.type === "intent") {
        this.keeper.changed(message.intent);
      }
    });
    // A page that opened before the daemon gets its board once the daemon is there.
    const connected = "onDidConnect" in daemon ? daemon.onDidConnect(() => void this.keeper.connected()) : undefined;
    // The file changed outside the whiteboard: a `git checkout`, a hand edit.
    const watcher = root
      ? vscode.workspace.createFileSystemWatcher(new vscode.RelativePattern(root, ".timonier/intent.json"))
      : undefined;
    const changed = () => void this.keeper.fileChanged();
    watcher?.onDidChange(changed);
    watcher?.onDidCreate(changed);
    watcher?.onDidDelete(changed);
    // Hidden, the panel loses its page; shown again, the commands wait for the new one.
    const visibility = panel.onDidChangeViewState(({ webviewPanel }) => {
      if (!webviewPanel.visible) this.ready = this.nextPage();
    });
    panel.onDidDispose(() => {
      // What was drawn last is written before the panel goes.
      void this.keeper.flush().finally(() => this.keeper.dispose());
      watcher?.dispose();
      connected?.dispose();
      visibility.dispose();
      messages.dispose();
      WhiteboardPanel.current = undefined;
    });
  }

  private nextPage(): Promise<void> {
    return new Promise((resolve) => (this.announceReady = resolve));
  }
}
