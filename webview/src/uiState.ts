/**
 * What the webview keeps of its view across reloads (vscode.setState): the open role groups and
 * the boxes the daemon expands. The graph itself stays in the daemon. Pure.
 */
import { roleGroupId, toggleExpanded, type Toggle } from "./map/mapModel";
import type { ViewData } from "./viewModel";

export interface UiState {
  openRoles: string[];
  expanded: string[];
  /** A folded box whose contents come first when the map is over its budget (PLAN.md 4.10). */
  focus?: string;
  /** What did not change since the last review is quieter (ADR 0024). */
  onlyChanges?: boolean;
  /** The review debt shows on the boxes (ADR 0030, Q44). */
  debt?: boolean;
  /** The map is compared with the whiteboard (ADR 0042). */
  intent?: boolean;
}

export const initialUi: UiState = { openRoles: [], expanded: [] };

function strings(value: unknown): string[] | undefined {
  return Array.isArray(value) && value.every((v) => typeof v === "string") ? [...value] : undefined;
}

/** What `getState` gave back, if it is a state of this version. */
export function parseUi(saved: unknown): UiState {
  if (typeof saved !== "object" || saved === null) return initialUi;
  const { openRoles, expanded, focus, onlyChanges, debt, intent } = saved as {
    openRoles?: unknown;
    expanded?: unknown;
    focus?: unknown;
    onlyChanges?: unknown;
    debt?: unknown;
    intent?: unknown;
  };
  return {
    openRoles: strings(openRoles) ?? [],
    expanded: strings(expanded) ?? [],
    ...(typeof focus === "string" ? { focus } : {}),
    ...(onlyChanges === true ? { onlyChanges } : {}),
    ...(debt === true ? { debt } : {}),
    ...(intent === true ? { intent } : {}),
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

/** Back to the opening of the map; the filter and the debt layer stay as they were. */
export function closeAll(ui: UiState): Applied {
  return {
    ui: {
      ...initialUi,
      ...(ui.onlyChanges ? { onlyChanges: true } : {}),
      ...(ui.debt ? { debt: true } : {}),
      ...(ui.intent ? { intent: true } : {}),
    },
    subscribe: ui.expanded.length > 0 ? [] : undefined,
  };
}

/** The layer of the review debt, on or off (ADR 0030). */
export function toggleDebt(ui: UiState): Applied {
  const { debt, ...rest } = ui;
  return { ui: debt ? rest : { ...rest, debt: true } };
}

/** The comparison with the whiteboard, on or off (ADR 0042). */
export function toggleIntent(ui: UiState): Applied {
  const { intent, ...rest } = ui;
  return { ui: intent ? rest : { ...rest, intent: true } };
}

/** The filter « only the changes », on or off (ADR 0024). */
export function toggleChanges(ui: UiState): Applied {
  const { onlyChanges, ...rest } = ui;
  return { ui: onlyChanges ? rest : { ...rest, onlyChanges: true } };
}

/**
 * What shows a box (ADR 0032, Q46): the role group of the top of its path open, and the boxes of
 * its path expanded. Nothing closes.
 */
export function reveal(ui: UiState, target: string, path: readonly string[], view: ViewData | undefined): Applied {
  const top = view?.nodes.get(path[0] ?? target);
  const group = top ? roleGroupId(top.role) : undefined;
  const openRoles = group && !ui.openRoles.includes(group) ? [...ui.openRoles, group].sort() : ui.openRoles;
  const missing = path.filter((id) => !ui.expanded.includes(id));
  const expanded = missing.length > 0 ? [...ui.expanded, ...missing].sort() : ui.expanded;
  return {
    ui: { ...ui, openRoles, expanded },
    ...(missing.length > 0 ? { subscribe: expanded } : {}),
  };
}
