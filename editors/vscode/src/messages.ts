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
    case "rendered":
      return isCount(message.subscription) && isCount(message.revision) && isCount(message.nodes);
    default:
      return false;
  }
}
