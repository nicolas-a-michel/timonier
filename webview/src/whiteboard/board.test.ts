/**
 * The whiteboard's model (ADR 0036): strokes drawn, erased and moved, and every step undone and
 * redone in order, as the developer did them.
 */
import { describe, expect, it } from "vitest";

import {
  EMPTY_BOARD,
  HISTORY_LIMIT,
  MIN_SIZE,
  add,
  carried,
  connect,
  draw,
  elementAt,
  erase,
  history,
  layerOf,
  move,
  nextId,
  parseBoard,
  redo,
  shapeOf,
  strokeOf,
  undo,
  update,
  type ArrowShape,
  type Board,
  type BoxShape,
  type History,
  type Point,
  type Shape,
  type ZoneShape,
} from "./board";
import { inkBox, inkPath, INK_PAD } from "./ink";

const line: Point[] = [
  [10, 20, 0.5],
  [30, 25, 0.5],
  [50, 40, 0.5],
];

function drawn(state: History, points: Point[] = line): History {
  const stroke = strokeOf(state.board, points);
  if (!stroke) throw new Error("no stroke");
  return draw(state, stroke);
}

describe("a stroke", () => {
  it("keeps its points relative to its top-left corner, and takes the next id", () => {
    const stroke = strokeOf(EMPTY_BOARD, line);
    expect(stroke).toEqual({
      id: "S-1",
      x: 10,
      y: 20,
      points: [
        [0, 0, 0.5],
        [20, 5, 0.5],
        [40, 20, 0.5],
      ],
    });
    expect(strokeOf(EMPTY_BOARD, [])).toBeUndefined();
    expect(strokeOf(EMPTY_BOARD, line, true)?.pen).toBe(true);
  });

  it("is inked as one closed path, inside its box", () => {
    const stroke = strokeOf(EMPTY_BOARD, line);
    if (!stroke) throw new Error("no stroke");
    const path = inkPath(stroke.points, false, { dx: INK_PAD, dy: INK_PAD });
    expect(path).toMatch(/^M[\d.]+,[\d.]+ Q.* T.*Z$/);
    const box = inkBox(stroke);
    expect(box).toEqual({ x: 10 - INK_PAD, y: 20 - INK_PAD, width: 40 + 2 * INK_PAD, height: 20 + 2 * INK_PAD });
    const numbers = [...path.matchAll(/-?[\d.]+/g)].map((m) => Number(m[0]));
    expect(Math.min(...numbers)).toBeGreaterThanOrEqual(0);
    expect(Math.max(...numbers)).toBeLessThanOrEqual(box.width);
  });
});

describe("the history", () => {
  it("undoes and redoes a stroke, and never reuses its id", () => {
    let state = drawn(history());
    expect(state.board.strokes.map((s) => s.id)).toEqual(["S-1"]);
    state = undo(state);
    expect(state.board.strokes).toEqual([]);
    state = redo(state);
    expect(state.board.strokes.map((s) => s.id)).toEqual(["S-1"]);
    state = drawn(undo(state));
    expect(state.board.strokes.map((s) => s.id)).toEqual(["S-2"]);
    // A new step forgets what was undone.
    expect(redo(state)).toBe(state);
  });

  it("puts erased strokes back where they stood in the drawing order", () => {
    let state = drawn(drawn(drawn(history())));
    state = erase(state, ["S-1", "S-3"]);
    expect(state.board.strokes.map((s) => s.id)).toEqual(["S-2"]);
    state = undo(state);
    expect(state.board.strokes.map((s) => s.id)).toEqual(["S-1", "S-2", "S-3"]);
    expect(erase(state, ["S-9"])).toBe(state);
  });

  it("moves strokes in one step, and back", () => {
    let state = drawn(drawn(history()), [[100, 100, 0.5]]);
    state = move(
      state,
      new Map([
        ["S-1", { x: 15, y: 20 }],
        ["S-2", { x: 0, y: 0 }],
      ]),
    );
    expect(state.board.strokes.map(({ x, y }) => [x, y])).toEqual([
      [15, 20],
      [0, 0],
    ]);
    expect(state.past).toHaveLength(3);
    state = undo(state);
    expect(state.board.strokes.map(({ x, y }) => [x, y])).toEqual([
      [10, 20],
      [100, 100],
    ]);
    // A move that goes nowhere is not a step.
    expect(move(state, new Map([["S-1", { x: 10, y: 20 }]]))).toBe(state);
  });

  it(`goes back ${HISTORY_LIMIT} steps at most`, () => {
    let state = history();
    for (let i = 0; i < HISTORY_LIMIT + 5; i++) state = drawn(state);
    expect(state.past).toHaveLength(HISTORY_LIMIT);
    while (state.past.length > 0) state = undo(state);
    expect(state.board.strokes).toHaveLength(5);
  });

  it("does nothing when there is nothing to undo or redo", () => {
    const state = history();
    expect(undo(state)).toBe(state);
    expect(redo(state)).toBe(state);
  });
});

