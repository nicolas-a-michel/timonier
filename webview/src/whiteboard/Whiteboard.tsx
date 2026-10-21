/**
 * The whiteboard (PLAN.md 7.1, 7.2; ADR 0036 to 0038): React Flow holds the board, its pan and
 * zoom. Each freehand stroke and each typed shape is one of its nodes, each arrow one of its
 * edges, so they are selected, moved, deleted and named for screen readers like the boxes of
 * the map. The pen, the eraser and the shape tools take the pointer over the board; the
 * properties panel sets what a shape says; every step is undone and redone from the history
 * (`board.ts`).
 */
import "@xyflow/react/dist/base.css";

import {
  Background,
  BackgroundVariant,
  Panel,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
  useViewport,
  type EdgeChange,
  type NodeChange,
  type ResizeParams,
  type XYPosition,
} from "@xyflow/react";
import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";

import { Toolbar } from "../design/components";
import type { Locale } from "../i18n";
import type { WhiteboardCommand, WhiteboardShape, WhiteboardTool } from "../protocol";
import {
  LIMITS,
  NOTE_HEIGHT,
  NOTE_WIDTH,
  add,
  carried,
  connect,
  draw,
  elementAt,
  erase,
  history,
  layerOf,
  move,
  redo,
  shapeOf,
  strokeOf,
  undo,
  update,
  type Board,
  type History,
  type Point,
  type Rect,
  type Shape,
  type XY,
} from "./board";
import { inkBox, inkPath, INK_PAD } from "./ink";
import { whiteboardMessages, type WhiteboardMessages } from "./messages";
import { Properties } from "./Properties";
import { edgeTypes, nodeTypes, type WhiteboardEdge, type WhiteboardNode } from "./shapes";

/** Where the palette's commands come in; those that came before the board are kept. */
export interface WhiteboardCommands {
  current: ((command: WhiteboardCommand) => void) | null;
  pending: WhiteboardCommand[];
}

export interface WhiteboardProps {
  locale: Locale;
  /** The board as the page kept it. */
  board: Board;
  /** After each step: the page keeps the board. */
  onChange?: (board: Board) => void;
  commandsRef?: WhiteboardCommands;
  /** Called once the board is on screen. */
  onReady?: () => void;
  /** Whether the last gestures reached `intent.json` (Q66); `null` before the first one. */
  saved?: Saved | null;
}

export interface Saved {
  saved: boolean;
  detail: string | null;
}

const TOOLS: readonly WhiteboardTool[] = ["select", "pen", "eraser", "box", "zone", "arrow", "note", "budget"];
/** The key that picks each tool. */
const KEYS: Record<string, WhiteboardTool> = {
  v: "select",
  p: "pen",
  e: "eraser",
  b: "box",
  z: "zone",
  a: "arrow",
  n: "note",
  m: "budget",
};
const SHAPE_TOOLS: readonly string[] = ["box", "zone", "note", "budget"];
/** Which shapes come over which: zones under the ink, boxes over it, notes on top. */
const LAYER = { zone: 0, stroke: 1, box: 2, note: 3, budget: 3 } as const;

/** Whether a key goes to a field the developer types in, not to the board; a radio or a button does not keep them. */
function typing(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target instanceof HTMLInputElement) return !["radio", "checkbox", "button"].includes(target.type);
  return target.isContentEditable || /^(TEXTAREA|SELECT)$/.test(target.tagName);
}

/** The pointer's pressure: a pen's own, otherwise none to say (the speed stands for it). */
const pressureOf = (event: PointerEvent | ReactPointerEvent) => (event.pointerType === "pen" ? event.pressure : 0.5);

/** How far around the pointer the eraser reaches, in pixels of the screen. */
const ERASER_REACH = 6;
const AROUND = [
  [0, 0],
  [ERASER_REACH, 0],
  [-ERASER_REACH, 0],
  [0, ERASER_REACH],
  [0, -ERASER_REACH],
] as const;

/** The strokes the eraser touches at a point of the screen: their ink only, not their boxes (whiteboard.css). */
function strokesAt(x: number, y: number): string[] {
  return AROUND.flatMap(([dx, dy]) =>
    document
      .elementsFromPoint(x + dx, y + dy)
      .flatMap((element) => element.closest<HTMLElement>(".react-flow__node-stroke")?.dataset.id ?? []),
  );
}

