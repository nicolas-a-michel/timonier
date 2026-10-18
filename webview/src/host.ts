import type { WebviewMessage } from "./protocol";

/** The part of the host API the webview uses. */
export interface Host {
  postMessage(message: WebviewMessage): void;
  /** What the webview kept of its view state, across reloads of its page. */
  getState?(): unknown;
  setState?(state: unknown): void;
}

// Injected by VSCode into its webviews; absent on the standalone dev page.
declare const acquireVsCodeApi: (() => Host) | undefined;

/** Connects to the host, if any. VSCode allows a single call per webview session. */
export function connectHost(): Host | undefined {
  return typeof acquireVsCodeApi === "function" ? acquireVsCodeApi() : undefined;
}
