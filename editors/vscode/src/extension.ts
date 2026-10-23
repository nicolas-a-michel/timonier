import { execFile } from "node:child_process";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";

import * as vscode from "vscode";

import { revealCode } from "./code";
import { DaemonSupervisor, type DaemonStatus } from "./daemon";
import {
  bundledProgram,
  dataDir,
  installCommandLine,
  onPath,
  programName,
  refreshCopy,
  type Machine,
} from "./commandLine";
import { daemonProgram } from "./launch";
import { recordedCommands, TEST_ARGS } from "./runTests";
import { missingIndexers, SCIP_INSTALL_ARGS } from "./scipIndexers";
import { detectAgents, setUp } from "./setup";

/** The program this package ships (ADR 0059), found at activation. */
let shipped: string | undefined;

/** The program the daemon and `timonier init` run as: the settings', else the shipped one. */
function initProgram(): string {
  return daemonProgram(process.env, vscode.workspace.getConfiguration("timonier").get<string>("daemon.path"), shipped);
}

/** What runs the copy for the agents, on this machine (ADR 0059). */
function machine(): Machine {
  return {
    env: process.env,
    home: homedir(),
    platform: process.platform,
    addToUserPath: (dir) =>
      new Promise((resolve, reject) => {
        // The user's PATH for the next processes; PowerShell tells the running ones it changed.
        const script =
          "$p = [Environment]::GetEnvironmentVariable('Path', 'User'); " +
          `[Environment]::SetEnvironmentVariable('Path', (($p, '${dir.replace(/'/g, "''")}') -ne '' -join ';'), 'User')`;
        execFile("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", script], (error) => {
          if (error) reject(new Error(error.message));
          else resolve();
        });
      }),
  };
}

/**
 * The Performances panel's « Run the tests »: the commands shown, approved by the click (Q84),
 * then `timonier test` in a terminal, which installs a runner's missing program first.
 */
async function runTestsNow(root: string | undefined): Promise<void> {
  if (!root) {
    void vscode.window.showInformationMessage(vscode.l10n.t("Open a folder: Timonier maps the project it holds."));
    return;
  }
  const commands = recordedCommands(root);
  if (commands === null) {
    void vscode.window.showInformationMessage(
      vscode.l10n.t("Set the project up first, with « Set up » in the Overview: it records how its tests run."),
    );
    return;
  }
  if (commands.length === 0) {
    void vscode.window.showInformationMessage(
      vscode.l10n.t("No test command is recorded for this project's languages yet."),
    );
    return;
  }
  const run = vscode.l10n.t("Run the tests");
  const chosen = await vscode.window.showWarningMessage(
    vscode.l10n.t("Run the project's tests on this machine?"),
    {
      modal: true,
      detail: `${vscode.l10n.t(
        "Timonier runs these commands, and installs first what they need and this machine lacks (cargo-nextest, say):",
      )}\n\n${commands.join("\n")}`,
    },
    run,
  );
  if (chosen !== run) return;
  const terminal = vscode.window.createTerminal({
    name: vscode.l10n.t("Timonier tests"),
    cwd: root,
    shellPath: initProgram(),
    shellArgs: TEST_ARGS,
  });
  terminal.show();
}

/**
 * « Install the SCIP indexers » (ADR 0073, Q94): the commands that install the missing ones,
 * shown, approved by the click, then run in a terminal, which asks the daemon to index at once.
 */
async function installScipNow(root: string | undefined): Promise<void> {
  if (!root) {
    void vscode.window.showInformationMessage(vscode.l10n.t("Open a folder: Timonier maps the project it holds."));
    return;
  }
  const json = await new Promise<string | null>((resolve) =>
    execFile(initProgram(), ["scip", "--json", "--root", root], { timeout: 30_000 }, (error, out) =>
      resolve(error ? null : String(out)),
    ),
  );
  const missing = json === null ? null : missingIndexers(json);
  if (missing === null) {
    void vscode.window.showErrorMessage(
      vscode.l10n.t("Timonier could not say which SCIP indexers this project needs."),
    );
    return;
  }
  const byHand =
    missing.byHand.length > 0 ? `\n\n${vscode.l10n.t("To install by hand:")}\n${missing.byHand.join("\n")}` : "";
  if (missing.commands.length === 0) {
    void vscode.window.showInformationMessage(
      missing.byHand.length === 0
        ? vscode.l10n.t("Every SCIP indexer of this project is installed: the map is made exact after each commit.")
        : `${vscode.l10n.t("No SCIP indexer of this project installs by a command.")}${byHand}`,
    );
    return;
  }
  const install = vscode.l10n.t("Install");
  const chosen = await vscode.window.showWarningMessage(
    vscode.l10n.t("Install the SCIP indexers on this machine?"),
    {
      modal: true,
      detail: `${vscode.l10n.t(
        "Timonier runs these commands, then indexes the last commit: the map gets the links tree-sitter misses.",
      )}\n\n${missing.commands.join("\n")}${byHand}`,
    },
    install,
  );
  if (chosen !== install) return;
  const terminal = vscode.window.createTerminal({
    name: vscode.l10n.t("Timonier SCIP"),
    cwd: root,
    shellPath: initProgram(),
    shellArgs: SCIP_INSTALL_ARGS,
  });
  terminal.show();
}

