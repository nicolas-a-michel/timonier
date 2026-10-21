/**
 * Where the word of a line that crosses a forbidden edge goes (ADR 0042, reviews n° 6 and 7 of
 * Phase 7): React Flow draws every box over the lines, and two lines of opposite ways share
 * their middle. The word goes on a stretch of its line that no box hides, where it covers the
 * least, and above all no title bar; off the middle, where the opposite line puts its label.
 * Pure.
 */

export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** What a label may lie over: a box. `solid`: the line is hidden under it; `title`: its words. */
export interface Obstacle extends Box {
  solid: boolean;
  title: boolean;
}

export interface Spot {
  x: number;
  y: number;
}

/** The point at `t` of a cubic Bézier path as `getBezierPath` writes it (`M… C… … …`). */
export function pointAt(path: string, t: number): Spot | undefined {
  const numbers = path.match(/-?\d+(?:\.\d+)?(?:e-?\d+)?/gi)?.map(Number);
  if (!numbers || numbers.length < 8) return undefined;
  const [x0, y0, x1, y1, x2, y2, x3, y3] = numbers as [number, number, number, number, number, number, number, number];
  const u = 1 - t;
  const a = u * u * u;
  const b = 3 * u * u * t;
  const c = 3 * u * t * t;
  const d = t * t * t;
  return { x: a * x0 + b * x1 + c * x2 + d * x3, y: a * y0 + b * y1 + c * y2 + d * y3 };
}

const inside = (box: Box, at: Spot) =>
  at.x > box.x && at.x < box.x + box.width && at.y > box.y && at.y < box.y + box.height;

const overlap = (a: Box, b: Box) =>
  Math.max(0, Math.min(a.x + a.width, b.x + b.width) - Math.max(a.x, b.x)) *
  Math.max(0, Math.min(a.y + a.height, b.y + b.height) - Math.max(a.y, b.y));

/** How bad a spot is: hidden by a box first, then what it covers, a title bar most. */
const HIDDEN = 1e12;
const TITLE = 50;
// Below a title bar: a word over the opposite line's label is drawn on top of it, still seen.
const MIDDLE = 2000;

/**
 * Where along the line the label of `size` goes: the visible stretch where it covers least,
 * from near its start (t = 0.3) rather than its middle. `undefined`: no path.
 */
export function labelSpot(path: string, obstacles: readonly Obstacle[], size: { width: number; height: number }) {
  let best: { at: Spot; score: number } | undefined;
  for (let i = 3; i <= 47; i++) {
    const t = i / 50;
    const at = pointAt(path, t);
    if (!at) return undefined;
    const label = { x: at.x - size.width / 2, y: at.y - size.height / 2, width: size.width, height: size.height };
    let score = Math.abs(t - 0.3) + (Math.abs(t - 0.5) < 0.08 ? MIDDLE : 0);
    for (const box of obstacles) {
      if (box.solid && inside(box, at)) score += HIDDEN;
      score += overlap(label, box) * (box.title ? TITLE : 1);
    }
    if (!best || score < best.score) best = { at, score };
  }
  return best?.at;
}
