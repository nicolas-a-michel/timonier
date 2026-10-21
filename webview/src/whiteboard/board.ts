/**
 * What the whiteboard holds (ADR 0036, 0038): freehand strokes, and the typed shapes that count
 * as constraints (a box is an intended component, an arrow a dependency with a policy, a zone a
 * layer, a note a constraint in words, a budget note a measurable limit), in the board's
 * coordinates; and the history that undoes and redoes what the developer did. Pure: the page
 * keeps the state.
 */

/** A point of a stroke: where, and how hard the pen pressed (0 to 1). */
export type Point = [x: number, y: number, pressure: number];

export interface XY {
  x: number;
  y: number;
}

export interface Rect extends XY {
  width: number;
  height: number;
}

/** A freehand stroke: its points, relative to its top-left corner `x`, `y`. */
export interface Stroke {
  id: string;
  x: number;
  y: number;
  points: Point[];
  /** Drawn with a pen that says its pressure; otherwise the pressure follows the speed. */
  pen?: boolean;
}

/** What an arrow says of the dependency it draws (spec « Whiteboard »). */
export type Policy = "allowed" | "forbidden" | "required";
export const POLICIES: readonly Policy[] = ["allowed", "forbidden", "required"];

/** An intended component: its name, and what it does in a sentence (Q63). */
export interface BoxShape extends Rect {
  kind: "box";
  id: string;
  name: string;
  role: string;
}

/** A layer: the boxes drawn inside it are in it (Q62). */
export interface ZoneShape extends Rect {
  kind: "zone";
  id: string;
  name: string;
}

/** A dependency between two boxes or zones; only its policy is chosen (Q64). */
export interface ArrowShape {
  kind: "arrow";
  id: string;
  src: string;
  dst: string;
  policy: Policy;
}

/** A constraint in words, attached to an element or not (Q65). */
export interface NoteShape extends XY {
  kind: "note";
  id: string;
  text: string;
  attachedTo: string | null;
}

/** The limits a budget note can set (spec « Performance budgets »); none is required. */
export interface Limits {
  /** Latency, in milliseconds. */
  p50Ms?: number;
  p95Ms?: number;
  /** Peak memory, in megabytes. */
  memoryMb?: number;
  allocations?: number;
  calls?: number;
}
export const LIMITS: readonly (keyof Limits)[] = ["p50Ms", "p95Ms", "memoryMb", "allocations", "calls"];

/** A measurable limit on a component (a box) or a path (an arrow). */
export interface BudgetShape extends XY, Limits {
  kind: "budget";
  id: string;
  attachedTo: string | null;
}

export type Shape = BoxShape | ZoneShape | ArrowShape | NoteShape | BudgetShape;
export type ShapeKind = Shape["kind"];
/** What a note or a budget can be attached to. */
export type Attachable = BoxShape | ZoneShape | ArrowShape;

/** The prefix of each kind's ids (spec: `C-`, `E-`, `L-`, `N-`), and the strokes'. */
export const PREFIX = { stroke: "S", box: "C", zone: "L", arrow: "E", note: "N", budget: "N" } as const;
export type Prefix = (typeof PREFIX)[keyof typeof PREFIX];
const PREFIXES: readonly Prefix[] = ["S", "C", "L", "E", "N"];

export interface Board {
  version: 2;
  /** In drawing order: the last one is on top. */
  strokes: Stroke[];
  /** In order of creation. */
  shapes: Shape[];
  /** The number of each prefix's next id: ids are never reused, an undone element included. */
  next: Record<Prefix, number>;
}

/** An element and where it stands in its list. */
export type Placed = { stroke: Stroke; index: number } | { shape: Shape; index: number };

/** What the developer did, and what undoes it. */
export type Operation =
  | { kind: "insert"; placed: Placed[] }
  | { kind: "remove"; placed: Placed[] }
  | { kind: "move"; moves: { id: string; from: XY; to: XY }[] }
  | { kind: "update"; before: Shape; after: Shape }
  /** Several operations as one step, in order. */
  | { kind: "batch"; operations: Operation[] };

export interface History {
  board: Board;
  past: Operation[];
  future: Operation[];
}

