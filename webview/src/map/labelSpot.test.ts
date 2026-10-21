/** Where the word of a forbidden line goes (ADR 0042, reviews n° 6 and 7 of Phase 7). */
import { describe, expect, it } from "vitest";

import { labelSpot, pointAt, type Obstacle } from "./labelSpot";

const path = "M0,0 C50,0 50,100 100,100";
const size = { width: 20, height: 10 };
const box = (x: number, y: number, width: number, height: number, title = false): Obstacle => ({
  x,
  y,
  width,
  height,
  solid: true,
  title,
});

describe("the spot of a forbidden line's word", () => {
  it("reads the curve React Flow draws", () => {
    expect(pointAt(path, 0)).toEqual({ x: 0, y: 0 });
    expect(pointAt(path, 1)).toEqual({ x: 100, y: 100 });
    expect(pointAt(path, 0.5)).toEqual({ x: 50, y: 50 });
    expect(pointAt("M1,2", 0.5)).toBeUndefined();
  });

  it("starts off the middle, where the opposite line puts its own label", () => {
    expect(labelSpot(path, [], size)).toEqual(pointAt(path, 0.3));
  });

  it("goes on a stretch of its line that no box hides", () => {
    // A box over the start of the line, up to t = 0.5: the word goes after it.
    const at = labelSpot(path, [box(-10, -10, 70, 60)], size);
    expect(at).toBeDefined();
    expect(at && (at.x > 60 || at.y > 50)).toBe(true);
  });

  it("covers a box's body rather than its title bar when it must cover one", () => {
    // Boxes along the whole line, but not hiding it (not solid): the body is the lesser cover.
    const title = { ...box(0, 0, 100, 40, true), solid: false };
    const body = { ...box(0, 40, 100, 60), solid: false };
    const at = labelSpot(path, [title, body], size);
    expect(at && at.y - size.height / 2 >= 40).toBe(true);
  });
});
