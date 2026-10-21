import type { WebviewMessage } from "./protocol/generated";

type Fields = Record<string, unknown>;

const isObject = (value: unknown): value is Fields => typeof value === "object" && value !== null;
const isCount = (value: unknown) => typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
const isStrings = (value: unknown) => Array.isArray(value) && value.every((item) => typeof item === "string");
const isPosition = (value: unknown) => isObject(value) && isCount(value.line) && isCount(value.column);
const isSpan = (value: unknown) =>
  value === null || (isObject(value) && isPosition(value.start) && isPosition(value.end));
/** Pixels of a map: a whole number, not absurdly far. */
const isPixel = (value: unknown) =>
  typeof value === "number" && Number.isSafeInteger(value) && Math.abs(value) <= 10_000_000;

/** The panels of the tracking (ADR 0031). */
export const PANELS = ["all", "bridge", "plan", "todo", "sessions"];

/** At most as many pins as the map keeps (ADR 0021). */
export const MAX_PINS = 5000;

const isPins = (value: unknown) =>
  isObject(value) &&
  !Array.isArray(value) &&
  Object.keys(value).length <= MAX_PINS &&
  Object.values(value).every(
    (pin) => isObject(pin) && typeof pin.parent === "string" && isPixel(pin.x) && isPixel(pin.y),
  );

/**
 * Whether `message` is one of the webview's messages (WebviewMessage), fields included: the
 * extension acts on them, so a page that went wrong cannot make it do something else.
 */
export function isWebviewMessage(message: unknown): message is WebviewMessage {
  if (!isObject(message)) return false;
  switch (message.type) {
    case "ready":
      return isCount(message.protocol);
    case "subscribe":
      return isObject(message.view) && isStrings(message.view.expanded);
    case "resync":
      return true;
    case "displayed":
      return isStrings(message.paths);
    case "openCode":
      return typeof message.file === "string" && isSpan(message.span);
    case "pins":
      return isPins(message.pins);
    case "markReviewed":
      return true;
    case "reveal":
      return typeof message.target === "string" && isStrings(message.path);
    case "openMap":
    case "configure":
      return true;
    case "trackingShown":
      return PANELS.includes(message.panel as string) && isCount(message.revision);
    case "compareIntent":
      return typeof message.on === "boolean";
    case "openWhiteboard":
    case "openPerformance":
      return true;
    case "showPerformance":
      return typeof message.on === "boolean";
    case "chooseRun":
      return typeof message.run === "string" && /^R-[1-9][0-9]*$/.test(message.run);
    case "intent":
      // The board itself is checked by the daemon, the only one that writes the file.
      return isObject(message.intent) && message.intent.version === 1;
    case "rendered":
      return isCount(message.subscription) && isCount(message.revision) && isCount(message.nodes);
    default:
      return false;
  }
}
