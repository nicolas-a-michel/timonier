import { readFileSync } from "node:fs";

import * as vscode from "vscode";

import { isWebviewMessage } from "./messages";
import { PerformanceFeed, POLL_MS } from "./performanceFeed";
import { METHODS, type PerfRun, type PerfRuns, type RegressionReport } from "./protocol/generated";
import type { DaemonSource } from "./viewRelay";
import { webviewEntry } from "./viteManifest";
import { createNonce, renderWebviewHtml } from "./webviewHtml";

export const PERFORMANCE_VIEW_TYPE = "timonier.performance";

/**
 * The Performances panel (PLAN.md 8.7, ADR 0051): one per window, revealed when it is already
 * open. The runs come from the daemon, asked again every few seconds while the panel shows.
 */
export class PerformancePanel {
  private static current: PerformancePanel | undefined;

  /** Opens the panel, or reveals it. Throws if its build is missing from the extension. */
  static show(extensionUri: vscode.Uri, daemon: DaemonSource, log: vscode.LogOutputChannel): void {
    if (PerformancePanel.current) {
      PerformancePanel.current.panel.reveal();
      return;
    }
    const webviewRoot = vscode.Uri.joinPath(extensionUri, "dist", "webview", "performance");
    const entry = webviewEntry(
      readFileSync(vscode.Uri.joinPath(webviewRoot, ".vite", "manifest.json").fsPath, "utf8"),
      "performance.html",
    );
    const panel = vscode.window.createWebviewPanel(
      PERFORMANCE_VIEW_TYPE,
      vscode.l10n.t("Timonier performances"),
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
      title: vscode.l10n.t("Timonier performances"),
    });
    PerformancePanel.current = new PerformancePanel(panel, daemon, log);
  }

  /** Resolves once the open panel's page has rendered; undefined when none is open. */
  static whenReady(): Promise<void> | undefined {
    return PerformancePanel.current?.ready;
  }

  private ready: Promise<void>;
  private announceReady: () => void = () => undefined;
  private timer: ReturnType<typeof setInterval> | undefined;

  private constructor(
    private readonly panel: vscode.WebviewPanel,
    daemon: DaemonSource,
    log: vscode.LogOutputChannel,
  ) {
    this.ready = this.nextPage();
    const client = () => {
      if (!daemon.client) throw new Error(vscode.l10n.t("the daemon is not running"));
      return daemon.client;
    };
    const feed = new PerformanceFeed({
      runs: () => client().sendRequest<PerfRuns>(METHODS.PERF_RUNS),
      run: (run) => client().sendRequest<PerfRun | null>(METHODS.PERF_RUN, { run }),
      regressions: () => client().sendRequest<RegressionReport>(METHODS.PERF_REGRESSIONS, {}),
      post: (message) => panel.webview.postMessage(message),
    });
    const polling = (on: boolean) => {
      if (this.timer !== undefined) clearInterval(this.timer);
      this.timer = on ? setInterval(() => void feed.poll(), POLL_MS) : undefined;
    };
    const messages = panel.webview.onDidReceiveMessage((message: unknown) => {
      if (!isWebviewMessage(message)) {
        log.warn("performance: a message the extension does not know", message);
        return;
      }
      if (message.type === "ready") {
        this.announceReady();
        void feed.opened();
        polling(true);
      } else if (message.type === "chooseRun") {
        void feed.choose(message.run);
      }
    });
    const connected = "onDidConnect" in daemon ? daemon.onDidConnect(() => void feed.opened()) : undefined;
    // Hidden, the panel loses its page and asks nothing; shown again, its new page says ready.
    const visibility = panel.onDidChangeViewState(({ webviewPanel }) => {
      if (!webviewPanel.visible) {
        polling(false);
        this.ready = this.nextPage();
      }
    });
    panel.onDidDispose(() => {
      polling(false);
      connected?.dispose();
      visibility.dispose();
      messages.dispose();
      PerformancePanel.current = undefined;
    });
  }

  private nextPage(): Promise<void> {
    return new Promise((resolve) => (this.announceReady = resolve));
  }
}