describe("the kept board", () => {
  it("reads back what the page kept, and an empty board from anything else", () => {
    const board = drawn(drawn(history())).board;
    expect(parseBoard(JSON.parse(JSON.stringify(board)))).toEqual(board);
    expect(parseBoard(undefined)).toBe(EMPTY_BOARD);
    expect(parseBoard({ version: 3, strokes: [], shapes: [], next: {} })).toBe(EMPTY_BOARD);
    expect(
      parseBoard({ version: 2, strokes: [{ id: "S-1", x: 0, y: 0, points: [[0, 0]] }], shapes: [], next: {} }),
    ).toBe(EMPTY_BOARD);
    const twice = { ...board, strokes: [board.strokes[0], board.strokes[0]] };
    expect(parseBoard(twice)).toBe(EMPTY_BOARD);
  });

  it("never gives a new stroke the id of a kept one", () => {
    const board = drawn(drawn(history())).board;
    expect(parseBoard({ ...board, next: { S: 1 } }).next.S).toBe(3);
  });
});

describe("the typed shapes (PLAN.md 7.2)", () => {
  /** Two zones, a box in each, an arrow between the zones, a note and a budget on the box. */
  function sample(): History {
    let state = history();
    const domain = shapeOf(state.board, "zone", { x: 0, y: 0 });
    state = add(state, domain);
    const infra = shapeOf(state.board, "zone", { x: 600, y: 0 });
    state = add(state, infra);
    const parser = {
      ...shapeOf(state.board, "box", { x: 40, y: 40 }),
      name: "Parser",
      role: "Turns tokens into a tree",
    };
    state = add(state, parser);
    state = add(state, { ...shapeOf(state.board, "box", { x: 640, y: 40 }), name: "Store" } as Shape);
    state = connect(state, domain.id, infra.id, "forbidden");
    state = add(state, {
      ...shapeOf(state.board, "note", { x: 60, y: 200 }),
      text: "No allocation in the hot loop",
      attachedTo: parser.id,
    } as Shape);
    state = add(state, {
      ...shapeOf(state.board, "budget", { x: 300, y: 200 }),
      attachedTo: parser.id,
      p95Ms: 5,
      memoryMb: 50,
    } as Shape);
    return state;
  }

  it("takes the spec's ids, by kind, never reused", () => {
    const { board } = sample();
    expect(board.shapes.map((s) => [s.kind, s.id])).toEqual([
      ["zone", "L-1"],
      ["zone", "L-2"],
      ["box", "C-1"],
      ["box", "C-2"],
      ["arrow", "E-1"],
      ["note", "N-1"],
      ["budget", "N-2"],
    ]);
    const undone = undo(sample());
    expect(nextId(add(undone, shapeOf(undone.board, "budget", { x: 0, y: 0 })).board, "note")).toBe("N-4");
  });

  it("serializes every kind, and reads it back the same", () => {
    const { board } = sample();
    const text = JSON.stringify(board);
    expect(parseBoard(JSON.parse(text))).toEqual(board);
    const read = JSON.parse(text) as Board;
    expect(read.shapes[4]).toEqual({
      kind: "arrow",
      id: "E-1",
      src: "L-1",
      dst: "L-2",
      policy: "forbidden",
    });
    expect(read.shapes[6]).toEqual({
      kind: "budget",
      id: "N-2",
      x: 300,
      y: 200,
      attachedTo: "C-1",
      p95Ms: 5,
      memoryMb: 50,
    });
  });

  it("refuses a kept board whose shapes do not hold together", () => {
    const { board } = sample();
    const broken = (shapes: unknown[]) => parseBoard({ ...board, shapes });
    expect(broken([...board.shapes, { kind: "arrow", id: "E-9", src: "C-1", dst: "C-404", policy: "allowed" }])).toBe(
      EMPTY_BOARD,
    );
    expect(broken([...board.shapes, { kind: "arrow", id: "E-9", src: "C-1", dst: "N-1", policy: "allowed" }])).toBe(
      EMPTY_BOARD,
    );
    expect(broken([...board.shapes, { kind: "arrow", id: "E-9", src: "C-1", dst: "C-2", policy: "maybe" }])).toBe(
      EMPTY_BOARD,
    );
    expect(broken([...board.shapes, { kind: "note", id: "N-9", x: 0, y: 0, text: "", attachedTo: "S-1" }])).toBe(
      EMPTY_BOARD,
    );
    expect(
      broken([...board.shapes, { kind: "box", id: "L-9", x: 0, y: 0, width: 10, height: 10, name: "", role: "" }]),
    ).toBe(EMPTY_BOARD);
    expect(broken([...board.shapes, { kind: "budget", id: "N-9", x: 0, y: 0, attachedTo: null, p95Ms: -1 }])).toBe(
      EMPTY_BOARD,
    );
  });

  it("reads a board of 7.1 (strokes only) as one with shapes", () => {
    const old = { version: 1, strokes: [{ id: "S-4", x: 1, y: 2, points: [[0, 0, 0.5]] }], next: 7 };
    expect(parseBoard(old)).toEqual({
      version: 2,
      strokes: old.strokes,
      shapes: [],
      next: { S: 7, C: 1, L: 1, E: 1, N: 1 },
    });
  });

  it("puts a box in the smallest zone that holds its middle (Q62)", () => {
    let state = sample();
    const inner = { ...shapeOf(state.board, "zone", { x: 20, y: 20 }), width: 300, height: 200 } as Shape;
    state = add(state, inner);
    const box = (id: string) => state.board.shapes.find((s) => s.id === id) as BoxShape;
    expect(layerOf(state.board, box("C-1"))?.id).toBe("L-3");
    expect(layerOf(state.board, box("C-2"))?.id).toBe("L-2");
    state = move(state, new Map([["C-1", { x: 2000, y: 2000 }]]));
    expect(layerOf(state.board, box("C-1"))).toBeUndefined();
  });

  it("removes the arrows of a removed box and detaches its notes, and undoes it all in one step", () => {
    let state = sample();
    state = erase(state, ["L-1"]);
    expect(state.board.shapes.map((s) => s.id)).toEqual(["L-2", "C-1", "C-2", "N-1", "N-2"]);
    state = erase(state, ["C-1"]);
    const notes = state.board.shapes.filter((s) => s.kind === "note" || s.kind === "budget");
    expect(notes.map((n) => ("attachedTo" in n ? n.attachedTo : "?"))).toEqual([null, null]);
    state = undo(undo(state));
    expect(state.board).toEqual(sample().board);
  });

  it("edits a shape as one step, keeping its id, and refuses what does not hold", () => {
    let state = sample();
    const box = state.board.shapes.find((s) => s.id === "C-1") as BoxShape;
    state = update(state, { ...box, name: "Lexer", width: 10 });
    const edited = state.board.shapes.find((s) => s.id === "C-1") as BoxShape;
    expect(edited.name).toBe("Lexer");
    expect(edited.width).toBe(MIN_SIZE.width);
    expect(undo(state).board.shapes.find((s) => s.id === "C-1")).toEqual(box);
    const arrow = state.board.shapes.find((s) => s.id === "E-1") as ArrowShape;
    expect(update(state, { ...arrow, policy: "required" }).board.shapes.find((s) => s.id === "E-1")).toMatchObject({
      policy: "required",
    });
    expect(update(state, { ...arrow, dst: arrow.src })).toBe(state);
    expect(update(state, { ...box, kind: "zone" })).toBe(state);
    expect(connect(state, "C-1", "C-1")).toBe(state);
  });

  it("moves with a zone what is drawn inside it, and finds what a note is put down on (Q65)", () => {
    const { board } = sample();
    const domain = board.shapes.find((s) => s.id === "L-1") as ZoneShape;
    expect(carried(board, domain)).toEqual(["C-1", "N-1"]);
    expect(elementAt(board, { x: 50, y: 50 })?.id).toBe("C-1");
    expect(elementAt(board, { x: 10, y: 300 })?.id).toBe("L-1");
    expect(elementAt(board, { x: 5000, y: 0 })).toBeUndefined();
  });
});
