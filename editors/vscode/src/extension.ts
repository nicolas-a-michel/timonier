import * as vscode from "vscode";

import { revealCode } from "./code";
import { DaemonSupervisor, type DaemonStatus } from "./daemon";
import { daemonProgram, initCommand } from "./launch";

/** The terminal where `timonier init` runs (Q53). */
export const INIT_TERMINAL = "Timonier init";
import { MapPanel } from "./mapPanel";
import {
  METHODS,
  type Direction,
  type MapCommand,
  type Pin,
  type ReviewMark,
  type TrackingSnapshot,
} from "./protocol/generated";
import { TrackingRelay, type TrackingSource } from "./trackingRelay";
import { registerTrackingViews } from "./trackingViews";
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
  /** The tracking last received from the daemon (ADR 0031). */
  tracking(): TrackingSnapshot | undefined;
  /** The badge of the bridge: 1 while the project is not set up (Q52). */
  bridgeBadge(): number | undefined;
  /** The revision a panel of the tracking last said it shows; undefined before its page. */
  trackingShown(panel: string): number | undefined;
}

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
  let tracked: TrackingSource;
  if (root) {
    const setting = vscode.workspace.getConfiguration("timonier").get<string>("daemon.path");
    const supervisor = new DaemonSupervisor(daemonProgram(process.env, setting), root, log);
    context.subscriptions.push(supervisor);
    supervisor.start();
    daemon = supervisor;
    tracked = supervisor;
    context.subscriptions.push(vscode.window.onDidChangeVisibleTextEditors(() => supervisor.announceEditors()));
  } else {
    daemon = {
      status: { state: "unavailable", detail: vscode.l10n.t("Open a folder: Timonier maps the project it holds.") },
      client: undefined,
    };
    tracked = daemon;
  }
  let bridgeBadge: number | undefined;
  // The plan, the todo and the journal: one subscription for the three panels (ADR 0031).
  const tracking = new TrackingRelay(tracked, log);
  context.subscriptions.push({ dispose: () => void tracking.dispose() });
  context.subscriptions.push(
    ...registerTrackingViews(
      context.extensionUri,
      tracking,
      log,
      {
        // The map, opened or shown, then the box (Q46).
        reveal: async (target, path) => {
          await vscode.commands.executeCommand("timonier.openMap");
          await MapPanel.command({ name: "reveal", target, path });
        },
        openMap: () => vscode.commands.executeCommand("timonier.openMap"),
        markReviewed: () => vscode.commands.executeCommand("timonier.markReviewed"),
        configure: () => vscode.commands.executeCommand("timonier.configure"),
      },
      (value) => (bridgeBadge = value),
    ),
  );

  // The boxes moved by hand on the map: kept in this workspace, on this machine (H13).
  const pins = {
    get: () => context.workspaceState.get<Record<string, Pin>>(PINS_KEY, {}),
    set: (next: Record<string, Pin>) => context.workspaceState.update(PINS_KEY, next),
  };

  context.subscriptions.push(
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
    // `timonier init` in a terminal, where its questions and what it does show (Q53).
    vscode.commands.registerCommand("timonier.configure", () => {
      if (!root) {
        void vscode.window.showInformationMessage(vscode.l10n.t("Open a folder: Timonier maps the project it holds."));
        return;
      }
      const setting = vscode.workspace.getConfiguration("timonier").get<string>("daemon.path");
      const terminal = vscode.window.createTerminal({ name: INIT_TERMINAL, cwd: root });
      terminal.sendText(initCommand(daemonProgram(process.env, setting)));
      terminal.show();
    }),
    // The bridge's « Where to look », from the palette (ADR 0032): a box, then the map on it.
    vscode.commands.registerCommand("timonier.whereToLook", async () => {
      const items = (tracking.state?.tracking.look ?? []).flatMap((item) => {
        if (item.kind === "debt") {
          const detail = vscode.l10n.t("{0} files to read again", item.debt.files);
          return [
            {
              label: item.item.within ? `${item.item.within} › ${item.item.name}` : item.item.name,
              description: item.debt.untracked > 0 ? `${detail} · ${vscode.l10n.t("untracked")}` : detail,
              target: item.item.id,
              path: item.path,
            },
          ];
        }
        if (item.kind === "cycle" && item.boxes[0]) {
          const first = item.boxes[0];
          return [
            {
              label: item.boxes.map((b) => b.name).join(" → "),
              description: vscode.l10n.t("cycle added"),
              target: first.id,
              path: item.path,
            },
          ];
        }
        return [];
      });
      if (items.length === 0) {
        void vscode.window.showInformationMessage(vscode.l10n.t("Nothing to read again."));
        return;
      }
      const picked = await vscode.window.showQuickPick(items, {
        placeHolder: vscode.l10n.t("Which box to show on the map?"),
      });
      if (!picked) return;
      await vscode.commands.executeCommand("timonier.openMap");
      await MapPanel.command({ name: "reveal", target: picked.target, path: picked.path });
    }),
    // The review diff starts again from here (ADR 0024); a discreet confirmation.
    vscode.commands.registerCommand("timonier.markReviewed", async () => {
      const client = daemon.client;
      if (!client) {
        void vscode.window.showInformationMessage(
          daemon.status.detail ?? vscode.l10n.t("Timonier is not connected to its daemon yet."),
        );
        return;
      }
      try {
        await client.sendRequest<ReviewMark>(METHODS.REVIEW_MARK, null);
        vscode.window.setStatusBarMessage(`$(pass) ${vscode.l10n.t("Timonier: marked as reviewed")}`, 4000);
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        log.warn(`review mark failed: ${reason}`);
        void vscode.window.showErrorMessage(vscode.l10n.t("Timonier could not mark the map as reviewed: {0}", reason));
      }
    }),
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
        ["timonier.map.onlyChanges", { name: "toggleChanges" }],
        ["timonier.map.reviewDetails", { name: "toggleReviewDetails" }],
        ["timonier.map.reviewDebt", { name: "toggleDebt" }],
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
    tracking: () => tracking.state,
    trackingShown: (panel) => tracking.shown(panel),
    bridgeBadge: () => bridgeBadge,
    daemonRequest: async <T>(method: string, params: unknown) => {
      const client = daemon.client;
      if (!client) throw new Error("not connected to the daemon");
      return client.sendRequest<T>(method, params);
    },
  };
}
