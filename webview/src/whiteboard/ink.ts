/**
 * How a stroke looks (ADR 0036): perfect-freehand turns its points into the outline of the
 * ink, drawn as one filled SVG path.
 */
import { getStroke, type StrokeOptions } from "perfect-freehand";

import type { Point, Stroke } from "./board";

/** The width of the ink, in pixels of the board at zoom 1. */
export const INK_SIZE = 4;
/** The room around the points that the ink takes: the half-width of the pen, and a margin. */
export const INK_PAD = INK_SIZE;

function options(pen: boolean, last: boolean): StrokeOptions {
  return { size: INK_SIZE, thinning: 0.5, smoothing: 0.5, streamline: 0.5, simulatePressure: !pen, last };
}

const average = (a: number, b: number) => (a + b) / 2;

/**
 * The SVG path of an outline: quadratic curves through the middles of its sides (the helper of
 * perfect-freehand's README).
 */
export function outlinePath(outline: readonly (readonly [number, number])[]): string {
  if (outline.length < 4) return "";
  const at = (i: number) => outline[i] as readonly [number, number];
  const fixed = (n: number) => n.toFixed(2);
  const [a, b, c] = [at(0), at(1), at(2)];
  let path = `M${fixed(a[0])},${fixed(a[1])} Q${fixed(b[0])},${fixed(b[1])} ${fixed(average(b[0], c[0]))},${fixed(average(b[1], c[1]))} T`;
  for (let i = 2; i < outline.length - 1; i++) {
    const [p, q] = [at(i), at(i + 1)];
    path += `${fixed(average(p[0], q[0]))},${fixed(average(p[1], q[1]))} `;
  }
  return `${path}Z`;
}

/** The ink of these points, shifted by `dx`, `dy`; `last` once the stroke is finished. */
export function inkPath(points: readonly Point[], pen: boolean, { dx = 0, dy = 0, last = true } = {}): string {
  const shifted = points.map(([x, y, pressure]) => [x + dx, y + dy, pressure]);
  return outlinePath(getStroke(shifted, options(pen, last)));
}

/** The box a stroke's ink takes, its corner at the stroke's corner less the pad. */
export function inkBox(stroke: Stroke): { x: number; y: number; width: number; height: number } {
  const width = Math.max(...stroke.points.map((p) => p[0]));
  const height = Math.max(...stroke.points.map((p) => p[1]));
  return { x: stroke.x - INK_PAD, y: stroke.y - INK_PAD, width: width + 2 * INK_PAD, height: height + 2 * INK_PAD };
}
