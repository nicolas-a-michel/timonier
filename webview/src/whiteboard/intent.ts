/**
 * The whiteboard as `.timonier/intent.json` holds it (ADR 0039): the meaning of the shapes
 * (layers, components with their layer, edges, notes, budgets) apart from where they are drawn
 * (`sketch`). The board becomes an intent after each gesture (Q66), and an intent a board when
 * the page opens or the file changes.
 */
import type { Locale } from "../i18n";
import type { Intent, IntentBudget, Place } from "../protocol";
import { EMPTY_BOARD, layerOf, parseBoard, type Board, type BudgetShape, type Shape, type Stroke } from "./board";
import { whiteboardMessages } from "./messages";

/** An id's number when it is numbered (`C-12`); named ids (`C-db-2`, Q69) have none. */
const numberOf = (id: string) => {
  const digits = /^[A-Z]-([1-9]\d*)$/.exec(id)?.[1];
  const n = Number(digits);
  return digits !== undefined && Number.isSafeInteger(n) ? n : Number.MAX_SAFE_INTEGER + 1;
};
/** The daemon's order (ADR 0039): by number, the named ids after, by name. */
export const byId = <T extends { id: string }>(items: T[]) =>
  [...items].sort((a, b) => numberOf(a.id) - numberOf(b.id) || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));

/** The limits of a budget, as the file names them; counts are whole. */
function limitsOf(shape: BudgetShape): Omit<IntentBudget, "id" | "attached_to"> {
  return {
    ...(shape.p50Ms === undefined ? {} : { p50_ms: shape.p50Ms }),
    ...(shape.p95Ms === undefined ? {} : { p95_ms: shape.p95Ms }),
    ...(shape.memoryMb === undefined ? {} : { memory_mb: shape.memoryMb }),
    ...(shape.allocations === undefined ? {} : { allocations: Math.round(shape.allocations) }),
    ...(shape.calls === undefined ? {} : { calls: Math.round(shape.calls) }),
  };
}

/** What the board means and where it is drawn, in the file's one order. */
export function intentOf(board: Board): Intent {
  const of = <K extends Shape["kind"]>(kind: K) =>
    byId(board.shapes.filter((s): s is Extract<Shape, { kind: K }> => s.kind === kind));
  const shapes: Record<string, Place> = {};
  for (const shape of byId(board.shapes)) {
    if (shape.kind === "box" || shape.kind === "zone") {
      shapes[shape.id] = { x: shape.x, y: shape.y, width: shape.width, height: shape.height };
    } else if (shape.kind === "note" || shape.kind === "budget") {
      shapes[shape.id] = { x: shape.x, y: shape.y };
    }
  }
  return {
    version: 1,
    layers: of("zone").map(({ id, name }) => ({ id, name })),
    components: of("box").map((box) => ({
      id: box.id,
      name: box.name,
      layer: layerOf(board, box)?.id ?? null,
      role: box.role,
    })),
    edges: of("arrow").map(({ id, src, dst, policy }) => ({ id, src, dst, kind: "depends_on", policy })),
    notes: of("note").map(({ id, text, attachedTo }) => ({ id, text, attached_to: attachedTo })),
    budgets: of("budget").map((budget) => ({ id: budget.id, attached_to: budget.attachedTo, ...limitsOf(budget) })),
    sketch: {
      shapes,
      strokes: board.strokes.map(({ id, x, y, points, pen }) => ({ id, x, y, points, ...(pen ? { pen: true } : {}) })),
      next: { ...board.next },
    },
  };
}

/**
 * The board an intent draws, and why it cannot when it cannot: then the page says so and waits,
 * rather than show an empty board its next gesture would write over the file (review of
 * Phase 7, I1). A shape the sketch does not place goes to the top-left corner, at its default
 * size.
 */
export function boardOf(intent: Intent | null, locale: Locale = "en"): { board: Board; problem: string | null } {
  if (!intent) return { board: EMPTY_BOARD, problem: null };
  const board = drawn(intent);
  const holds =
    intent.layers.length + intent.components.length + intent.edges.length + intent.notes.length + intent.budgets.length;
  const empty = board.shapes.length === 0 && board.strokes.length === 0;
  return empty && (holds > 0 || intent.sketch.strokes.length > 0)
    ? { board: EMPTY_BOARD, problem: whiteboardMessages[locale].cannotDraw }
    : { board, problem: null };
}

function drawn(intent: Intent): Board {
  const at = (id: string) => intent.sketch.shapes[id] ?? { x: 0, y: 0 };
  const shapes: unknown[] = [
    ...intent.layers.map(({ id, name }) => {
      const place = at(id);
      return { kind: "zone", id, name, x: place.x, y: place.y, width: place.width ?? 480, height: place.height ?? 320 };
    }),
    ...intent.components.map(({ id, name, role }) => {
      const place = at(id);
      return {
        kind: "box",
        id,
        name,
        role,
        x: place.x,
        y: place.y,
        width: place.width ?? 180,
        height: place.height ?? 80,
      };
    }),
    ...intent.edges.map(({ id, src, dst, policy }) => ({ kind: "arrow", id, src, dst, policy })),
    ...intent.notes.map(({ id, text, attached_to }) => ({
      kind: "note",
      id,
      text,
      attachedTo: attached_to,
      ...at(id),
    })),
    ...intent.budgets.map((budget) => ({
      kind: "budget",
      id: budget.id,
      attachedTo: budget.attached_to,
      ...at(budget.id),
      ...(budget.p50_ms === undefined ? {} : { p50Ms: budget.p50_ms }),
      ...(budget.p95_ms === undefined ? {} : { p95Ms: budget.p95_ms }),
      ...(budget.memory_mb === undefined ? {} : { memoryMb: budget.memory_mb }),
      ...(budget.allocations === undefined ? {} : { allocations: budget.allocations }),
      ...(budget.calls === undefined ? {} : { calls: budget.calls }),
    })),
  ];
  // The sketch's places of notes carry no size: keep only what a note holds.
  const cleaned = shapes.map((shape) => {
    const s = shape as Record<string, unknown>;
    if (s.kind === "note" || s.kind === "budget") {
      delete s.width;
      delete s.height;
    }
    return s;
  });
  const strokes: Stroke[] = intent.sketch.strokes.map(({ id, x, y, points, pen }) => ({
    id,
    x,
    y,
    points,
    ...(pen ? { pen: true } : {}),
  }));
  return parseBoard({ version: 2, strokes, shapes: cleaned, next: intent.sketch.next });
}
