/**
 * Messages between the webview and its host (the VSCode extension first). Their types are
 * generated from the Rust protocol (ADR 0019): `protocol/generated.ts`, never edited by hand.
 */
import { PROTOCOL_VERSION, type HostMessage, type ReviewSummary, type WebviewMessage } from "./protocol/generated";

export * from "./protocol/generated";

/** The summary before the first review: no diff (ADR 0024, Q27). */
export const NO_REVIEW: ReviewSummary = {
  since: null,
  added: [],
  modified: [],
  removed: [],
  dependenciesAdded: [],
  dependenciesRemoved: [],
  cycles: [],
  fans: [],
};

/** Sent once the webview has rendered: the host knows the bundle runs under its CSP. */
export function readyMessage(): WebviewMessage {
  return { type: "ready", protocol: PROTOCOL_VERSION };
}

/** The view with `expanded` open; the top of the map when it is empty. */
export function subscribeTo(expanded: string[] = []): WebviewMessage {
  return { type: "subscribe", view: { expanded } };
}

/** Whether `data`, received by `postMessage`, is one of the host's messages. */
export function isHostMessage(data: unknown): data is HostMessage {
  if (typeof data !== "object" || data === null) return false;
  const { type, density } = data as { type?: unknown; density?: unknown };
  // The density picks the map's metrics: one of the two, or the layout has none (Q32).
  if (type === "density") return density === "comfortable" || density === "compact";
  return type === "snapshot" || type === "delta" || type === "daemon" || type === "command" || type === "pins";
}
