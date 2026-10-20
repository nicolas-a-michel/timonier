import { isWebviewMessage } from "./messages";
import type { Density, HostMessage, WebviewMessage } from "./protocol/generated";

/** What the map panel does with its page's messages, apart from VSCode itself: for the tests. */
export interface MapHostActions {
  /** A message for the page. */
  post(message: HostMessage): unknown;
  /** The density the settings give now (Q32). */
  density(): Density;
  /** The page has rendered for the first time since it loaded. */
  ready(): void;
  /** The map's « Mark as reviewed » button: the command, confirmation included (ADR 0024). */
  markReviewed(): unknown;
  /** Every other message goes to the view relay. */
  relay(message: WebviewMessage): unknown;
}

/** The density a value of the `timonier.map.density` setting gives: comfortable unless compact. */
export function densityOf(setting: unknown): Density {
  return setting === "compact" ? "compact" : "comfortable";
}

/** A message from the map's page. What fails the validation is dropped. */
export function onPageMessage(message: unknown, actions: MapHostActions): void {
  if (!isWebviewMessage(message)) return;
  if (message.type === "ready") {
    actions.ready();
    // Before the view arrives: the first layout is at the right density.
    actions.post({ type: "density", density: actions.density() });
  }
  if (message.type === "markReviewed") {
    actions.markReviewed();
    return;
  }
  actions.relay(message);
}

/** A change of the settings: the page follows the density at once. */
export function onSettingsChange(affects: (section: string) => boolean, actions: MapHostActions): void {
  if (affects("timonier.map.density")) actions.post({ type: "density", density: actions.density() });
}