/** The name each shape goes by in the panel's lists and the screen readers' words. */
function namesOf(board: Board, t: WhiteboardMessages): Map<string, string> {
  const names = new Map<string, string>();
  for (const shape of board.shapes) {
    if (shape.kind === "box") names.set(shape.id, shape.name || `${t.unnamedBox} ${shape.id}`);
    if (shape.kind === "zone") names.set(shape.id, shape.name || `${t.unnamedZone} ${shape.id}`);
  }
  for (const shape of board.shapes) {
    if (shape.kind === "arrow") names.set(shape.id, `${names.get(shape.src)} → ${names.get(shape.dst)}`);
    if (shape.kind === "note") names.set(shape.id, `${t.kind.note} ${shape.id}`);
    if (shape.kind === "budget") names.set(shape.id, `${t.kind.budget} ${shape.id}`);
  }
  return names;
}

/** The selection once React Flow's changes are applied. */
function reselect(current: ReadonlySet<string>, changes: { id: string; selected: boolean }[]): Set<string> {
  const next = new Set(current);
  for (const change of changes) {
    if (change.selected) next.add(change.id);
    else next.delete(change.id);
  }
  return next;
}

function Canvas({ locale, board, onChange, commandsRef, onReady, saved = null }: WhiteboardProps) {
  const t = whiteboardMessages[locale];
  const flow = useReactFlow<WhiteboardNode, WhiteboardEdge>();
  const viewport = useViewport();
  const [state, setState] = useState<History>(() => history(board));
  const [tool, setTool] = useState<WhiteboardTool>("pen");
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  /** The corners of what is being dragged or resized, before it ends: as React Flow places nodes. */
  const [moving, setMoving] = useState<ReadonlyMap<string, XYPosition>>(new Map());
  const [sizes, setSizes] = useState<ReadonlyMap<string, { width: number; height: number }>>(new Map());
  const [measured, setMeasured] = useState<ReadonlyMap<string, { width?: number; height?: number }>>(new Map());
  /** The stroke being drawn, in the board's coordinates. */
  const [live, setLive] = useState<{ points: Point[]; pen: boolean } | null>(null);
  const [erasing, setErasing] = useState<ReadonlySet<string>>(new Set());
  /** The first end of the arrow being drawn. */
  const [arrowFrom, setArrowFrom] = useState<string | null>(null);
  /** A shape just added: its first field takes the focus. */
  const [fresh, setFresh] = useState<string | null>(null);
  const surface = useRef<HTMLDivElement>(null);
  const last = useRef<{ x: number; y: number } | null>(null);
  const resizing = useRef(false);

  // The page keeps every step, so that a reload of the panel finds the board as it was left.
  const kept = useRef(state.board);
  useEffect(() => {
    if (state.board === kept.current) return;
    kept.current = state.board;
    onChange?.(state.board);
  }, [state.board, onChange]);

  const shapes = useMemo(() => new Map(state.board.shapes.map((shape) => [shape.id, shape])), [state.board]);
  const names = useMemo(() => namesOf(state.board, t), [state.board, t]);
  // What an undo took away is no longer selected.
  const present = useMemo(() => {
    const ids = new Set([...state.board.strokes.map((s) => s.id), ...shapes.keys()]);
    return new Set([...selected].filter((id) => ids.has(id)));
  }, [selected, state.board, shapes]);

  /** Where a shape stands now, a drag or a resize included, in the board's coordinates. */
  const rectNow = (shape: Exclude<Shape, { kind: "arrow" }>): Rect => {
    const at = moving.get(shape.id) ?? shape;
    if (shape.kind === "box" || shape.kind === "zone") {
      const size = sizes.get(shape.id) ?? shape;
      return { x: at.x, y: at.y, width: size.width, height: size.height };
    }
    return { x: at.x, y: at.y, width: NOTE_WIDTH, height: measured.get(shape.id)?.height ?? NOTE_HEIGHT };
  };
  const rectOfId = (id: string): Rect | undefined => {
    const shape = shapes.get(id);
    return shape && shape.kind !== "arrow" ? rectNow(shape) : undefined;
  };

  const selecting = tool === "select";
  const onResized = (id: string, rect: ResizeParams) => {
    resizing.current = false;
    const shape = shapes.get(id);
    if (shape && (shape.kind === "box" || shape.kind === "zone")) setState((s) => update(s, { ...shape, ...rect }));
    setSizes((current) => new Map([...current].filter(([key]) => key !== id)));
    setMoving((current) => new Map([...current].filter(([key]) => key !== id)));
  };

  const nodes = useMemo((): WhiteboardNode[] => {
    const common = (id: string) => ({
      placeholder: "",
      pending: arrowFrom === id,
      resizable: selecting && present.has(id) && present.size === 1,
      onResized,
    });
    const strokes = state.board.strokes.map((stroke, index): WhiteboardNode => {
      const box = inkBox(stroke);
      return {
        id: stroke.id,
        type: "stroke",
        position: moving.get(stroke.id) ?? { x: box.x, y: box.y },
        width: box.width,
        height: box.height,
        measured: measured.get(stroke.id),
        data: { stroke, erasing: erasing.has(stroke.id) },
        selected: present.has(stroke.id),
        connectable: false,
        zIndex: LAYER.stroke,
        ariaLabel: t.stroke(index + 1),
      };
    });
    const typed = state.board.shapes.flatMap((shape): WhiteboardNode[] => {
      if (shape.kind === "arrow") return [];
      const rect = rectNow(shape);
      const base = {
        id: shape.id,
        position: { x: rect.x, y: rect.y },
        measured: measured.get(shape.id),
        selected: present.has(shape.id),
        connectable: false,
        zIndex: LAYER[shape.kind],
      };
      const on = "attachedTo" in shape && shape.attachedTo ? (names.get(shape.attachedTo) ?? null) : null;
      switch (shape.kind) {
        case "box": {
          const layer = layerOf(state.board, rect);
          return [
            {
              ...base,
              type: "box",
              width: rect.width,
              height: rect.height,
              data: { ...common(shape.id), shape, placeholder: t.unnamedBox },
              ariaLabel: t.boxLabel(
                names.get(shape.id) ?? "",
                layer ? (names.get(layer.id) ?? null) : null,
                shape.role,
              ),
            },
          ];
        }
        case "zone": {
          const boxes = state.board.shapes.filter((s) => s.kind === "box" && layerOf(state.board, s)?.id === shape.id);
          return [
            {
              ...base,
              type: "zone",
              width: rect.width,
              height: rect.height,
              data: { ...common(shape.id), shape, placeholder: t.unnamedZone },
              ariaLabel: t.zoneLabel(names.get(shape.id) ?? "", boxes.length),
            },
          ];
        }
        case "note":
          return [
            {
              ...base,
              type: "note",
              width: NOTE_WIDTH,
              data: { ...common(shape.id), shape, on: on && t.on(on), placeholder: t.emptyNote },
              ariaLabel: t.noteLabel(shape.text || t.emptyNote, on),
            },
          ];
        case "budget": {
          const lines = LIMITS.flatMap((key) => {
            const value = shape[key];
            return value === undefined ? [] : [t.limitLine(key, value)];
          });
          return [
            {
              ...base,
              type: "budget",
              width: NOTE_WIDTH,
              data: {
                ...common(shape.id),
                shape,
                on: on && t.on(on),
                lines,
                title: t.kind.budget,
                placeholder: t.noLimit,
              },
              ariaLabel: t.budgetLabel(lines.length > 0 ? lines.join(", ") : t.noLimit, on),
            },
          ];
        }
      }
    });
    const order = (node: WhiteboardNode) => node.zIndex ?? 0;
    return [...strokes, ...typed].sort((a, b) => order(a) - order(b));
    // `onResized` and `rectNow` read the same state as the dependencies.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.board, moving, sizes, measured, present, erasing, arrowFrom, selecting, names, t]);

  const edges = useMemo((): WhiteboardEdge[] => {
    const lines: WhiteboardEdge[] = [];
    for (const shape of state.board.shapes) {
      if (shape.kind === "arrow") {
        const from = rectOfId(shape.src);
        const to = rectOfId(shape.dst);
        if (!from || !to) continue;
        lines.push({
          id: shape.id,
          source: shape.src,
          target: shape.dst,
          type: "arrow",
          data: {
            policy: shape.policy,
            from,
            to,
            mark: shape.policy === "allowed" ? null : t.policyMark[shape.policy],
          },
          selected: present.has(shape.id),
          selectable: selecting,
          focusable: true,
          ariaLabel: t.arrowLabel(shape.policy, names.get(shape.src) ?? "", names.get(shape.dst) ?? ""),
        });
      } else if ((shape.kind === "note" || shape.kind === "budget") && shape.attachedTo) {
        // A note on an arrow says so in its words; on a box or a zone, a dotted line joins them.
        const target = shapes.get(shape.attachedTo);
        if (!target || target.kind === "arrow") continue;
        const from = rectOfId(shape.id);
        const to = rectOfId(target.id);
        if (!from || !to) continue;
        lines.push({
          id: `attach:${shape.id}`,
          source: shape.id,
          target: target.id,
          type: "attach",
          data: { from, to },
          selectable: false,
          focusable: false,
        });
      }
    }
    return lines;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.board, moving, sizes, measured, present, selecting, names, shapes, t]);

  /** From where React Flow puts a node to where the board keeps its element. */
  const boardPosition = (id: string, at: XYPosition): XY =>
    shapes.has(id) ? at : { x: at.x + INK_PAD, y: at.y + INK_PAD };
  const nodePosition = (id: string): XYPosition | undefined => {
    const shape = shapes.get(id);
    if (shape && shape.kind !== "arrow") return { x: shape.x, y: shape.y };
    const stroke = state.board.strokes.find((s) => s.id === id);
    return stroke ? { x: stroke.x - INK_PAD, y: stroke.y - INK_PAD } : undefined;
  };

  const onNodesChange = (changes: NodeChange<WhiteboardNode>[]) => {
    let nextMoving: Map<string, XYPosition> | undefined;
    const chosen: { id: string; selected: boolean }[] = [];
    let nextMeasured: Map<string, { width?: number; height?: number }> | undefined;
    let nextSizes: Map<string, { width: number; height: number }> | undefined;
    const done = new Map<string, XY>();
    // A resize moves its node too: those moves are not a step until the resize ends.
    if (changes.some((change) => change.type === "dimensions" && change.resizing)) resizing.current = true;
    for (const change of changes) {
      if (change.type === "dimensions" && change.dimensions) {
        if (change.resizing) {
          nextSizes ??= new Map(sizes);
          nextSizes.set(change.id, change.dimensions);
        } else {
          nextMeasured ??= new Map(measured);
          nextMeasured.set(change.id, change.dimensions);
        }
      } else if (change.type === "position" && change.position) {
        nextMoving ??= new Map(moving);
        const shape = shapes.get(change.id);
        // A zone carries what is drawn inside it (Q62): the same move, for each.
        const along = shape?.kind === "zone" && !resizing.current ? carried(state.board, shape) : [];
        const dx = change.position.x - (shape && shape.kind !== "arrow" ? shape.x : 0);
        const dy = change.position.y - (shape && shape.kind !== "arrow" ? shape.y : 0);
        const shifted = along.flatMap((id) => {
          const at = nodePosition(id);
          return at ? [[id, { x: at.x + dx, y: at.y + dy }] as const] : [];
        });
        if (change.dragging || resizing.current) {
          nextMoving.set(change.id, change.position);
          for (const [id, at] of shifted) nextMoving.set(id, at);
        } else {
          // A drag's end, or an arrow key: a step of the history.
          nextMoving.delete(change.id);
          done.set(change.id, boardPosition(change.id, change.position));
          for (const [id, at] of shifted) {
            nextMoving.delete(id);
            if (!done.has(id)) done.set(id, boardPosition(id, at));
          }
        }
      } else if (change.type === "select") {
        chosen.push(change);
      }
    }
    if (nextMoving) setMoving(nextMoving);
    if (chosen.length > 0) setSelected((current) => reselect(current, chosen));
    if (nextMeasured) setMeasured(nextMeasured);
    if (nextSizes) setSizes(nextSizes);
    if (done.size > 0) setState((current) => move(current, done));
  };

  // Nodes and edges change in two calls for one click: each applies to what the other left.
  const onEdgesChange = (changes: EdgeChange<WhiteboardEdge>[]) => {
    const chosen = changes.flatMap((change) => (change.type === "select" ? [change] : []));
    if (chosen.length > 0) setSelected((current) => reselect(current, chosen));
  };

  const remove = (ids: Iterable<string>) => {
    const gone = [...ids].filter((id) => !id.startsWith("attach:"));
    setState((current) => erase(current, gone));
    setSelected((current) => new Set([...current].filter((id) => !gone.includes(id))));
  };

  const pickTool = (next: WhiteboardTool) => {
    setTool(next);
    setArrowFrom(null);
    // Drawing and erasing: what was selected no longer is. A note keeps it: it attaches there (Q65).
    if (next === "pen" || next === "eraser") setSelected(new Set());
  };

  /** The one shape selected, if one is. */
  const single = present.size === 1 ? shapes.get([...present][0] ?? "") : undefined;

  /** Selects what was just added, its first field focused. */
  const chose = (next: History, id: string) => {
    if (next === state) return;
    setState(next);
    setSelected(new Set([id]));
    setFresh(id);
  };

  /** Adds a shape and selects it; a note goes on what is selected, or on what it is put down on (Q65). */
  const place = (kind: WhiteboardShape, at: XY, beside?: Rect) => {
    let shape = shapeOf(state.board, kind, at);
    if (shape.kind === "box" || shape.kind === "zone") {
      shape = { ...shape, x: at.x - shape.width / 2, y: at.y - shape.height / 2 };
    }
    if (shape.kind === "note" || shape.kind === "budget") {
      const target = single && single.kind !== "note" && single.kind !== "budget" ? single : elementAt(state.board, at);
      const rect = target && target.kind !== "arrow" ? rectNow(target) : undefined;
      shape = {
        ...shape,
        attachedTo: target?.id ?? null,
        ...(beside && rect ? { x: rect.x + rect.width + 24, y: rect.y } : {}),
      };
    }
    chose(add(state, shape), shape.id);
    setTool("select");
  };

  /** An arrow between two boxes or zones, selected. */
  const link = (src: string, dst: string) => {
    const next = connect(state, src, dst);
    chose(next, next.board.shapes.at(-1)?.id ?? "");
  };

  /** Adds a shape where the developer looks: the keyboard's and the palette's way. */
  const addAtCenter = (kind: WhiteboardShape) => {
    const rect = surface.current?.getBoundingClientRect();
    const center = flow.screenToFlowPosition({
      x: (rect?.left ?? 0) + (rect?.width ?? 0) / 2,
      y: (rect?.top ?? 0) + (rect?.height ?? 0) / 2,
    });
    const beside =
      (kind === "note" || kind === "budget") && single && single.kind !== "arrow" ? rectNow(single) : undefined;
    place(kind, center, beside);
  };

  const run = (command: WhiteboardCommand) => {
    switch (command.name) {
      case "undo":
        setState(undo);
        return;
      case "redo":
        setState(redo);
        return;
      case "tool":
        pickTool(command.tool);
        return;
      case "add":
        addAtCenter(command.shape);
        return;
      case "deleteSelection":
        remove(present);
        return;
      case "fitView":
        void flow.fitView({ padding: 0.2, duration: 0, maxZoom: 1 });
        return;
      case "zoomIn":
        void flow.zoomIn({ duration: 0 });
        return;
      case "zoomOut":
        void flow.zoomOut({ duration: 0 });
        return;
    }
  };
  // The palette's commands and the keys see the latest state: `run` is taken after each render.
  const runRef = useRef(run);
  useLayoutEffect(() => {
    runRef.current = run;
  });
  useEffect(() => {
    if (!commandsRef) return;
    commandsRef.current = (command) => runRef.current(command);
    for (const command of commandsRef.pending.splice(0)) runRef.current(command);
    return () => {
      commandsRef.current = null;
    };
  }, [commandsRef]);

  // The keys of the board, wherever the focus is on the page, but not in a field.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (typing(event.target)) return;
      const key = event.key.toLowerCase();
      const command = event.ctrlKey || event.metaKey;
      let next: WhiteboardCommand | undefined;
      if (command && !event.altKey && key === "z") next = { name: event.shiftKey ? "redo" : "undo" };
      else if (command && !event.altKey && key === "y") next = { name: "redo" };
      else if (!command && !event.altKey && !event.shiftKey) {
        const picked = KEYS[key];
        if (picked) next = { name: "tool", tool: picked };
        else if (key === "escape") next = { name: "tool", tool: "select" };
        else if (key === "delete" || key === "backspace") next = { name: "deleteSelection" };
      }
      if (!next) return;
      event.preventDefault();
      if (key === "escape") setSelected(new Set());
      runRef.current(next);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const announced = useRef(false);
  useEffect(() => {
    if (announced.current) return;
    announced.current = true;
    onReady?.();
  }, [onReady]);

  const onBoard = (event: { clientX: number; clientY: number }, pressure: number): Point => {
    const at = flow.screenToFlowPosition({ x: event.clientX, y: event.clientY });
    return [at.x, at.y, pressure];
  };

  /** The strokes the eraser met between its last point and this one, every few pixels. */
  const sweep = (x: number, y: number) => {
    const from = last.current ?? { x, y };
    const steps = Math.max(1, Math.ceil(Math.hypot(x - from.x, y - from.y) / 3));
    const met: string[] = [];
    for (let i = 1; i <= steps; i++) {
      met.push(...strokesAt(from.x + ((x - from.x) * i) / steps, from.y + ((y - from.y) * i) / steps));
    }
    last.current = { x, y };
    if (met.length > 0) setErasing((current) => new Set([...current, ...met]));
  };

  const onPointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (selecting || event.button !== 0) return;
    // The toolbar and the other panels keep their clicks.
    if (!(event.target instanceof Element) || event.target.closest(".react-flow__panel")) return;
    if (!event.target.closest(".react-flow__pane, .react-flow__node")) return;
    event.preventDefault();
    // The shapes and the arrows come on the click, once React Flow has had it (it clears the
    // selection on a click of the board).
    if (tool !== "pen" && tool !== "eraser") return;
    event.currentTarget.setPointerCapture(event.pointerId);
    if (tool === "pen") {
      const pressure = pressureOf(event);
      setLive({ points: [onBoard(event, pressure)], pen: event.pointerType === "pen" });
    } else {
      last.current = null;
      setErasing(new Set());
      sweep(event.clientX, event.clientY);
    }
  };

  const onClick = (event: ReactMouseEvent<HTMLDivElement>) => {
    if (!SHAPE_TOOLS.includes(tool) && tool !== "arrow") return;
    if (!(event.target instanceof Element) || event.target.closest(".react-flow__panel")) return;
    if (!event.target.closest(".react-flow__pane, .react-flow__node")) return;
    const at = flow.screenToFlowPosition({ x: event.clientX, y: event.clientY });
    if (tool !== "arrow") {
      place(tool as WhiteboardShape, at);
      return;
    }
    const id = event.target.closest<HTMLElement>(".react-flow__node")?.dataset.id;
    const end = id ? shapes.get(id) : undefined;
    if (!end || (end.kind !== "box" && end.kind !== "zone")) return;
    if (!arrowFrom) {
      setArrowFrom(end.id);
      return;
    }
    link(arrowFrom, end.id);
    setArrowFrom(null);
    setTool("select");
  };

  const onPointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (!event.currentTarget.hasPointerCapture(event.pointerId)) return;
    // The points the browser merged into this event too, for a smooth ink.
    const events = event.nativeEvent.getCoalescedEvents?.() ?? [];
    const all = events.length > 0 ? events : [event.nativeEvent];
    if (tool === "pen") {
      setLive((current) =>
        current ? { ...current, points: [...current.points, ...all.map((e) => onBoard(e, pressureOf(e)))] } : current,
      );
    } else if (tool === "eraser") {
      for (const e of all) sweep(e.clientX, e.clientY);
    }
  };

  const onPointerUp = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (!event.currentTarget.hasPointerCapture(event.pointerId)) return;
    event.currentTarget.releasePointerCapture(event.pointerId);
    if (tool === "pen" && live) {
      setState((current) => {
        const stroke = strokeOf(current.board, live.points, live.pen);
        return stroke ? draw(current, stroke) : current;
      });
      setLive(null);
    } else if (tool === "eraser") {
      const met = erasing;
      setState((current) => erase(current, met));
      setErasing(new Set());
      last.current = null;
    }
  };

  const onPointerCancel = () => {
    setLive(null);
    setErasing(new Set());
    last.current = null;
  };

  const ends = useMemo(
    () =>
      state.board.shapes.filter((s) => s.kind !== "note" && s.kind !== "budget").map(({ id, kind }) => ({ id, kind })),
    [state.board],
  );
  const empty = state.board.strokes.length === 0 && state.board.shapes.length === 0 && !live;
  const arrowHint = tool === "arrow" ? (arrowFrom ? t.arrowTo(names.get(arrowFrom) ?? arrowFrom) : t.arrowFrom) : null;
  return (
    <div
      ref={surface}
      className="tim-whiteboard"
      data-tool={tool}
      onPointerDown={onPointerDown}
      onClick={onClick}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerCancel}
    >
      <ReactFlow<WhiteboardNode, WhiteboardEdge>
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        aria-label={t.label}
        // Selection only: the other tools take the pointer; the wheel still zooms.
        panOnDrag={selecting}
        nodesDraggable={selecting}
        elementsSelectable={selecting}
        nodesFocusable
        edgesFocusable
        nodesConnectable={false}
        elevateNodesOnSelect={false}
        // Delete and Backspace are the board's keys, as one step with what goes along (`erase`).
        deleteKeyCode={null}
        nodeDragThreshold={2}
        zoomOnDoubleClick={false}
        minZoom={0.1}
        maxZoom={4}
        onlyRenderVisibleElements
        ariaLabelConfig={{
          "node.a11yDescription.default": t.strokeHint,
          "node.a11yDescription.keyboardDisabled": t.strokeHint,
        }}
      >
        {/* The board of the map: the same faint lines (one visual language). */}
        <Background id="minor" variant={BackgroundVariant.Lines} gap={16} color="var(--tim-grid-minor)" />
        <Background id="major" variant={BackgroundVariant.Lines} gap={128} color="var(--tim-grid-major)" />
        <Panel position="top-left">
          <Toolbar
            label={t.tools}
            items={[
              ...TOOLS.map((id) => ({
                id,
                label: t.tool[id],
                pressed: tool === id,
                // From the keyboard, a shape comes at once, in the middle of the view.
                onSelect: ({ keyboard }: { keyboard: boolean }) =>
                  keyboard && SHAPE_TOOLS.includes(id) ? addAtCenter(id as WhiteboardShape) : pickTool(id),
              })),
              { id: "undo", label: t.undo, onSelect: () => run({ name: "undo" }) },
              { id: "redo", label: t.redo, onSelect: () => run({ name: "redo" }) },
              { id: "delete", label: t.deleteSelection, onSelect: () => remove(present) },
              { id: "fit", label: t.fitView, onSelect: () => run({ name: "fitView" }) },
              { id: "zoom-in", label: t.zoomIn, onSelect: () => run({ name: "zoomIn" }) },
              { id: "zoom-out", label: t.zoomOut, onSelect: () => run({ name: "zoomOut" }) },
            ]}
          />
        </Panel>
        {single && (
          <Panel position="top-right">
            <Properties
              key={single.id}
              shape={single}
              t={t}
              names={names}
              ends={ends}
              focus={fresh === single.id}
              onChange={(shape) => setState((current) => update(current, shape))}
              onConnect={link}
              onDone={() =>
                surface.current
                  ?.querySelector<HTMLElement>(`.react-flow__node[data-id="${CSS.escape(single.id)}"]`)
                  ?.focus()
              }
              onRemove={(id) => remove([id])}
            />
          </Panel>
        )}
        <Panel position="bottom-left">
          <p className="tim-whiteboard__count" role="status">
            {arrowHint ?? t.count(state.board.strokes.length, state.board.shapes.length)}
          </p>
          {/* A status, not an alert: the extension tries again every 2 s, and a screen reader
              says it once, when it changes (review of Phase 7, m7). */}
          {saved && (
            <p className="tim-whiteboard__saved" data-saved={saved.saved} role="status">
              {saved.saved ? t.saved : t.notSaved(saved.detail ?? "")}
            </p>
          )}
        </Panel>
      </ReactFlow>
      {live && (
        <svg className="tim-whiteboard__live" aria-hidden>
          {/* In the board's coordinates, as the strokes are: the same ink at every zoom. */}
          <g transform={`translate(${viewport.x} ${viewport.y}) scale(${viewport.zoom})`}>
            <path d={inkPath(live.points, live.pen, { last: false })} />
          </g>
        </svg>
      )}
      {empty && (
        <div className="tim-whiteboard__hint">
          <h1 className="tim-whiteboard__hint-title">{t.emptyTitle}</h1>
          <p>{t.emptyText}</p>
        </div>
      )}
    </div>
  );
}

export function Whiteboard(props: WhiteboardProps) {
  return (
    <ReactFlowProvider>
      <Canvas {...props} />
    </ReactFlowProvider>
  );
}