/** How many steps undo goes back. */
export const HISTORY_LIMIT = 200;
/** At most as many strokes and shapes as a board keeps, points in a stroke, letters in a text. */
export const MAX_STROKES = 10_000;
export const MAX_SHAPES = 2_000;
export const MAX_POINTS = 5_000;
export const MAX_TEXT = 2_000;
export const MAX_NAME = 200;

/** The size a new shape takes, before the developer changes it. */
export const DEFAULT_SIZE: Record<"box" | "zone", { width: number; height: number }> = {
  box: { width: 180, height: 80 },
  zone: { width: 480, height: 320 },
};
/** The smallest a box or a zone can be made. */
export const MIN_SIZE = { width: 60, height: 40 };

export const EMPTY_BOARD: Board = { version: 2, strokes: [], shapes: [], next: { S: 1, C: 1, L: 1, E: 1, N: 1 } };

export function history(board: Board = EMPTY_BOARD): History {
  return { board, past: [], future: [] };
}

/** Two decimals are enough on a board, and keep the state small. */
const round = (value: number) => Math.round(value * 100) / 100;

const idOf = (placed: Placed) => ("stroke" in placed ? placed.stroke.id : placed.shape.id);

function numberOf(id: string): { prefix: Prefix; n: number } | undefined {
  const match = /^([SCLEN])-([1-9]\d*)$/.exec(id);
  if (!match) return undefined;
  const n = Number(match[2]);
  return Number.isSafeInteger(n) ? { prefix: match[1] as Prefix, n } : undefined;
}

/**
 * The prefix of a well-formed id (Q69): numbered as the whiteboard makes them (`C-12`), or named
 * as the spec's example writes them by hand (`C-parser`); the same rule as the daemon's.
 */
function prefixOf(id: string): Prefix | undefined {
  const numbered = numberOf(id);
  if (numbered) return numbered.prefix;
  const named = /^([SCLEN])-([A-Za-z0-9][A-Za-z0-9_-]{0,63})$/.exec(id);
  return named && !/^\d+$/.test(named[2] ?? "") ? (named[1] as Prefix) : undefined;
}

/** A text's length as the daemon counts it: in characters, not UTF-16 units. */
const length = (text: string) => [...text].length;
/** The text cut to `max` characters. */
const cut = (text: string, max: number) => (length(text) <= max ? text : [...text].slice(0, max).join(""));

/** The counters once these ids exist: past the highest of each prefix. */
function counted(next: Record<Prefix, number>, ids: Iterable<string>): Record<Prefix, number> {
  const counters = { ...next };
  for (const id of ids) {
    const number = numberOf(id);
    if (number) counters[number.prefix] = Math.max(counters[number.prefix], number.n + 1);
  }
  return counters;
}

/** The next id of a kind on this board. */
export function nextId(board: Board, kind: keyof typeof PREFIX): string {
  const prefix = PREFIX[kind];
  return `${prefix}-${board.next[prefix]}`;
}

/**
 * A stroke from the points the pointer went through, in the board's coordinates; `undefined`
 * when there is nothing to keep.
 */
export function strokeOf(board: Board, points: readonly Point[], pen = false): Stroke | undefined {
  if (points.length === 0) return undefined;
  const kept = points.slice(0, MAX_POINTS);
  const x = round(Math.min(...kept.map((p) => p[0])));
  const y = round(Math.min(...kept.map((p) => p[1])));
  return {
    id: nextId(board, "stroke"),
    x,
    y,
    points: kept.map(([px, py, pressure]) => [round(px - x), round(py - y), round(pressure)]),
    ...(pen ? { pen: true } : {}),
  };
}

/** A new shape of a kind, its top-left corner at `at`; an arrow is made with `connect`. */
export function shapeOf(board: Board, kind: Exclude<ShapeKind, "arrow">, at: XY): Shape {
  const x = round(at.x);
  const y = round(at.y);
  switch (kind) {
    case "box":
      return { kind, id: nextId(board, kind), x, y, ...DEFAULT_SIZE.box, name: "", role: "" };
    case "zone":
      return { kind, id: nextId(board, kind), x, y, ...DEFAULT_SIZE.zone, name: "" };
    case "note":
      return { kind, id: nextId(board, kind), x, y, text: "", attachedTo: null };
    case "budget":
      return { kind, id: nextId(board, kind), x, y, attachedTo: null };
  }
}

