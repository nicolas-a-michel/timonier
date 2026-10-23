import { readFileSync } from "node:fs";

import * as vscode from "vscode";

import { DriftFeed } from "./driftFeed";
import { BudgetFeed, PerfMapFeed } from "./perfMapFeed";
import { webviewEntry, type WebviewEntry } from "./viteManifest";
import { densityOf, onPageMessage, onSettingsChange, type MapHostActions } from "./mapHost";
import { ViewRelay, type DaemonSource, type PinStore, type Rendered } from "./viewRelay";
import {
  METHODS,
  type Density,
  type BudgetsView,
  type DriftView,
  type MapCommand,
  type PerfMap,
  type PerfRuns,
  type Role,
  type Span,
} from "./protocol/generated";
import { createNonce, renderWebviewHtml } from "./webviewHtml";
import { askRename, ROLES } from "./naming";

export const MAP_VIEW_TYPE = "timonier.map";

/** The density the settings give the map (Q32): comfortable unless set to compact. */
export function mapDensity(): Density {
  return densityOf(vscode.workspace.getConfiguration("timonier").get<string>("map.density"));
}

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
    root?: string,
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
    MapPanel.current = new MapPanel(panel, webviewRoot, entry, daemon, log, openCode, pins, root);
  }

  /** Resolves once the open map's webview has rendered; undefined when no map is open. */
  static whenReady(): Promise<void> | undefined {
    return MapPanel.current?.ready;
  }

  /** The Performances layer the open map was last given, while it shows it (ADR 0051). */
  static performance(): PerfMap | null | undefined {
    return MapPanel.current?.perf.last;
  }

  /** The budgets the open map was last given (ADR 0052): for the tests. */
  static budgets(): BudgetsView | null | undefined {
    return MapPanel.current?.budgets.last;
  }

  /** The drift the open map was last given, while it compares with the whiteboard (ADR 0042). */
  static drift(): DriftView | null | undefined {
    return MapPanel.current?.drift.last;
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
  private readonly drift: DriftFeed;
  private readonly perf: PerfMapFeed;
  private readonly budgets: BudgetFeed;

  private constructor(
    private readonly panel: vscode.WebviewPanel,
    webviewRoot: vscode.Uri,
    entry: WebviewEntry,
    daemon: DaemonSource,
    log: vscode.LogOutputChannel,
    openCode?: (file: string, span: Span | null) => Promise<unknown>,
    pins?: PinStore,
    root?: string,
  ) {
    const webview = panel.webview;
    this.relay = new ViewRelay(daemon, (message) => webview.postMessage(message), log, openCode, pins);
    // The map compared with the whiteboard (ADR 0042): the drift, again after each change of the
    // code the map shows, and of `intent.json`.
    this.drift = new DriftFeed({
      fetch: () => {
        if (!daemon.client) return Promise.reject(new Error("the daemon is not running"));
        return daemon.client.sendRequest<DriftView>(METHODS.INTENT_DRIFT);
      },
      post: (message) => webview.postMessage(message),
    });
    // The Performances layer (ADR 0051): built again when a run with traces ends.
    this.perf = new PerfMapFeed({
      fetch: () => {
        if (!daemon.client) return Promise.reject(new Error("the daemon is not running"));
        return daemon.client.sendRequest<PerfMap>(METHODS.PERF_MAP, {});
      },
      runs: () => {
        if (!daemon.client) return Promise.reject(new Error("the daemon is not running"));
        return daemon.client.sendRequest<PerfRuns>(METHODS.PERF_RUNS);
      },
      post: (message) => webview.postMessage(message),
    });
    // The budgets of the whiteboard, always while the map shows (Q74, ADR 0052).
    this.budgets = new BudgetFeed({
      fetch: () => {
        if (!daemon.client) return Promise.reject(new Error("the daemon is not running"));
        return daemon.client.sendRequest<BudgetsView | null>(METHODS.PERF_BUDGETS);
      },
      runs: () => {
        if (!daemon.client) return Promise.reject(new Error("the daemon is not running"));
        return daemon.client.sendRequest<PerfRuns>(METHODS.PERF_RUNS);
      },
      post: (message) => webview.postMessage(message),
    });
    const rendered = this.relay.onDidRender(() => this.drift.changed());
    const whiteboard = root
      ? vscode.workspace.createFileSystemWatcher(new vscode.RelativePattern(root, ".timonier/intent.json"))
      : undefined;
    const drawn = () => {
      this.drift.changed();
      void this.budgets.changed();
    };
    whiteboard?.onDidChange(drawn);
    whiteboard?.onDidCreate(drawn);
    whiteboard?.onDidDelete(drawn);
    let announceReady: () => void = () => undefined;
    this.ready = new Promise((resolve) => (announceReady = resolve));
    const actions: MapHostActions = {
      post: (message) => webview.postMessage(message),
      density: mapDensity,
      ready: () => {
        announceReady();
        void this.budgets.start();
      },
      markReviewed: () => vscode.commands.executeCommand("timonier.markReviewed"),
      compareIntent: (on) => this.drift.compare(on),
      openWhiteboard: () => vscode.commands.executeCommand("timonier.openWhiteboard"),
      showPerformance: (on) => this.perf.show(on),
      openPerformance: () => vscode.commands.executeCommand("timonier.openPerformance"),
      rename: (box) => void rename(daemon, box, log),
      relay: (message) => this.relay.receive(message),
    };
    const messages = webview.onDidReceiveMessage((message: unknown) => onPageMessage(message, actions));
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
      if (!webviewPanel.visible) {
        this.relay.unloaded();
        // The new page says again whether it compares, and whether it shows performances.
        void this.drift.compare(false);
        void this.perf.show(false);
        this.budgets.stop();
      }
    });
    const settings = vscode.workspace.onDidChangeConfiguration((event) =>
      onSettingsChange((section) => event.affectsConfiguration(section), actions),
    );
    panel.onDidDispose(() => {
      this.drift.dispose();
      this.perf.dispose();
      this.budgets.stop();
      rendered.dispose();
      whiteboard?.dispose();
      settings.dispose();
      visibility.dispose();
      messages.dispose();
      void this.relay.dispose();
      MapPanel.current = undefined;
    });
  }
}

