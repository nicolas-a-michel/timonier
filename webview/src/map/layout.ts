/**
 * The layout of the map with elkjs (PLAN.md 4.7, 4.8): layered, left to right. Each container
 * (the map, an open group, an open box) is laid out on its own, from the inside out: its direct
 * children form a flat graph, and an edge between two deeper items counts between their
 * ancestors there.
 *
 * Stability (ADR 0021): with a previous layout, what did not change keeps its place exactly.
 * - A container whose children and their sizes are the same keeps their positions: an edge
 *   added or removed moves nothing, as React Flow draws the edges itself.
 * - A container whose children grew or shrank makes room as one would on a board: the columns
 *   after a wider item move right by as much, the items below a taller one in its column move
 *   down by as much. The edges keep going left to right, and closing undoes what opening did.
 * - Newcomers go below their siblings; what they overlap moves down.
 * - Only a container opened for the first time is laid out by ELK.
 *
 * The bundled ELK runs in the webview's thread: the CSP allows no worker.
 */
import ELK, { type ElkExtendedEdge, type ElkNode } from "elkjs/lib/elk.bundled.js";

import type { Pin } from "../protocol";
import type { MapContent, MapItem } from "./mapModel";

/** Sizes of what is not a container, in pixels: the CSS of the map gives them the same. */
export const BOX = { width: 208, height: 88 };
export const ROLE = { width: 240, header: 56, line: 20, lines: 5 };
/** Room inside a container for its header. */
export const CONTAINER_HEADER = 44;
/** Where the pins are, from the top: level with the title bar (the CSS gives these heights). */
export const PIN_Y = { box: 16, role: CONTAINER_HEADER / 2, frame: CONTAINER_HEADER / 2 };

export interface Placed {
  x: number;
  y: number;
  width: number;
  height: number;
}

export type Layout = Map<string, Placed>;

/** A layout and what it laid out: the previous one, for the next. */
export interface Laid {
  content: MapContent;
  placed: Layout;
}

/** The map itself, as a container: the parent of a pin at the top (ADR 0021). */
export const ROOT = "@root";

/** Boxes moved by hand (ADR 0021, « Boîtes déplacées »): where each stays, in its container. */
export type Pins = Readonly<Record<string, Pin>>;

/** The gap between two boxes of a column; the layers are farther apart. */
const NODE_GAP = 24;
const LAYER_GAP = 64;

const OPTIONS = {
  "elk.algorithm": "layered",
  "elk.direction": "RIGHT",
  "elk.spacing.nodeNode": String(NODE_GAP),
  "elk.layered.spacing.nodeNodeBetweenLayers": String(LAYER_GAP),
  "elk.spacing.edgeNode": "16",
};

const PADDING = {
  root: { top: 16, left: 16, bottom: 16, right: 16 },
  container: { top: CONTAINER_HEADER, left: 12, bottom: 12, right: 12 },
};

function elkPadding(p: { top: number; left: number; bottom: number; right: number }): string {
  return `[top=${p.top},left=${p.left},bottom=${p.bottom},right=${p.right}]`;
}

/** Passes that settle a first layout: ELK's interactive strategies, from its own result. */
const SETTLE = 2;

const INTERACTIVE = {
  "elk.layered.cycleBreaking.strategy": "INTERACTIVE",
  "elk.layered.layering.strategy": "INTERACTIVE",
  "elk.layered.crossingMinimization.strategy": "INTERACTIVE",
  "elk.layered.nodePlacement.strategy": "INTERACTIVE",
};

/** The height of a closed role group: its members, five at most, and a « more » line. */
export function roleHeight(members: number): number {
  const lines = Math.min(members, ROLE.lines) + (members > ROLE.lines ? 1 : 0);
  return ROLE.header + lines * ROLE.line + 12;
}

function parentOf(item: MapItem): string {
  return item.kind === "box" ? item.parent : ROOT;
}

/** What is not a container, at its fixed size. */
function leafSize(item: MapItem): { width: number; height: number } {
  return item.kind === "role" && !item.open
    ? { width: ROLE.width, height: roleHeight(item.members.length) }
    : { width: BOX.width, height: BOX.height };
}