/** The operation that undoes `operation`. */
export function inverse(operation: Operation): Operation {
  switch (operation.kind) {
    case "insert":
      return { kind: "remove", placed: operation.placed };
    case "remove":
      return { kind: "insert", placed: operation.placed };
    case "move":
      return { kind: "move", moves: operation.moves.map(({ id, from, to }) => ({ id, from: to, to: from })) };
    case "update":
      return { kind: "update", before: operation.after, after: operation.before };
    case "batch":
      return { kind: "batch", operations: operation.operations.map(inverse).reverse() };
  }
}

/** Puts each element back at its place, in the order of the places. */
function inserted<T>(list: readonly T[], placed: { item: T; index: number }[]): T[] {
  const result = [...list];
  for (const { item, index } of [...placed].sort((a, b) => a.index - b.index)) {
    result.splice(Math.min(index, result.length), 0, item);
  }
  return result;
}

/** The board once `operation` is done. */
export function apply(board: Board, operation: Operation): Board {
  switch (operation.kind) {
    case "insert": {
      const strokes = operation.placed.flatMap((p) => ("stroke" in p ? [{ item: p.stroke, index: p.index }] : []));
      const shapes = operation.placed.flatMap((p) => ("shape" in p ? [{ item: p.shape, index: p.index }] : []));
      return {
        ...board,
        strokes: inserted(board.strokes, strokes),
        shapes: inserted(board.shapes, shapes),
        next: counted(board.next, operation.placed.map(idOf)),
      };
    }
    case "remove": {
      const gone = new Set(operation.placed.map(idOf));
      return {
        ...board,
        strokes: board.strokes.filter((stroke) => !gone.has(stroke.id)),
        shapes: board.shapes.filter((shape) => !gone.has(shape.id)),
      };
    }
    case "move": {
      const to = new Map(operation.moves.map(({ id, to }) => [id, to]));
      const moved = <T extends { id: string }>(item: T): T => {
        const at = to.get(item.id);
        return at && "x" in item ? { ...item, x: round(at.x), y: round(at.y) } : item;
      };
      return { ...board, strokes: board.strokes.map(moved), shapes: board.shapes.map(moved) };
    }
    case "update":
      return {
        ...board,
        shapes: board.shapes.map((shape) => (shape.id === operation.after.id ? operation.after : shape)),
      };
    case "batch":
      return operation.operations.reduce(apply, board);
  }
}

/** Whether `operation` changes anything: an empty one is not kept in the history. */
function changes(operation: Operation): boolean {
  switch (operation.kind) {
    case "move":
      return operation.moves.some(({ from, to }) => from.x !== to.x || from.y !== to.y);
    case "update":
      return JSON.stringify(operation.before) !== JSON.stringify(operation.after);
    case "batch":
      return operation.operations.some(changes);
    default:
      return operation.placed.length > 0;
  }
}

function inserts(operation: Operation): { strokes: number; shapes: number } {
  if (operation.kind === "insert") {
    const strokes = operation.placed.filter((p) => "stroke" in p).length;
    return { strokes, shapes: operation.placed.length - strokes };
  }
  if (operation.kind === "batch") {
    return operation.operations
      .map(inserts)
      .reduce((a, b) => ({ strokes: a.strokes + b.strokes, shapes: a.shapes + b.shapes }), {
        strokes: 0,
        shapes: 0,
      });
  }
  return { strokes: 0, shapes: 0 };
}

/** Does `operation`, and keeps it for undo; what was undone can no longer be redone. */
export function perform(state: History, operation: Operation): History {
  if (!changes(operation)) return state;
  // An id past the last number the daemon accepts is not made (review n° 2 of Phase 7, m-3).
  if (operation.kind === "insert" && !operation.placed.every((placed) => prefixOf(idOf(placed)))) return state;
  const added = inserts(operation);
  if (state.board.strokes.length + added.strokes > MAX_STROKES) return state;
  if (state.board.shapes.length + added.shapes > MAX_SHAPES) return state;
  return {
    board: apply(state.board, operation),
    past: [...state.past, operation].slice(-HISTORY_LIMIT),
    future: [],
  };
}

export function undo(state: History): History {
  const last = state.past.at(-1);
  if (!last) return state;
  return { board: apply(state.board, inverse(last)), past: state.past.slice(0, -1), future: [last, ...state.future] };
}

