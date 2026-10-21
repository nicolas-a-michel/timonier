import { readFileSync } from "node:fs";

import * as vscode from "vscode";

import { isWebviewMessage } from "./messages";
import { METHODS, type HostMessage, type Workflow, type WorkflowView } from "./protocol/generated";
import type { DaemonSource } from "./viewRelay";
import { webviewEntry } from "./viteManifest";
import { createNonce, renderWebviewHtml } from "./webviewHtml";

export const WORKFLOW_VIEW_TYPE = "timonier.workflow";

/**
 * The development process's settings (ADR 0056): a page of the tracking's build, in an editor
 * tab. It asks the daemon for `.timonier/workflow.json`, and sends each setting back; the
 * daemon checks it and writes the file.
 */
export class WorkflowPanel {
  private static current: WorkflowPanel | undefined;

  static show(extensionUri: vscode.Uri, daemon: DaemonSource, log: vscode.LogOutputChannel): void {
    if (WorkflowPanel.current) {
      WorkflowPanel.current.panel.reveal();
      return;
    }
    const webviewRoot = vscode.Uri.joinPath(extensionUri, "dist", "webview", "tracking");
    const entry = webviewEntry(
      readFileSync(vscode.Uri.joinPath(webviewRoot, ".vite", "manifest.json").fsPath, "utf8"),
      "tracking.html",
    );
    const title = vscode.l10n.t("Development process");
    const panel = vscode.window.createWebviewPanel(WORKFLOW_VIEW_TYPE, title, vscode.ViewColumn.Active, {
      enableScripts: true,
      localResourceRoots: [webviewRoot],
    });
    const webview = panel.webview;
    const asUri = (file: string) => webview.asWebviewUri(vscode.Uri.joinPath(webviewRoot, file)).toString();
    webview.html = renderWebviewHtml({
      cspSource: webview.cspSource,
      nonce: createNonce(),
      scriptUri: asUri(entry.script),
      styleUris: entry.styles.map(asUri),
      lang: vscode.env.language,
      title,
      panel: "workflow",
    });
    WorkflowPanel.current = new WorkflowPanel(panel, daemon, log);
  }

  /** What the page was last given: for the tests. */
  static last(): HostMessage | undefined {
    return WorkflowPanel.current?.sent;
  }

  private sent: HostMessage | undefined;

  private constructor(
    private readonly panel: vscode.WebviewPanel,
    private readonly daemon: DaemonSource,
    private readonly log: vscode.LogOutputChannel,
  ) {
    const messages = panel.webview.onDidReceiveMessage((message: unknown) => {
      if (!isWebviewMessage(message)) return;
      if (message.type === "askWorkflow") void this.send();
      if (message.type === "setWorkflow") void this.set(message.workflow);
    });
    panel.onDidDispose(() => {
      messages.dispose();
      WorkflowPanel.current = undefined;
    });
  }

  private post(message: HostMessage): void {
    this.sent = message;
    void this.panel.webview.postMessage(message);
  }

  /** The process as the daemon reads it; `problem` says a refusal, kept beside it. */
  private async send(problem: string | null = null): Promise<void> {
    const client = this.daemon.client;
    if (!client) {
      this.post({ type: "workflow", view: null, problem: vscode.l10n.t("the daemon is not running") });
      return;
    }
    try {
      const view = await client.sendRequest<WorkflowView>(METHODS.WORKFLOW_GET);
      this.post({ type: "workflow", view, problem });
    } catch (error) {
      this.post({ type: "workflow", view: null, problem: String(error) });
    }
  }

  private async set(workflow: Workflow): Promise<void> {
    const client = this.daemon.client;
    if (!client) return this.send();
    try {
      await client.sendRequest(METHODS.WORKFLOW_SET, workflow);
      this.log.info(`development process set: ${workflow.preset}`);
      await this.send();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      await this.send(message);
    }
  }
}