/** The containers, their children, and the edges between children of each. */
function structure(content: MapContent) {
  const children = new Map<string, MapItem[]>();
  const byId = new Map<string, MapItem>();
  for (const item of content.items) {
    byId.set(item.id, item);
    const parent = parentOf(item);
    const list = children.get(parent);
    if (list) list.push(item);
    else children.set(parent, [item]);
  }
  const chains = new Map<string, string[]>();
  const chain = (id: string): string[] => {
    const known = chains.get(id);
    if (known) return known;
    const item = byId.get(id);
    const result = item ? [...chain(parentOf(item)), id] : [ROOT];
    chains.set(id, result);
    return result;
  };
  const projected = new Map<string, Set<string>>();
  for (const edge of content.edges) {
    const a = chain(edge.source);
    const b = chain(edge.target);
    let common = 0;
    while (common < a.length && common < b.length && a[common] === b[common]) common++;
    const container = a[common - 1];
    const from = a[common];
    const to = b[common];
    if (!container || !from || !to) continue;
    const edges = projected.get(container) ?? new Set();
    edges.add(`${from}\u0000${to}`);
    projected.set(container, edges);
  }
  const isContainer = (item: MapItem) => item.open && (children.get(item.id)?.length ?? 0) > 0;
  return { children, projected, isContainer };
}

const elk = new ELK();

type Structure = ReturnType<typeof structure>;
type Size = { width: number; height: number };

/** One container with ELK: its children at the given sizes, and positions as hints if given. */
async function elkContainer(
  container: string,
  inside: MapItem[],
  sizes: Map<string, Size>,
  projected: Structure["projected"],
  hints?: Layout,
): Promise<{ positions: Layout; size: Size }> {
  const nodes: ElkNode[] = inside.map((item) => {
    const size = sizes.get(item.id) ?? leafSize(item);
    const at = hints?.get(item.id);
    return { id: item.id, ...size, ...(at ? { x: at.x, y: at.y } : {}) };
  });
  const edges: ElkExtendedEdge[] = [...(projected.get(container) ?? [])].map((pair, i) => {
    const [source = "", target = ""] = pair.split("\u0000");
    return { id: `${container}#${i}`, sources: [source], targets: [target] };
  });
  const graph: ElkNode = {
    id: container,
    layoutOptions: {
      ...OPTIONS,
      ...(hints ? INTERACTIVE : {}),
      "elk.padding": elkPadding(paddingOf(container)),
    },
    children: nodes,
    edges,
  };
  const result = await elk.layout(graph);
  const positions: Layout = new Map();
  for (const node of result.children ?? []) {
    positions.set(node.id, { x: node.x ?? 0, y: node.y ?? 0, width: node.width ?? 0, height: node.height ?? 0 });
  }
  return { positions, size: extent(positions.values(), paddingOf(container)) };
}

function paddingOf(container: string) {
  return container === ROOT ? PADDING.root : PADDING.container;
}

/**
 * A container's size: what it holds, and its padding. Both layouts compute it so, then a size
 * changes only when the contents do.
 */
function extent(inside: Iterable<Placed>, padding: { right: number; bottom: number }): Size {
  let width = 0;
  let height = 0;
  for (const at of inside) {
    width = Math.max(width, at.x + at.width);
    height = Math.max(height, at.y + at.height);
  }
  return { width: width + padding.right, height: height + padding.bottom };
}

/** A fresh layout of every container, settled by interactive passes on its own result. */
async function fresh(content: MapContent, pins: Pins): Promise<Layout> {
  const shape = structure(content);
  const placed: Layout = new Map();
  const lay = async (container: string): Promise<Size> => {
    const inside = shape.children.get(container) ?? [];
    const sizes = new Map<string, Size>();
    for (const item of inside) if (shape.isContainer(item)) sizes.set(item.id, await lay(item.id));
    let { positions, size } = await elkContainer(container, inside, sizes, shape.projected);
    for (let i = 0; i < SETTLE; i++) {
      ({ positions, size } = await elkContainer(container, inside, sizes, shape.projected, positions));
    }
    if (!inside.some((item) => pins[item.id]?.parent === container)) {
      for (const [id, at] of positions) placed.set(id, at);
      return size;
    }
    const rects: Rect[] = [...positions].map(([id, at]) => ({ id, ...at }));
    sweep(rects, pin(rects, container, pins));
    for (const rect of rects) placed.set(rect.id, { x: rect.x, y: rect.y, width: rect.width, height: rect.height });
    return extent(rects, paddingOf(container));
  };
  await lay(ROOT);
  return placed;
}