/** The role names of the quick pick, in the interface's language. */
function roleLabel(role: Role): string {
  switch (role) {
    case "project":
      return vscode.l10n.t("Project");
    case "entry_point":
      return vscode.l10n.t("Entry point");
    case "interface":
      return vscode.l10n.t("Interface");
    case "background_task":
      return vscode.l10n.t("Background task");
    case "data_model":
      return vscode.l10n.t("Data model");
    case "persistence":
      return vscode.l10n.t("Persistence");
    case "utility":
      return vscode.l10n.t("Utility");
    case "external_libraries":
      return vscode.l10n.t("External libraries");
  }
}

/**
 * Asks a box's name and role, then gives them to the daemon, which keeps them in
 * `.timonier/names.json` and redraws the map (ADR 0075).
 */
async function rename(
  daemon: DaemonSource,
  box: { target: string; name: string; role: Role | null },
  log: vscode.LogOutputChannel,
): Promise<void> {
  const computed = vscode.l10n.t("As Timonier computes it");
  const asked = await askRename(box, {
    name: (current) =>
      vscode.window.showInputBox({
        title: vscode.l10n.t("Rename the box"),
        prompt: vscode.l10n.t("A few words that say what it does; empty, the name Timonier computes."),
        value: current,
        validateInput: (value) => (value.trim().length > 60 ? vscode.l10n.t("At most 60 characters.") : undefined),
      }),
    role: async (current) => {
      const items: { label: string; role: Role | null; description?: string }[] = [
        { label: computed, role: null },
        ...ROLES.map((role) => ({
          label: roleLabel(role),
          role,
          description: role === current ? vscode.l10n.t("now") : undefined,
        })),
      ];
      const picked = await vscode.window.showQuickPick(items, { title: vscode.l10n.t("Its role on the map") });
      return picked ? picked.role : undefined;
    },
  });
  if (!asked) return;
  if (!daemon.client) {
    void vscode.window.showErrorMessage(vscode.l10n.t("the daemon is not running"));
    return;
  }
  try {
    await daemon.client.sendRequest(METHODS.NAMES_SET, asked);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    log.warn(`names/set: ${reason}`);
    void vscode.window.showErrorMessage(vscode.l10n.t("Timonier could not rename the box: {0}", reason));
  }
}
