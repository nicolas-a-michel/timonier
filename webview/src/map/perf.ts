/**
 * The map's Performances layer (PLAN.md 8.7, ADR 0051): what each item of the map cost in the
 * run, and the traffic its lines carry. The daemon gives, for each node a span is attached to, its
 * calls, its own time (spans less the spans they called) and the time no other node called it for,
 * and, for each link, the time the callee's spans so took; the map adds them up on the item that
 * shows the node, as the intent layer finds it (ADR 0042). An item's cumulative time (Q76) is the
 * time it was entered for: from no node, or from a node another item shows. Pure.
 */
import type { Locale } from "../i18n";
import { performanceMessages } from "../performance/messages";
import type { BudgetsView, PerfMap } from "../protocol";
import type { ViewData } from "../viewModel";
import { shownBy } from "./intent";
import type { MapContent } from "./mapModel";

/** What an item of the map cost. */
export interface PerfMark {
  calls: number;
  /** Nanoseconds spent in it and in what it called (Q76): what the box says. */
  totalNs: number;
  /** Nanoseconds spent in it, its calls to other nodes aside: in its tooltip and details. */
  selfNs: number;
}

export interface PerfMarks {
  run: string | null;
  items: Map<string, PerfMark>;
  /** The calls each line carries, between items that are not the same. */
  lines: Map<string, number>;
  /** The busiest line's calls: the thickest. */
  most: number;
}

export function perfMarks(view: PerfMap, content: MapContent, data: ViewData): PerfMarks {
  const items = new Map<string, PerfMark>();
  for (const node of view.nodes) {
    const item = shownBy(node.node, view, content, data);
    if (!item) continue;
    const mark = items.get(item) ?? { calls: 0, totalNs: 0, selfNs: 0 };
    mark.calls += node.calls;
    mark.selfNs += node.selfNs;
    mark.totalNs += node.enteredNs;
    items.set(item, mark);
  }
  // Entered from another item: its time counts for the item called.
  for (const link of view.links) {
    const to = shownBy(link.to, view, content, data);
    const mark = to ? items.get(to) : undefined;
    if (mark && shownBy(link.from, view, content, data) !== to) mark.totalNs += link.ns;
  }
  const lines = new Map<string, number>();
  const byEnds = new Map(content.edges.map((edge) => [`${edge.source}\u0000${edge.target}`, edge.id]));
  for (const link of view.links) {
    const from = shownBy(link.from, view, content, data);
    const to = shownBy(link.to, view, content, data);
    if (!from || !to || from === to) continue;
    const line = byEnds.get(`${from}\u0000${to}`);
    if (line) lines.set(line, (lines.get(line) ?? 0) + link.calls);
  }
  return { run: view.run, items, lines, most: Math.max(0, ...lines.values()) };
}

/**
 * How thick a line is drawn for its traffic (ADR 0021: thickness is for the measured traffic):
 * from the usual width up to four times it, on a log scale, so that one busy line does not flatten
 * the others.
 */
export function trafficWidth(calls: number, most: number): number {
  if (calls <= 0 || most <= 0) return 1;
  return 1 + (3 * Math.log1p(calls)) / Math.log1p(most);
}

/** The budgets an item of the map holds that the last runs exceed (ADR 0052), in words. */
export function budgetMarks(
  view: BudgetsView,
  content: MapContent,
  data: ViewData,
  locale: Locale,
): Map<string, string[]> {
  const t = performanceMessages[locale];
  const items = new Map<string, string[]>();
  for (const check of view.checks) {
    if (check.state !== "exceeded") continue;
    const over = check.limits.filter((l) => l.exceeded).map((l) => t.limitWords(l.kind, l.measured ?? 0, l.limit));
    // Attached to nothing: the tests, in the page's language.
    const said = t.budgetWords(check.id, check.attachedTo === null ? t.theTests : check.on, over.join(", "));
    for (const box of check.boxes) {
      const item = shownBy(box, view, content, data);
      if (!item) continue;
      const words = items.get(item) ?? [];
      if (!words.includes(said)) words.push(said);
      items.set(item, words);
    }
  }
  return items;
}
