/**
 * The map compared with the whiteboard (PLAN.md 7.6, ADR 0042): which drawn item stands for
 * which drawn component, and which line crosses a forbidden edge. The daemon anchors and
 * compares (Q68); the map only finds, for each real box the drift cites, the item that shows
 * it: the box itself when drawn, else the deepest drawn box that holds it, else its role group.
 * Pure.
 */
import type { DriftView } from "../protocol";
import type { ViewData } from "../viewModel";
import { roleGroupId, type MapContent } from "./mapModel";

/** What an item of the map says of the whiteboard. */
export interface IntentMark {
  /** Drawn components that stand for this very box. */
  drawn: string[];
  /** Drawn components that stand for a box inside it, not drawn on the map. */
  within: string[];
  /** For a role group: its members that a drawn component stands for (`drawn`), or that hold
   * one deeper; marked in its list (reviews n° 2 and 3, I-2, m-1). */
  members: { id: string; name: string; drawn: boolean; names: string[] }[];
}

export interface IntentMarks {
  items: Map<string, IntentMark>;
  /** The lines that cross a forbidden edge, and the edges they cross (« Front → Data »). */
  lines: Map<string, string[]>;
}

/** The name of a drawn end, as the panel and the lines say it. */
export function drawnName(view: DriftView, id: string): string {
  return view.names[id] || id;
}

/** The item that shows a real box on this map, if any. */
export function shownBy(id: string, view: DriftView, content: MapContent, data: ViewData): string | undefined {
  const drawn = new Set(content.items.map((item) => item.id));
  const line = [...(view.ancestors[id] ?? []), id];
  for (let i = line.length - 1; i >= 0; i--) {
    const at = line[i];
    if (at !== undefined && drawn.has(at)) return at;
  }
  const top = data.nodes.get(line[0] ?? id);
  const group = top ? roleGroupId(top.role) : undefined;
  return group && drawn.has(group) ? group : undefined;
}

export function intentMarks(view: DriftView, content: MapContent, data: ViewData): IntentMarks {
  const items = new Map<string, IntentMark>();
  for (const anchor of view.drift.anchors) {
    if (!anchor.actual) continue;
    const item = shownBy(anchor.actual, view, content, data);
    if (!item) continue;
    const mark = items.get(item) ?? { drawn: [], within: [], members: [] };
    (item === anchor.actual ? mark.drawn : mark.within).push(anchor.name || anchor.component);
    const member = (view.ancestors[anchor.actual] ?? [])[0] ?? anchor.actual;
    if (item.startsWith("@role/")) {
      const drawn = member === anchor.actual;
      const name = anchor.name || anchor.component;
      const known = mark.members.find((m) => m.id === member);
      if (!known) mark.members.push({ id: member, name: data.nodes.get(member)?.name ?? member, drawn, names: [name] });
      else {
        known.drawn ||= drawn;
        known.names.push(name);
      }
    }
    items.set(item, mark);
  }
  const lines = new Map<string, string[]>();
  for (const crossing of view.drift.forbidden) {
    const said = `${drawnName(view, crossing.src)} → ${drawnName(view, crossing.dst)}`;
    for (const dependency of crossing.dependencies) {
      const source = shownBy(dependency.from, view, content, data);
      const target = shownBy(dependency.to, view, content, data);
      if (!source || !target || source === target) continue;
      for (const edge of content.edges) {
        if (edge.source !== source || edge.target !== target) continue;
        const crossed = lines.get(edge.id) ?? [];
        if (!crossed.includes(said)) crossed.push(said);
        lines.set(edge.id, crossed);
      }
    }
  }
  return { items, lines };
}
