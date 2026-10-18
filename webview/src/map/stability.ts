/**
 * How much a layout moved what did not change (PLAN.md 4.8): the measure the stable layout is
 * held to. Pure.
 */
import type { Layout } from "./layout";
import type { MapContent } from "./mapModel";

/** Each item's position on the map, its containers' offsets added. */
export function absolute(content: MapContent, placed: Layout): Map<string, { x: number; y: number }> {
  const parents = new Map(content.items.map((item) => [item.id, item.kind === "box" ? item.parent : undefined]));
  const result = new Map<string, { x: number; y: number }>();
  const at = (id: string): { x: number; y: number } | undefined => {
    const known = result.get(id);
    if (known) return known;
    const own = placed.get(id);
    if (!own) return undefined;
    const parent = parents.get(id);
    const offset = parent ? at(parent) : { x: 0, y: 0 };
    if (!offset) return undefined;
    const position = { x: offset.x + own.x, y: offset.y + own.y };
    result.set(id, position);
    return position;
  };
  for (const id of placed.keys()) at(id);
  return result;
}

/** The mean distance, in pixels, that the items of both layouts moved from one to the other. */
export function meanDisplacement(
  before: Map<string, { x: number; y: number }>,
  after: Map<string, { x: number; y: number }>,
  only?: ReadonlySet<string>,
): number {
  let total = 0;
  let count = 0;
  for (const [id, a] of before) {
    const b = after.get(id);
    if (!b || (only && !only.has(id))) continue;
    total += Math.hypot(b.x - a.x, b.y - a.y);
    count++;
  }
  return count === 0 ? 0 : total / count;
}