export function redo(state: History): History {
  const next = state.future[0];
  if (!next) return state;
  return { board: apply(state.board, next), past: [...state.past, next], future: state.future.slice(1) };
}

/** Adds a stroke on top. */
export function draw(state: History, stroke: Stroke): History {
  return perform(state, { kind: "insert", placed: [{ stroke, index: state.board.strokes.length }] });
}

/** Adds a shape; a note or a budget may come attached. */
export function add(state: History, shape: Shape): History {
  if (shape.kind === "arrow" && !connectable(state.board, shape.src, shape.dst)) return state;
  return perform(state, { kind: "insert", placed: [{ shape, index: state.board.shapes.length }] });
}

/** Whether an arrow can go from `src` to `dst`: two different boxes or zones on the board. */
export function connectable(board: Board, src: string, dst: string): boolean {
  if (src === dst) return false;
  const ends = board.shapes.filter((shape) => shape.id === src || shape.id === dst);
  return ends.length === 2 && ends.every((shape) => shape.kind === "box" || shape.kind === "zone");
}

/** An arrow from `src` to `dst`, allowed until the developer says otherwise. */
export function connect(state: History, src: string, dst: string, policy: Policy = "allowed"): History {
  return add(state, { kind: "arrow", id: nextId(state.board, "arrow"), src, dst, policy });
}

/**
 * Removes these elements, in one step; with them go the arrows that lose an end, and the notes
 * attached to what goes are detached.
 */
export function erase(state: History, ids: Iterable<string>): History {
  const gone = new Set(ids);
  for (const shape of state.board.shapes) {
    if (shape.kind === "arrow" && (gone.has(shape.src) || gone.has(shape.dst))) gone.add(shape.id);
  }
  const placed: Placed[] = [
    ...state.board.strokes.flatMap((stroke, index) => (gone.has(stroke.id) ? [{ stroke, index }] : [])),
    ...state.board.shapes.flatMap((shape, index) => (gone.has(shape.id) ? [{ shape, index }] : [])),
  ];
  if (placed.length === 0) return state;
  const detached: Operation[] = state.board.shapes.flatMap((shape) =>
    (shape.kind === "note" || shape.kind === "budget") &&
    shape.attachedTo &&
    gone.has(shape.attachedTo) &&
    !gone.has(shape.id)
      ? [{ kind: "update" as const, before: shape, after: { ...shape, attachedTo: null } }]
      : [],
  );
  return perform(state, { kind: "batch", operations: [...detached, { kind: "remove", placed }] });
}

/** Moves elements to these corners, in one step. */
export function move(state: History, to: ReadonlyMap<string, XY>): History {
  const moves = [...state.board.strokes, ...state.board.shapes].flatMap((item) => {
    const at = to.get(item.id);
    return at && "x" in item
      ? [{ id: item.id, from: { x: item.x, y: item.y }, to: { x: round(at.x), y: round(at.y) } }]
      : [];
  });
  return perform(state, { kind: "move", moves });
}

/** Changes a shape, as one step: its words, its policy, its size, what it is attached to. */
export function update(state: History, after: Shape): History {
  const before = state.board.shapes.find((shape) => shape.id === after.id);
  if (!before || before.kind !== after.kind) return state;
  const checked = sanitize(after, state.board);
  return checked ? perform(state, { kind: "update", before, after: checked }) : state;
}

/** The shape with its words cut to size and its numbers kept sensible; `undefined` if it cannot be. */
function sanitize(shape: Shape, board: Board): Shape | undefined {
  switch (shape.kind) {
    case "box":
    case "zone": {
      const sized = {
        ...shape,
        x: round(shape.x),
        y: round(shape.y),
        width: round(Math.max(MIN_SIZE.width, shape.width)),
        height: round(Math.max(MIN_SIZE.height, shape.height)),
        name: cut(shape.name, MAX_NAME),
      };
      return sized.kind === "box" ? { ...sized, role: cut(sized.role, MAX_TEXT) } : sized;
    }
    case "arrow":
      return connectable(board, shape.src, shape.dst) && POLICIES.includes(shape.policy) ? shape : undefined;
    case "note":
      return attachableTo(board, shape.attachedTo) ? { ...shape, text: cut(shape.text, MAX_TEXT) } : undefined;
    case "budget": {
      if (!attachableTo(board, shape.attachedTo)) return undefined;
      const limits: Limits = {};
      for (const key of LIMITS) {
        const value = shape[key];
        if (value !== undefined && isLimit(value)) limits[key] = value;
      }
      return { kind: "budget", id: shape.id, x: shape.x, y: shape.y, attachedTo: shape.attachedTo, ...limits };
    }
  }
}

