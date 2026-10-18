/**
 * Messages between the webview and its host (the VSCode extension first). Their types are
 * generated from the Rust protocol (ADR 0019): `protocol/generated.ts`, never edited by hand.
 */
import { PROTOCOL_VERSION, type HostMessage, type WebviewMessage } from "./protocol/generated";

export * from "./protocol/generated";

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
  const { type } = data as { type?: unknown };
  return type === "snapshot" || type === "delta" || type === "daemon" || type === "command" || type === "pins";
}
