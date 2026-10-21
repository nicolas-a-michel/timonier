/**
 * The whiteboard and `intent.json` (PLAN.md 7.3, ADR 0039): the board becomes an intent and back
 * the same; moving a shape changes only its place, never its id; the meaning is apart from the
 * geometry.
 */
import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";

import { describe, expect, it } from "vitest";

import type { Intent } from "../protocol";

import { add, connect, draw, history, move, shapeOf, strokeOf, type History, type Shape } from "./board";
import { boardOf, byId, intentOf } from "./intent";

function sample(): History {
  let state = history();
  state = add(state, { ...shapeOf(state.board, "zone", { x: 0, y: 0 }), name: "Domain" } as Shape);
  state = add(state, { ...shapeOf(state.board, "zone", { x: 600, y: 0 }), name: "Infra" } as Shape);
  state = add(state, {
    ...shapeOf(state.board, "box", { x: 40, y: 40 }),
    name: "Parser",
    role: "Tokens to a tree",
  } as Shape);
  state = add(state, { ...shapeOf(state.board, "box", { x: 2000, y: 40 }), name: "Cli" } as Shape);
  state = connect(state, "L-1", "L-2", "forbidden");
  state = add(state, {
    ...shapeOf(state.board, "note", { x: 60, y: 200 }),
    text: "No allocation",
    attachedTo: "C-1",
  } as Shape);
  state = add(state, {
    ...shapeOf(state.board, "budget", { x: 300, y: 200 }),
    attachedTo: "E-1",
    p95Ms: 5,
    calls: 2.4,
  } as Shape);
  const stroke = strokeOf(state.board, [
    [10, 10, 0.5],
    [30, 15, 0.5],
  ]);
  return stroke ? draw(state, stroke) : state;
}

describe("intent.json", () => {
  it("says what the board means, apart from where it is drawn", () => {
    const intent = intentOf(sample().board);
    expect(intent.layers).toEqual([
      { id: "L-1", name: "Domain" },
      { id: "L-2", name: "Infra" },
    ]);
    // The layer comes from the drawing (Q62).
    expect(intent.components).toEqual([
      { id: "C-1", name: "Parser", layer: "L-1", role: "Tokens to a tree" },
      { id: "C-2", name: "Cli", layer: null, role: "" },
    ]);
    expect(intent.edges).toEqual([{ id: "E-1", src: "L-1", dst: "L-2", kind: "depends_on", policy: "forbidden" }]);
    expect(intent.notes).toEqual([{ id: "N-1", text: "No allocation", attached_to: "C-1" }]);
    expect(intent.budgets).toEqual([{ id: "N-2", attached_to: "E-1", p95_ms: 5, calls: 2 }]);
    expect(intent.sketch.shapes["C-1"]).toEqual({ x: 40, y: 40, width: 180, height: 80 });
    expect(intent.sketch.shapes["N-1"]).toEqual({ x: 60, y: 200 });
    expect(intent.sketch.shapes["E-1"]).toBeUndefined();
    expect(intent.sketch.strokes.map((s) => s.id)).toEqual(["S-1"]);
    expect(intent.sketch.next).toEqual({ S: 2, C: 3, L: 3, E: 2, N: 3 });
  });

  it("draws back the same board", () => {
    const { board } = sample();
    const back = boardOf(JSON.parse(JSON.stringify(intentOf(board))) as ReturnType<typeof intentOf>).board;
    const sorted = (shapes: Shape[]) => [...shapes].sort((a, b) => a.id.localeCompare(b.id));
    // Counts are whole in the file.
    const expected = sorted(board.shapes).map((s) => (s.kind === "budget" ? { ...s, calls: 2 } : s));
    expect(sorted(back.shapes)).toEqual(expected);
    expect(back.strokes).toEqual(board.strokes);
    expect(back.next).toEqual(board.next);
    expect(intentOf(back)).toEqual(intentOf(board));
  });

  it("keeps every id when a shape moves, and changes only its place", () => {
    const before = sample();
    const after = move(before, new Map([["C-1", { x: 900, y: 40 }]]));
    const [a, b] = [intentOf(before.board), intentOf(after.board)];
    expect(b.sketch.shapes["C-1"]).toEqual({ x: 900, y: 40, width: 180, height: 80 });
    expect(b.components.map((c) => c.id)).toEqual(a.components.map((c) => c.id));
    // Moved out of Domain into Infra: its layer follows the drawing.
    expect(b.components[0]?.layer).toBe("L-2");
    expect({ ...b.sketch.shapes, "C-1": null }).toEqual({ ...a.sketch.shapes, "C-1": null });
  });

  it("draws nothing from no intent, and nothing from one that does not hold together", () => {
    expect(boardOf(null)).toEqual({ board: boardOf(null).board, problem: null });
    expect(boardOf(null).board.shapes).toEqual([]);
    const broken = intentOf(sample().board);
    broken.edges.push({ id: "E-9", src: "C-1", dst: "C-404", kind: "depends_on", policy: "allowed" });
    // Not an empty board the next gesture would write over the file: the reason.
    expect(boardOf(broken).board.shapes).toEqual([]);
    expect(boardOf(broken).problem).toMatch(/cannot draw/);
  });
});

/** The test set the daemon's tests read too: both sides accept and refuse the same files (I1). */
describe("the test set shared with the daemon", () => {
  const folder = path.join(import.meta.dirname, "..", "..", "..", "crates", "timonier-core", "tests", "intent");
  const read = (kind: string) =>
    readdirSync(path.join(folder, kind)).map((file) => ({
      file,
      intent: {
        layers: [],
        components: [],
        edges: [],
        notes: [],
        budgets: [],
        ...JSON.parse(readFileSync(path.join(folder, kind, file), "utf8")),
      } as Intent,
    }));

  it("draws every file the daemon accepts, and goes back to the same file", () => {
    for (const { file, intent } of read("accepted")) {
      intent.sketch = {
        shapes: {},
        strokes: [],
        next: {},
        ...(intent.sketch as Partial<Intent["sketch"]> | undefined),
      };
      const { board, problem } = boardOf(intent);
      expect(problem, file).toBeNull();
      expect(board.shapes.length + board.strokes.length, file).toBeGreaterThan(0);
      const back = intentOf(board);
      expect(
        back.components.map((c) => c.id),
        file,
      ).toEqual(byId(intent.components).map((c) => c.id));
    }
  });

  it("orders the ids as the daemon does: by number, the named ones after, by name", () => {
    const ids = ["C-db", "C-10", "C-db-2", "C-2", "C-Api"].map((id) => ({ id }));
    expect(byId(ids).map((item) => item.id)).toEqual(["C-2", "C-10", "C-Api", "C-db", "C-db-2"]);
  });

  it("makes no id past the last number the daemon accepts", () => {
    const { board } = boardOf({
      version: 1,
      layers: [],
      components: [{ id: "C-9007199254740991", name: "Last", layer: null, role: "" }],
      edges: [],
      notes: [],
      budgets: [],
      sketch: { shapes: {}, strokes: [], next: {} },
    });
    const state = history(board);
    expect(add(state, shapeOf(state.board, "box", { x: 0, y: 0 }))).toBe(state);
  });

  it("says why it cannot draw a file the daemon refuses", () => {
    for (const { file, intent } of read("refused")) {
      intent.sketch = {
        shapes: {},
        strokes: [],
        next: {},
        ...(intent.sketch as Partial<Intent["sketch"]> | undefined),
      };
      expect(boardOf(intent).problem, file).not.toBeNull();
    }
  });
});
