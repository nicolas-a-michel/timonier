import * as vscode from "vscode";

import { MapPanel } from "./mapPanel";

/** What `activate` exports. Only the integration tests use it for now. */
export interface TimonierApi {
  /** Resolves once the open map's webview has rendered; undefined when no map is open. */
  whenMapReady(): Promise<void> | undefined;
}

export function activate(context: vscode.ExtensionContext): TimonierApi {
  context.subscriptions.push(
    vscode.commands.registerCommand("timonier.openMap", () => {
      try {
        MapPanel.show(context.extensionUri);
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        void vscode.window.showErrorMessage(vscode.l10n.t("Timonier could not open the map: {0}", reason));
      }
    }),
  );
  return { whenMapReady: () => MapPanel.whenReady() };
}
