/**
 * What the webview keeps of its view across reloads (vscode.setState): the open role groups and
 * the boxes the daemon expands. The graph itself stays in the daemon. Pure.
 */
import { toggleExpanded, type Toggle } from "./map/mapModel";
import type { ViewData } from "./viewModel";

export interface UiState {
  openRoles: string[];
  expanded: string[];
  /** A folded box whose contents come first when the map is over its budget (PLAN.md 4.10). */
  focus?: string;
}

export const initialUi: UiState = { openRoles: [], expanded: [] };

function strings(value: unknown): string[] | undefined {
  return Array.isArray(value) && value.every((v) => typeof v === "string") ? [...value] : undefined;
}

/** What `getState` gave back, if it is a state of this version. */
export function parseUi(saved: unknown): UiState {
  if (typeof saved !== "object" || saved === null) return initialUi;
  const { openRoles, expanded, focus } = saved as { openRoles?: unknown; expanded?: unknown; focus?: unknown };
  return {
    openRoles: strings(openRoles) ?? [],
    expanded: strings(expanded) ?? [],
    ...(typeof focus === "string" ? { focus } : {}),
  };
}

export interface Applied {
  ui: UiState;
  /** The view to ask the daemon for, when the expanded boxes changed. */
  subscribe?: string[];
}

export function applyToggle(ui: UiState, toggle: Toggle, view: ViewData | undefined): Applied {
  if (toggle.kind === "focus") return { ui: { ...ui, focus: toggle.id } };
  if (toggle.kind === "role") {
    const open = ui.openRoles.includes(toggle.id);
    const openRoles = open ? ui.openRoles.filter((id) => id !== toggle.id) : [...ui.openRoles, toggle.id].sort();
    return { ui: { ...ui, openRoles } };
  }
  if (!view) return { ui };
  const expanded = toggleExpanded(ui.expanded, toggle.id, view);
  return { ui: { ...ui, expanded }, subscribe: expanded };
}

/** Back to the opening of the map. */
export function closeAll(ui: UiState): Applied {
  return { ui: initialUi, subscribe: ui.expanded.length > 0 ? [] : undefined };
}
