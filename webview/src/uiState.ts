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
  /** What did not change since the last review is quieter (ADR 0024). */
  onlyChanges?: boolean;
}

export const initialUi: UiState = { openRoles: [], expanded: [] };

function strings(value: unknown): string[] | undefined {
  return Array.isArray(value) && value.every((v) => typeof v === "string") ? [...value] : undefined;
}

/** What `getState` gave back, if it is a state of this version. */
export function parseUi(saved: unknown): UiState {
  if (typeof saved !== "object" || saved === null) return initialUi;
  const { openRoles, expanded, focus, onlyChanges } = saved as {
    openRoles?: unknown;
    expanded?: unknown;
    focus?: unknown;
    onlyChanges?: unknown;
  };
  return {
    openRoles: strings(openRoles) ?? [],
    expanded: strings(expanded) ?? [],
    ...(typeof focus === "string" ? { focus } : {}),
    ...(onlyChanges === true ? { onlyChanges } : {}),
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

/** Back to the opening of the map; the filter stays as it was. */
export function closeAll(ui: UiState): Applied {
  return {
    ui: { ...initialUi, ...(ui.onlyChanges ? { onlyChanges: true } : {}) },
    subscribe: ui.expanded.length > 0 ? [] : undefined,
  };
}

/** The filter « only the changes », on or off (ADR 0024). */
export function toggleChanges(ui: UiState): Applied {
  const { onlyChanges, ...rest } = ui;
  return { ui: onlyChanges ? rest : { ...rest, onlyChanges: true } };
}
