import { describe, expect, it } from "vitest";

import { directionOf, nearest, type Box } from "./navigation";

/**
 *   a   b
 *       c   d
 */
const boxes = new Map<string, Box>([
  ["a", { x: 0, y: 0, width: 100, height: 50 }],
  ["b", { x: 200, y: 0, width: 100, height: 50 }],
  ["c", { x: 200, y: 100, width: 100, height: 50 }],
  ["d", { x: 400, y: 100, width: 100, height: 50 }],
]);

describe("arrow navigation", () => {
  it("goes to the nearest item in the direction of the key", () => {
    expect(nearest("a", "right", boxes)).toBe("b");
    expect(nearest("b", "down", boxes)).toBe("c");
    expect(nearest("c", "up", boxes)).toBe("b");
    expect(nearest("c", "right", boxes)).toBe("d");
    expect(nearest("d", "left", boxes)).toBe("c");
  });

  it("stays put at the edge of the map", () => {
    expect(nearest("a", "left", boxes)).toBeUndefined();
    expect(nearest("a", "up", boxes)).toBeUndefined();
    expect(nearest("missing", "up", boxes)).toBeUndefined();
  });

  it("knows the arrow keys only", () => {
    expect(directionOf("ArrowUp")).toBe("up");
    expect(directionOf("Enter")).toBeUndefined();
  });
});
