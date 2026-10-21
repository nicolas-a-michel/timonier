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
  return (
    type === "snapshot" ||
    type === "delta" ||
    type === "daemon" ||
    type === "command" ||
    type === "pins" ||
    type === "drift" ||
    type === "perfMap" ||
    type === "budgets"
  );
}

/** The host's messages to the Performances panel (ADR 0051): the runs, the run shown, the daemon's state. */
export type PerformanceHostMessage = Extract<
  HostMessage,
  { type: "perfRuns" | "perfRun" | "perfRegressions" | "daemon" }
>;

export function isPerformanceMessage(data: unknown): data is PerformanceHostMessage {
  if (typeof data !== "object" || data === null) return false;
  const { type } = data as { type?: unknown };
  return type === "perfRuns" || type === "perfRun" || type === "perfRegressions" || type === "daemon";
}

/** The host's messages to a panel of the tracking (ADR 0031): the state, and the daemon's. */
export type TrackingHostMessage = Extract<HostMessage, { type: "tracking" | "daemon" }>;

export function isTrackingMessage(data: unknown): data is TrackingHostMessage {
  if (typeof data !== "object" || data === null) return false;
  const { type } = data as { type?: unknown };
  return type === "tracking" || type === "daemon";
}

/** The host's messages to the whiteboard (ADR 0037, 0039): a command of the palette, the board, whether it is saved. */
export type WhiteboardHostMessage = Extract<HostMessage, { type: "whiteboard" | "intent" | "intentSaved" }>;

const TOOLS: readonly string[] = ["select", "pen", "eraser", "box", "zone", "note", "budget", "arrow"];
const SHAPES: readonly string[] = ["box", "zone", "note", "budget"];
const COMMANDS: readonly string[] = ["undo", "redo", "deleteSelection", "fitView", "zoomIn", "zoomOut"];

export function isWhiteboardMessage(data: unknown): data is WhiteboardHostMessage {
  if (typeof data !== "object" || data === null) return false;
  const { type, command } = data as {
    type?: unknown;
    command?: { name?: unknown; tool?: unknown; shape?: unknown } | null;
  };
  // The board and its saving come from the extension, which had them from the daemon.
  if (type === "intent" || type === "intentSaved") return true;
  if (type !== "whiteboard" || typeof command !== "object" || command === null) return false;
  if (command.name === "tool") return TOOLS.includes(command.tool as string);
  if (command.name === "add") return SHAPES.includes(command.shape as string);
  return COMMANDS.includes(command.name as string);
}
