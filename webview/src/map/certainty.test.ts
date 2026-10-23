import { describe, expect, it } from "vitest";

import { certaintyMarks, lineCertainty, sureShare } from "./certainty";
import type { MapContent, MapEdge, MapItem } from "./mapModel";

const box = (id: string, parent: string): MapItem =>
  ({ kind: "box", id, parent, open: false, folded: false, node: { id } }) as unknown as MapItem;
const line = (source: string, target: string, confirmed: number, edges: number): MapEdge => ({
  id: `${source}→${target}`,
  source,
  target,
  approximate: confirmed === 0,
  confirmed,
  weight: edges,
  edges,
  kinds: ["calls"],
  review: null,
});

describe("how sure the map is", () => {
  it("counts each line for its ends and what holds them, once, and the whole map", () => {
    const content: MapContent = {
      items: [box("a", "@role/x"), box("a/1", "a"), box("a/2", "a"), box("b", "@role/x")],
      edges: [line("a/1", "a/2", 3, 4), line("a/1", "b", 0, 6)],
    };
    const marks = certaintyMarks(content);
    // Inside `a`: counted once for `a`, though both ends are in it.
    expect(marks.items.get("a")).toEqual({ confirmed: 3, of: 10 });
    expect(marks.items.get("a/2")).toEqual({ confirmed: 3, of: 4 });
    expect(marks.items.get("b")).toEqual({ confirmed: 0, of: 6 });
    expect(marks.whole).toEqual({ confirmed: 3, of: 10 });
    expect(sureShare(marks.whole)).toBe(0.3);
    expect(sureShare({ confirmed: 0, of: 0 })).toBeNull();
  });

  it("draws a line confirmed, partly confirmed, or tree-sitter's alone", () => {
    expect(lineCertainty(4, 4)).toBe("confirmed");
    expect(lineCertainty(1, 4)).toBe("partial");
    expect(lineCertainty(0, 4)).toBe("approximate");
  });
});