/**
 * Puts the pinned items of `container` where they were left, inside its padding, and returns
 * them: they stay put, the others make room.
 */
function pin(rects: Rect[], container: string, pins: Pins): Set<string> {
  const padding = paddingOf(container);
  const fixed = new Set<string>();
  for (const rect of rects) {
    const at = pins[rect.id];
    if (!at || at.parent !== container) continue;
    rect.x = container === ROOT ? at.x : Math.max(at.x, padding.left);
    rect.y = container === ROOT ? at.y : Math.max(at.y, padding.top);
    fixed.add(rect.id);
  }
  return fixed;
}

interface Rect {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Whether two boxes come closer than half the gap of a column: an overlap to undo. */
function overlap(a: Rect, b: Rect): boolean {
  const margin = NODE_GAP / 2;
  return (
    a.x < b.x + b.width + margin &&
    b.x < a.x + a.width + margin &&
    a.y < b.y + b.height + margin &&
    b.y < a.y + a.height + margin
  );
}

/** Items side by side in columns: those whose spans along x overlap share one. */
function columnsOf(rects: Rect[]): Rect[][] {
  const columns: Rect[][] = [];
  let right = -Infinity;
  for (const rect of [...rects].sort((a, b) => a.x - b.x)) {
    const last = columns.at(-1);
    if (last && rect.x < right) {
      last.push(rect);
      right = Math.max(right, rect.x + rect.width);
    } else {
      columns.push([rect]);
      right = rect.x + rect.width;
    }
  }
  return columns;
}

/**
 * Newcomers go where their edges lead, at the bottom of a column: after the column of what calls
 * them, or else before the column of what they call, so that their edges go left to right. With
 * no edge to a sibling, the first column.
 */
function placeNewcomers(
  rects: Rect[],
  newcomers: { id: string; width: number; height: number }[],
  edges: Set<string>,
  padding: { left: number; top: number },
): void {
  const columns = columnsOf(rects);
  const columnOf = new Map<string, number>();
  columns.forEach((column, index) => {
    for (const rect of column) columnOf.set(rect.id, index);
  });
  const callers = new Map<string, string[]>();
  const callees = new Map<string, string[]>();
  for (const pair of edges) {
    const [from = "", to = ""] = pair.split("\u0000");
    callers.set(to, [...(callers.get(to) ?? []), from]);
    callees.set(from, [...(callees.get(from) ?? []), to]);
  }
  const at = (ids: string[] | undefined) =>
    (ids ?? []).map((id) => columnOf.get(id)).filter((c): c is number => c !== undefined);
  for (const newcomer of newcomers) {
    const before = at(callers.get(newcomer.id));
    const after = at(callees.get(newcomer.id));
    const index =
      before.length > 0 ? Math.max(...before) + 1 : after.length > 0 ? Math.max(Math.min(...after) - 1, 0) : 0;
    let column = columns[index];
    if (!column) {
      // A new column, right of the last one.
      const last = columns.at(-1) ?? [];
      const right = last.reduce((r, rect) => Math.max(r, rect.x + rect.width), padding.left - LAYER_GAP);
      column = [];
      columns.push(column);
      const rect = { ...newcomer, x: right + LAYER_GAP, y: padding.top };
      column.push(rect);
      rects.push(rect);
      columnOf.set(rect.id, columns.length - 1);
      continue;
    }
    const x = Math.min(...column.map((rect) => rect.x));
    const lowest = column.reduce((low, rect) => Math.max(low, rect.y + rect.height), padding.top - NODE_GAP);
    const rect = { ...newcomer, x, y: lowest + NODE_GAP };
    column.push(rect);
    rects.push(rect);
    columnOf.set(rect.id, index);
  }
}

/**
 * In order, each box moves down below whatever it overlaps among those already in place. The
 * `fixed` ones are in place from the start, and never move.
 */
function sweep(rects: Rect[], fixed: ReadonlySet<string> = new Set()): void {
  const settled: Rect[] = rects.filter((rect) => fixed.has(rect.id));
  for (const rect of rects) {
    if (fixed.has(rect.id)) continue;
    for (let moves = 0; moves <= settled.length; moves++) {
      const blocking = settled.find((other) => overlap(rect, other));
      if (!blocking) break;
      rect.y = blocking.y + blocking.height + NODE_GAP;
    }
    settled.push(rect);
  }
}

/** The layout from `previous`: see the module's comment. */
async function incremental(content: MapContent, previous: Laid, pins: Pins): Promise<Layout> {
  const shape = structure(content);
  const before = structure(previous.content);
  const previousParent = new Map<string, string>();
  for (const [parent, list] of before.children) for (const item of list) previousParent.set(item.id, parent);
  const placed: Layout = new Map();
  const lay = async (container: string): Promise<Size> => {
    const inside = shape.children.get(container) ?? [];
    const sizes = new Map<string, Size>();
    for (const item of inside) sizes.set(item.id, shape.isContainer(item) ? await lay(item.id) : leafSize(item));
    const known = inside.filter((item) => previousParent.get(item.id) === container && previous.placed.has(item.id));
    // Opened for the first time: nothing to keep.
    if (known.length === 0) {
      const { positions } = await elkContainer(container, inside, sizes, shape.projected);
      const rects: Rect[] = [...positions].map(([id, at]) => ({ id, ...at }));
      sweep(rects, pin(rects, container, pins));
      for (const rect of rects) placed.set(rect.id, { x: rect.x, y: rect.y, width: rect.width, height: rect.height });
      return extent(rects, paddingOf(container));
    }
    const padding = paddingOf(container);
    const sizeOf = (id: string, item?: MapItem) => sizes.get(id) ?? (item ? leafSize(item) : BOX);
    const kept: Rect[] = known.map((item) => {
      const at = previous.placed.get(item.id) ?? { x: padding.left, y: padding.top, ...leafSize(item) };
      return { id: item.id, x: at.x, y: at.y, width: at.width, height: at.height };
    });
    const rects: Rect[] = [];
    // What the columns before this one grew by, or shrank.
    let shift = 0;
    for (const column of columnsOf(kept)) {
      let right = -Infinity;
      let grown = -Infinity;
      for (const was of column) {
        const size = sizeOf(was.id);
        let dy = 0;
        for (const other of column) {
          if (other !== was && other.y + other.height <= was.y) dy += sizeOf(other.id).height - other.height;
        }
        rects.push({ id: was.id, x: was.x + shift, y: was.y + dy, ...size });
        right = Math.max(right, was.x + was.width);
        grown = Math.max(grown, was.x + size.width);
      }
      shift += grown - right;
    }
    rects.sort((a, b) => a.y - b.y || a.x - b.x);
    const knownIds = new Set(known.map((item) => item.id));
    placeNewcomers(
      rects,
      inside.filter((item) => !knownIds.has(item.id)).map((item) => ({ id: item.id, ...sizeOf(item.id, item) })),
      shape.projected.get(container) ?? new Set(),
      padding,
    );
    sweep(rects, pin(rects, container, pins));
    for (const rect of rects) placed.set(rect.id, { x: rect.x, y: rect.y, width: rect.width, height: rect.height });
    return extent(rects, padding);
  };
  await lay(ROOT);
  return placed;
}

/**
 * Positions relative to each item's container, as React Flow wants them for `parentId`: a fresh,
 * settled layout; or, with `previous`, the incremental one that keeps what did not change. The
 * boxes moved by hand stay where `pins` says.
 */
export async function layout(content: MapContent, previous?: Laid, pins: Pins = {}): Promise<Layout> {
  return previous ? incremental(content, previous, pins) : fresh(content, pins);
}