function attachableTo(board: Board, id: string | null): boolean {
  if (id === null) return true;
  const target = board.shapes.find((shape) => shape.id === id);
  return target !== undefined && (target.kind === "box" || target.kind === "zone" || target.kind === "arrow");
}

const isLimit = (value: number) => Number.isFinite(value) && value >= 0 && value <= 1e12;

/** The rectangle a shape takes on the board; notes and budgets have a fixed width. */
export const NOTE_WIDTH = 200;
export const NOTE_HEIGHT = 72;
export function rectOf(shape: Exclude<Shape, ArrowShape>): Rect {
  if (shape.kind === "box" || shape.kind === "zone") return shape;
  return { x: shape.x, y: shape.y, width: NOTE_WIDTH, height: NOTE_HEIGHT };
}

const inside = (outer: Rect, x: number, y: number) =>
  x >= outer.x && x <= outer.x + outer.width && y >= outer.y && y <= outer.y + outer.height;
const contains = (outer: Rect, inner: Rect) =>
  inside(outer, inner.x, inner.y) && inside(outer, inner.x + inner.width, inner.y + inner.height);

/**
 * The layer of a box (Q62): the smallest zone that holds its middle; computed from the drawing,
 * never kept.
 */
export function layerOf(board: Board, box: Rect): ZoneShape | undefined {
  const x = box.x + box.width / 2;
  const y = box.y + box.height / 2;
  return board.shapes
    .filter((shape): shape is ZoneShape => shape.kind === "zone" && shape !== box && inside(shape, x, y))
    .sort((a, b) => a.width * a.height - b.width * b.height)[0];
}

/** What a zone carries when it is moved: the strokes and shapes drawn entirely inside it. */
export function carried(board: Board, zone: ZoneShape): string[] {
  const strokes = board.strokes.filter((stroke) => {
    const width = Math.max(...stroke.points.map((p) => p[0]));
    const height = Math.max(...stroke.points.map((p) => p[1]));
    return contains(zone, { x: stroke.x, y: stroke.y, width, height });
  });
  const shapes = board.shapes.filter(
    (shape): shape is Exclude<Shape, ArrowShape> =>
      shape.kind !== "arrow" && shape.id !== zone.id && contains(zone, rectOf(shape)),
  );
  return [...strokes, ...shapes].map((item) => item.id);
}

/** The box or zone under a point, the smallest first: where a note put down attaches (Q65). */
export function elementAt(board: Board, at: XY): BoxShape | ZoneShape | undefined {
  return board.shapes
    .filter(
      (shape): shape is BoxShape | ZoneShape =>
        (shape.kind === "box" || shape.kind === "zone") && inside(shape, at.x, at.y),
    )
    .sort((a, b) => (a.kind === b.kind ? a.width * a.height - b.width * b.height : a.kind === "box" ? -1 : 1))[0];
}

const isNumber = (value: unknown): value is number => typeof value === "number" && Number.isFinite(value);
const isText = (value: unknown, max: number): value is string => typeof value === "string" && length(value) <= max;
type Fields = Record<string, unknown>;

function isStroke(value: unknown): value is Stroke {
  if (typeof value !== "object" || value === null) return false;
  const { id, x, y, points, pen } = value as Fields;
  return (
    typeof id === "string" &&
    prefixOf(id) === "S" &&
    isNumber(x) &&
    isNumber(y) &&
    (pen === undefined || typeof pen === "boolean") &&
    Array.isArray(points) &&
    points.length > 0 &&
    points.length <= MAX_POINTS &&
    points.every((p) => Array.isArray(p) && p.length === 3 && p.every(isNumber))
  );
}

const isRect = (v: Fields) =>
  isNumber(v.x) && isNumber(v.y) && isNumber(v.width) && isNumber(v.height) && v.width > 0 && v.height > 0;

