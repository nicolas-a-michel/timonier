/**
 * How sure the map is (PLAN.md 11.5): of the stored links each line stands for, those a
 * manifest, a language server or SCIP confirmed, the others tree-sitter's alone. A box counts
 * the lines that touch it or what it holds, each once; the map counts every line it draws.
 * Pure.
 */
import type { MapContent } from "./mapModel";

/** Links confirmed, of how many. */
export interface Sureness {
  confirmed: number;
  of: number;
}

export interface CertaintyMarks {
  items: Map<string, Sureness>;
  /** The map as drawn: every line it shows. */
  whole: Sureness;
}

export function certaintyMarks(content: MapContent): CertaintyMarks {
  const parentOf = new Map<string, string>();
  for (const item of content.items) if (item.kind === "box") parentOf.set(item.id, item.parent);
  const items = new Map<string, Sureness>();
  const whole: Sureness = { confirmed: 0, of: 0 };
  for (const line of content.edges) {
    whole.confirmed += line.confirmed;
    whole.of += line.edges;
    // The two ends and what holds them, each once: a line inside a box counts for it once.
    const touched = new Set<string>();
    for (const end of [line.source, line.target]) {
      for (let at: string | undefined = end; at !== undefined; at = parentOf.get(at)) touched.add(at);
    }
    for (const id of touched) {
      const sure = items.get(id) ?? { confirmed: 0, of: 0 };
      sure.confirmed += line.confirmed;
      sure.of += line.edges;
      items.set(id, sure);
    }
  }
  return { items, whole };
}

/** The share of a count, 0 to 1; `null` with nothing to count. */
export function sureShare(sure: Sureness): number | null {
  return sure.of > 0 ? sure.confirmed / sure.of : null;
}

/** How a line is drawn: confirmed, partly, or tree-sitter's alone (never by colour alone). */
export function lineCertainty(confirmed: number, of: number): "confirmed" | "partial" | "approximate" {
  if (of > 0 && confirmed >= of) return "confirmed";
  return confirmed > 0 ? "partial" : "approximate";
}
