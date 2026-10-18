import * as vscode from "vscode";

import { revealCode } from "./code";
import { DaemonSupervisor, type DaemonStatus } from "./daemon";
import { daemonProgram } from "./launch";
import { MapPanel } from "./mapPanel";
import type { Direction, MapCommand, Pin } from "./protocol/generated";
import type { DaemonSource, Rendered } from "./viewRelay";

/** What `activate` exports. Only the integration tests use it for now. */
export interface TimonierApi {
  /** Resolves once the open map's webview has rendered; undefined when no map is open. */
  whenMapReady(): Promise<void> | undefined;
  /** What the open map's webview last said it shows. */
  mapRendered(): Rendered | undefined;
  onMapRender(listener: (rendered: Rendered) => void): vscode.Disposable;
  daemonStatus(): DaemonStatus;
  /** The daemon's process, when known. */
  daemonPid(): number | undefined;
  /** Stops supervising and stops the daemon. */
  stopDaemon(): Promise<void>;
  /** A request to the daemon, once connected: for the tests. */
  daemonRequest<T>(method: string, params: unknown): Promise<T>;
  /** A command for the open map, as the palette sends them: for the tests. */
  mapCommand(command: MapCommand): Thenable<boolean>;
}

export const HOME_VIEW_ID = "timonier.home";

/** Where the workspace state keeps the boxes moved by hand on the map (ADR 0021). */
const PINS_KEY = "timonier.map.pins";

/** The project of the window: its first folder on disk. Other folders come later. */
function projectRoot(): string | undefined {
  return vscode.workspace.workspaceFolders?.find((folder) => folder.uri.scheme === "file")?.uri.fsPath;
}

export function activate(context: vscode.ExtensionContext): TimonierApi {
  const log = vscode.window.createOutputChannel("Timonier", { log: true });
  context.subscriptions.push(log);
  const root = projectRoot();
  let daemon: DaemonSource;
  if (root) {
    const setting = vscode.workspace.getConfiguration("timonier").get<string>("daemon.path");
    const supervisor = new DaemonSupervisor(daemonProgram(process.env, setting), root, log);
    context.subscriptions.push(supervisor);
    supervisor.start();
    daemon = supervisor;
    context.subscriptions.push(vscode.window.onDidChangeVisibleTextEditors(() => supervisor.announceEditors()));
  } else {
    daemon = {
      status: { state: "unavailable", detail: vscode.l10n.t("Open a folder: Timonier maps the project it holds.") },
      client: undefined,
    };
  }

  // The boxes moved by hand on the map: kept in this workspace, on this machine (H13).
  const pins = {
    get: () => context.workspaceState.get<Record<string, Pin>>(PINS_KEY, {}),
    set: (next: Record<string, Pin>) => context.workspaceState.update(PINS_KEY, next),
  };

  context.subscriptions.push(
    // The overview has no data yet: VSCode then shows its welcome content (package.json,
    // `viewsWelcome`), which carries the « Open Map » button. Plan, todo and log land here later.
    vscode.window.createTreeView<never>(HOME_VIEW_ID, {
      treeDataProvider: { getChildren: () => [], getTreeItem: (element: never) => element },
    }),
    vscode.commands.registerCommand("timonier.openMap", () => {
      try {
        const openCode = (file: string, span: Parameters<typeof revealCode>[2]) =>
          root ? revealCode(root, file, span) : Promise.resolve(undefined);
        MapPanel.show(context.extensionUri, daemon, log, openCode, pins);
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        void vscode.window.showErrorMessage(vscode.l10n.t("Timonier could not open the map: {0}", reason));
      }
    }),
    vscode.commands.registerCommand("timonier.restartDaemon", () => {
      if (daemon instanceof DaemonSupervisor) {
        daemon.restart();
      } else {
        void vscode.window.showInformationMessage(daemon.status.detail ?? "");
      }
    }),
    vscode.commands.registerCommand("timonier.showLog", () => log.show()),
    // Moving a box without dragging it (WCAG 2.5.7): a direction, picked from the palette.
    vscode.commands.registerCommand("timonier.map.moveSelection", async () => {
      const directions: (vscode.QuickPickItem & { direction: Direction })[] = [
        { label: `$(arrow-left) ${vscode.l10n.t("Left")}`, direction: "left" },
        { label: `$(arrow-right) ${vscode.l10n.t("Right")}`, direction: "right" },
        { label: `$(arrow-up) ${vscode.l10n.t("Up")}`, direction: "up" },
        { label: `$(arrow-down) ${vscode.l10n.t("Down")}`, direction: "down" },
      ];
      const picked = await vscode.window.showQuickPick(directions, {
        placeHolder: vscode.l10n.t("Which way does the selection move?"),
      });
      if (!picked) return;
      if (!(await MapPanel.command({ name: "move", target: null, direction: picked.direction }))) {
        void vscode.window.showInformationMessage(vscode.l10n.t("Open the map first: « Timonier: Open Map »."));
      }
    }),
    // One palette command for each action of the map (PLAN.md 4.9).
    ...(
      [
        ["timonier.map.fitView", { name: "fitView" }],
        ["timonier.map.zoomIn", { name: "zoomIn" }],
        ["timonier.map.zoomOut", { name: "zoomOut" }],
        ["timonier.map.closeAll", { name: "closeAll" }],
        ["timonier.map.toggle", { name: "toggle", target: null }],
        ["timonier.map.openCode", { name: "openCode", target: null }],
        ["timonier.map.resetLayout", { name: "resetLayout" }],
      ] as const
    ).map(([id, command]) =>
      vscode.commands.registerCommand(id, async () => {
        if (!(await MapPanel.command(command))) {
          void vscode.window.showInformationMessage(vscode.l10n.t("Open the map first: « Timonier: Open Map »."));
        }
      }),
    ),
  );
  return {
    whenMapReady: () => MapPanel.whenReady(),
    mapRendered: () => MapPanel.rendered(),
    onMapRender: (listener) => MapPanel.onDidRender(listener),
    daemonStatus: () => daemon.status,
    daemonPid: () => (daemon instanceof DaemonSupervisor ? daemon.pid : undefined),
    stopDaemon: async () => {
      if (daemon instanceof DaemonSupervisor) await daemon.stop();
    },
    mapCommand: (command) => MapPanel.command(command),
    daemonRequest: async <T>(method: string, params: unknown) => {
      const client = daemon.client;
      if (!client) throw new Error("not connected to the daemon");
      return client.sendRequest<T>(method, params);
    },
  };
}