/** Puts the shipped program on the agents' PATH, and says what was done (ADR 0059). */
async function installCommandLineNow(log: vscode.LogOutputChannel): Promise<boolean> {
  if (!shipped) {
    void vscode.window.showInformationMessage(
      vscode.l10n.t(
        "This Timonier package ships no program for this system: install timonier and set timonier.daemon.path.",
      ),
    );
    return false;
  }
  try {
    const installed = await installCommandLine(shipped, machine());
    log.info(`command line: ${installed.linked.join(", ")}; left: ${installed.left.join(", ") || "none"}`);
    if (installed.left.length > 0) {
      void vscode.window.showWarningMessage(
        vscode.l10n.t("Timonier left {0}: it holds another program.", installed.left.join(", ")),
      );
    }
    if (installed.notOnPath) {
      void vscode.window.showInformationMessage(
        process.platform === "win32"
          ? vscode.l10n.t(
              "Timonier added its folder to your PATH: restart VSCode and your agents so that they find it.",
            )
          : vscode.l10n.t("~/.local/bin is not on your PATH: add it in your shell's profile, then restart VSCode."),
      );
    }
    return installed.left.length < 2;
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    void vscode.window.showErrorMessage(vscode.l10n.t("Timonier could not install its command line: {0}", reason));
    return false;
  }
}

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
  type DriftView,
  type BudgetsView,
  type HostMessage,
  type PerfMap,
  type WhiteboardCommand,
} from "./protocol/generated";
import { TrackingRelay, type TrackingSource } from "./trackingRelay";
import { registerTrackingViews } from "./trackingViews";
import type { DaemonSource, Rendered } from "./viewRelay";
import { PerformancePanel } from "./performancePanel";
import { WorkflowPanel } from "./workflowPanel";
import { WhiteboardPanel } from "./whiteboardPanel";

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
  /** The drift the open map was last given (ADR 0042): for the tests. */
  mapDrift(): DriftView | null | undefined;
  /** Resolves once the open whiteboard's page has rendered; undefined when none is open. */
  whenWhiteboardReady(): Promise<void> | undefined;
  /** A command for the open whiteboard, as the palette sends them: for the tests. */
  whiteboardCommand(command: WhiteboardCommand): Promise<boolean>;
  /** Resolves once the open Performances panel's page has rendered; undefined when none is open. */
  whenPerformanceReady(): Promise<void> | undefined;
  /** The Performances layer the open map was last given (ADR 0051): for the tests. */
  mapPerformance(): PerfMap | null | undefined;
  /** The whiteboard's budgets the open map was last given (ADR 0052): for the tests. */
  mapBudgets(): BudgetsView | null | undefined;
  /** What the open process panel was last given (ADR 0056): for the tests. */
  workflow(): HostMessage | undefined;
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
  shipped = bundledProgram(context.extensionPath);
  // A copy installed for the agents follows the extension's updates (ADR 0059).
  const copyDir = dataDir(process.env, homedir());
  if (shipped && existsSync(path.join(copyDir, "bin", programName(process.platform)))) {
    const from = shipped;
    void refreshCopy(from, copyDir).catch((error: unknown) => log.warn(`command line not updated: ${String(error)}`));
  }
  const root = projectRoot();
  let daemon: DaemonSource;
  let tracked: TrackingSource;
  if (root) {
    const supervisor = new DaemonSupervisor(initProgram(), root, log);
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
        openWhiteboard: () => vscode.commands.executeCommand("timonier.openWhiteboard"),
        openPerformance: () => vscode.commands.executeCommand("timonier.openPerformance"),
        markReviewed: () => vscode.commands.executeCommand("timonier.markReviewed"),
        configure: () => vscode.commands.executeCommand("timonier.configure"),
        // A project set up (`init.json`, as the daemon says) shows no setup: nothing to detect.
        detectAgents: () =>
          root && !existsSync(path.join(root, ".timonier", "init.json"))
            ? detectAgents(initProgram(), root)
            : Promise.resolve([]),
        setUp: async (agents) => {
          if (!root) return { ok: false, detail: vscode.l10n.t("Open a folder: Timonier maps the project it holds.") };
          // The agents call `timonier` by its name: it must be on their PATH first (ADR 0059).
          if (shipped && !(await onPath("timonier", process.env.PATH ?? process.env.Path))) {
            await installCommandLineNow(log);
          }
          const done = await setUp(initProgram(), root, agents, log);
          if (done.ok) {
            void vscode.window.showInformationMessage(vscode.l10n.t("Timonier set the project up for its agents."));
          }
          return done;
        },
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
        MapPanel.show(context.extensionUri, daemon, log, openCode, pins, root);
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
    vscode.commands.registerCommand("timonier.installCommandLine", async () => {
      if (await installCommandLineNow(log)) {
        void vscode.window.showInformationMessage(vscode.l10n.t("The timonier command is installed for your agents."));
      }
    }),
    // `timonier init` in a terminal, where its questions and what it does show (Q53).
    vscode.commands.registerCommand("timonier.configure", () => {
      if (!root) {
        void vscode.window.showInformationMessage(vscode.l10n.t("Open a folder: Timonier maps the project it holds."));
        return;
      }
      // The program itself as the terminal's process: no shell to quote for, PowerShell
      // included (T-125).
      const terminal = vscode.window.createTerminal({
        name: INIT_TERMINAL,
        cwd: root,
        shellPath: initProgram(),
        shellArgs: ["init"],
      });
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
        ["timonier.map.compareIntent", { name: "toggleIntent" }],
        ["timonier.map.showPerformance", { name: "togglePerformance" }],
        ["timonier.map.certainty", { name: "toggleCertainty" }],
        ["timonier.map.rename", { name: "rename", target: null }],
      ] as const
    ).map(([id, command]) =>
      vscode.commands.registerCommand(id, async () => {
        if (!(await MapPanel.command(command))) {
          void vscode.window.showInformationMessage(vscode.l10n.t("Open the map first: « Timonier: Open Map »."));
        }
      }),
    ),
    // The Performances panel's « Run the tests », from the command palette too (review 9.5, m2).
    vscode.commands.registerCommand("timonier.runTests", () => runTestsNow(root)),
    vscode.commands.registerCommand("timonier.installScip", () => installScipNow(root)),
    // The Performances panel (ADR 0051).
    vscode.commands.registerCommand("timonier.openPerformance", () => {
      try {
        PerformancePanel.show(context.extensionUri, daemon, log, () => void runTestsNow(root));
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        void vscode.window.showErrorMessage(vscode.l10n.t("Timonier could not open the performances: {0}", reason));
      }
    }),
    // The development process's settings (ADR 0056).
    vscode.commands.registerCommand("timonier.openWorkflow", () => {
      try {
        WorkflowPanel.show(context.extensionUri, daemon, log);
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        void vscode.window.showErrorMessage(vscode.l10n.t("Timonier could not open the process: {0}", reason));
      }
    }),
    // The whiteboard (ADR 0036, 0037): one palette command for each of its actions.
    vscode.commands.registerCommand("timonier.openWhiteboard", () => {
      try {
        WhiteboardPanel.show(context.extensionUri, daemon, root, log);
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        void vscode.window.showErrorMessage(vscode.l10n.t("Timonier could not open the whiteboard: {0}", reason));
      }
    }),
    ...(
      [
        ["timonier.whiteboard.undo", { name: "undo" }],
        ["timonier.whiteboard.redo", { name: "redo" }],
        ["timonier.whiteboard.select", { name: "tool", tool: "select" }],
        ["timonier.whiteboard.pen", { name: "tool", tool: "pen" }],
        ["timonier.whiteboard.eraser", { name: "tool", tool: "eraser" }],
        ["timonier.whiteboard.arrow", { name: "tool", tool: "arrow" }],
        ["timonier.whiteboard.addBox", { name: "add", shape: "box" }],
        ["timonier.whiteboard.addZone", { name: "add", shape: "zone" }],
        ["timonier.whiteboard.addNote", { name: "add", shape: "note" }],
        ["timonier.whiteboard.addBudget", { name: "add", shape: "budget" }],
        ["timonier.whiteboard.deleteSelection", { name: "deleteSelection" }],
        ["timonier.whiteboard.fitView", { name: "fitView" }],
        ["timonier.whiteboard.zoomIn", { name: "zoomIn" }],
        ["timonier.whiteboard.zoomOut", { name: "zoomOut" }],
      ] as const
    ).map(([id, command]) =>
      vscode.commands.registerCommand(id, async () => {
        if (!(await WhiteboardPanel.command(command))) {
          void vscode.window.showInformationMessage(
            vscode.l10n.t("Open the whiteboard first: « Timonier: Open the Whiteboard »."),
          );
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
    mapDrift: () => MapPanel.drift(),
    whenWhiteboardReady: () => WhiteboardPanel.whenReady(),
    whiteboardCommand: (command) => WhiteboardPanel.command(command),
    whenPerformanceReady: () => PerformancePanel.whenReady(),
    mapPerformance: () => MapPanel.performance(),
    mapBudgets: () => MapPanel.budgets(),
    workflow: () => WorkflowPanel.last(),
    daemonRequest: async <T>(method: string, params: unknown) => {
      const client = daemon.client;
      if (!client) throw new Error("not connected to the daemon");
      return client.sendRequest<T>(method, params);
    },
  };
}
