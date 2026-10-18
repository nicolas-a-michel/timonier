/**
 * Moving the focus on the map with the arrow keys (PLAN.md 4.9): to the nearest item in that
 * direction. Pure.
 */

export type Direction = "left" | "right" | "up" | "down";

export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

const KEYS: Record<string, Direction> = { ArrowLeft: "left", ArrowRight: "right", ArrowUp: "up", ArrowDown: "down" };

export function directionOf(key: string): Direction | undefined {
  return KEYS[key];
}

/**
 * The item whose centre lies in `direction` from `from`'s, nearest along that direction, a step
 * aside costing twice a step ahead; `undefined` when there is none.
 */
export function nearest(from: string, direction: Direction, boxes: ReadonlyMap<string, Box>): string | undefined {
  const start = boxes.get(from);
  if (!start) return undefined;
  const cx = start.x + start.width / 2;
  const cy = start.y + start.height / 2;
  let best: { id: string; cost: number } | undefined;
  for (const [id, box] of boxes) {
    if (id === from) continue;
    const dx = box.x + box.width / 2 - cx;
    const dy = box.y + box.height / 2 - cy;
    const [ahead, aside] =
      direction === "right" ? [dx, dy] : direction === "left" ? [-dx, dy] : direction === "down" ? [dy, dx] : [-dy, dx];
    if (ahead <= 0) continue;
    const cost = ahead + 2 * Math.abs(aside);
    if (!best || cost < best.cost) best = { id, cost };
  }
  return best?.id;
}
