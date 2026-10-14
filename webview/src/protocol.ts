/**
 * Messages between the webview and its host (the VSCode extension first).
 * Phase 1 only has the handshake; the full protocol (subscriptions, versioned deltas) comes in
 * Phase 4, with types generated from a single source (PLAN.md task 4.1).
 */
export const PROTOCOL_VERSION = 1;

/** Sent once the webview has rendered: the host knows the bundle runs under its CSP. */
export interface ReadyMessage {
  type: "ready";
  protocol: number;
}

export type WebviewToHost = ReadyMessage;

export function readyMessage(): ReadyMessage {
  return { type: "ready", protocol: PROTOCOL_VERSION };
}