/** The shape read back, its fields checked one by one; references are checked by `parseBoard`. */
function shapeFrom(value: unknown): Shape | undefined {
  if (typeof value !== "object" || value === null) return undefined;
  const v = value as Fields;
  const prefix = typeof v.id === "string" ? prefixOf(v.id) : undefined;
  const id = v.id as string;
  switch (v.kind) {
    case "box":
      return prefix === "C" && isRect(v) && isText(v.name, MAX_NAME) && isText(v.role, MAX_TEXT)
        ? {
            kind: "box",
            id,
            x: v.x as number,
            y: v.y as number,
            width: v.width as number,
            height: v.height as number,
            name: v.name,
            role: v.role,
          }
        : undefined;
    case "zone":
      return prefix === "L" && isRect(v) && isText(v.name, MAX_NAME)
        ? {
            kind: "zone",
            id,
            x: v.x as number,
            y: v.y as number,
            width: v.width as number,
            height: v.height as number,
            name: v.name,
          }
        : undefined;
    case "arrow":
      return prefix === "E" &&
        typeof v.src === "string" &&
        typeof v.dst === "string" &&
        POLICIES.includes(v.policy as Policy)
        ? { kind: "arrow", id, src: v.src, dst: v.dst, policy: v.policy as Policy }
        : undefined;
    case "note":
      return prefix === "N" &&
        isNumber(v.x) &&
        isNumber(v.y) &&
        isText(v.text, MAX_TEXT) &&
        (v.attachedTo === null || typeof v.attachedTo === "string")
        ? { kind: "note", id, x: v.x, y: v.y, text: v.text, attachedTo: v.attachedTo }
        : undefined;
    case "budget": {
      if (
        prefix !== "N" ||
        !isNumber(v.x) ||
        !isNumber(v.y) ||
        !(v.attachedTo === null || typeof v.attachedTo === "string")
      )
        return undefined;
      const limits: Limits = {};
      for (const key of LIMITS) {
        const limit = v[key];
        if (limit === undefined) continue;
        if (!isNumber(limit) || !isLimit(limit)) return undefined;
        limits[key] = limit;
      }
      return { kind: "budget", id, x: v.x, y: v.y, attachedTo: v.attachedTo, ...limits };
    }
    default:
      return undefined;
  }
}

/**
 * The board a page kept (`getState`), or an empty one when it kept nothing readable. A board of
 * version 1 (strokes only, PLAN.md 7.1) is read as one of version 2.
 */
export function parseBoard(value: unknown): Board {
  if (typeof value !== "object" || value === null) return EMPTY_BOARD;
  const v = value as Fields;
  const strokes = v.strokes;
  const rawShapes = v.version === 1 ? [] : v.shapes;
  if ((v.version !== 1 && v.version !== 2) || !Array.isArray(strokes) || !Array.isArray(rawShapes)) return EMPTY_BOARD;
  if (strokes.length > MAX_STROKES || rawShapes.length > MAX_SHAPES || !strokes.every(isStroke)) return EMPTY_BOARD;
  const shapes = rawShapes.map(shapeFrom);
  if (!shapes.every((shape): shape is Shape => shape !== undefined)) return EMPTY_BOARD;
  const ids = [...strokes.map((s) => s.id), ...shapes.map((s) => s.id)];
  if (new Set(ids).size !== ids.length) return EMPTY_BOARD;
  // Every reference names a shape that can take it.
  const board: Board = { version: 2, strokes, shapes, next: { ...EMPTY_BOARD.next } };
  for (const shape of shapes) {
    if (shape.kind === "arrow" && !connectable(board, shape.src, shape.dst)) return EMPTY_BOARD;
    if ((shape.kind === "note" || shape.kind === "budget") && !attachableTo(board, shape.attachedTo))
      return EMPTY_BOARD;
  }
  const kept = v.version === 1 ? { S: v.next } : ((v.next as Fields | undefined) ?? {});
  const next = { ...EMPTY_BOARD.next };
  for (const prefix of PREFIXES) {
    const n = kept[prefix];
    if (isNumber(n) && Number.isSafeInteger(n) && n > 0) next[prefix] = n;
  }
  return { ...board, next: counted(next, ids) };
}
