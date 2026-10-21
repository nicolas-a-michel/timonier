import type { PanelKind } from "./TrackingPanel";

/** Which panel a page is: the host says it on the root element (ADR 0031); all otherwise. */
export function panelOf(value: string | null | undefined): PanelKind {
  return value === "plan" || value === "todo" || value === "sessions" || value === "bridge" || value === "workflow"
    ? value
    : "all";
}
