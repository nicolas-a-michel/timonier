/**
 * The architecture map (PLAN.md 4.7): React Flow draws what `mapContent` keeps, where `layout`
 * puts it. The layout is computed; a box moved by hand stays where it was left (ADR 0021). A
 * click or Enter opens a role or a box; hovering or focusing an item shows its details.
 */
import "@xyflow/react/dist/base.css";

import {
  Background,
  BackgroundVariant,
  Panel,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
  type Node,
  type NodeChange,
  type XYPosition,
} from "@xyflow/react";
import { useEffect, useMemo, useRef, useState, type FocusEvent, type KeyboardEvent } from "react";

import { EdgeLegend, Toolbar } from "../design/components";
import { ArrowIcon } from "../design/icons";
import { messages, type Locale } from "../i18n";
import type { BudgetsView, Density, DriftView, MapCommand, NodeDebt, PerfMap, Role, Span } from "../protocol";
import { performanceMessages } from "../performance/messages";
import { duration } from "../performance/model";
import type { ViewData } from "../viewModel";
import { edgeTypes, nodeTypes, type BoxNode, type LineEdge, type RoleNode, type Sides } from "./elements";
import { layout, METRICS, ROOT, type Layout, type Pins } from "./layout";
import { changed, mapContent, symbolOf, toggleOf, type MapContent, type MapItem, type Toggle } from "./mapModel";
import { directionOf, nearest, type Direction } from "./navigation";
import { intentMarks, type IntentMark, type IntentMarks } from "./intent";
import type { Obstacle } from "./labelSpot";
import { IntentPanel } from "./intentPanel";
import { budgetMarks, perfMarks, type PerfMark, type PerfMarks } from "./perf";
import { PerfPanel } from "./perfPanel";
import { reviewLines, ReviewSummaryPanel } from "./review";
import { absolute } from "./stability";

export interface MapViewProps {
  view: ViewData;
  locale: Locale;
  openRoles: ReadonlySet<string>;
  /** A folded box whose contents come first within the budget (PLAN.md 4.10). */
  focus?: string;
  onToggle: (toggle: Toggle) => void;
  /** Back to the opening: every group and box closed. */
  onCloseAll: () => void;
  /** Called once a layout is on screen. */
  onLaidOut?: () => void;
  /** Open this code in the editor. */
  onOpenCode?: (file: string, span: Span | null) => void;
  /** The files the map now draws, relative to the root: they come first for refinement. */
  onDisplayed?: (paths: string[]) => void;
  /** Where the palette's commands come in (PLAN.md 4.9). */
  commandsRef?: MapCommands;
  /** The boxes moved by hand (ADR 0021, Q20). */
  pins?: Pins;
  /** Boxes were moved, or put back where computed: every pin, as they are now. */
  onPins?: (pins: Pins) => void;
  /** How dense the map is drawn: the sizes of its boxes and the room between them (Q32). */
  density?: Density;
  /** The « Mark as reviewed » button (ADR 0024). */
  onMarkReviewed?: () => void;
  /** What did not change since the last review is quieter (ADR 0024). */
  onlyChanges?: boolean;
  onToggleChanges?: () => void;
  /** The review debt shows on the boxes (ADR 0030, Q44). */
  showDebt?: boolean;
  onToggleDebt?: () => void;
  /** Open what leads to a box (ADR 0032): the host's state changes, the map centers it. */
  onReveal?: (target: string, path: string[]) => void;
  /** The map compared with the whiteboard (ADR 0042), and the drift the host gave for it:
   * `undefined` until it comes, `null` when it cannot. */
  compareIntent?: boolean;
  intent?: DriftView | null;
  onToggleIntent?: () => void;
  onOpenWhiteboard?: () => void;
  /** The Performances layer (ADR 0051), and what the host gave for it: `undefined` until it
   * comes, `null` when it cannot. */
  showPerformance?: boolean;
  performance?: PerfMap | null;
  onTogglePerformance?: () => void;
  onOpenPerformance?: () => void;
  /** The whiteboard's budgets on the last runs: an exceeded one marks its box, always (Q74). */
  budgets?: BudgetsView | null;
}

/** What an item cost in the run, for screen readers and the details (ADR 0051). */
function perfLine(mark: PerfMark | undefined, locale: Locale): string[] {
  if (!mark) return [];
  return [
    `${performanceMessages[locale].boxWords(duration(mark.totalNs, locale), duration(mark.selfNs, locale), mark.calls)}.`,
  ];
}

/** The words of a review debt, for screen readers: as many files, and whether untracked. */
function debtLine(debt: NodeDebt | null, locale: Locale): string[] {
  if (!debt) return [];
  const t = messages[locale].review;
  return [`${t.debtFiles(debt.files)}${debt.untracked > 0 ? `, ${t.untracked}` : ""}.`];
}

/** What a box says of the whiteboard, for screen readers (ADR 0042); a role, member by member. */
function intentLine(mark: IntentMark | undefined, locale: Locale): string[] {
  if (!mark) return [];
  const t = messages[locale].intent;
  if (mark.members.length > 0) {
    return mark.members.map((m) => `${m.name}: ${(m.drawn ? t.drawnBadge : t.withinBadge)(m.names.join(", "))}.`);
  }
  return [
    ...(mark.drawn.length > 0 ? [`${t.drawnBadge(mark.drawn.join(", "))}.`] : []),
    ...(mark.within.length > 0 ? [`${t.withinBadge(mark.within.join(", "))}.`] : []),
  ];
}

/** Where the palette's commands come in; those that came before the map are kept in `pending`. */
export interface MapCommands {
  current: ((command: MapCommand) => void) | null;
  pending: MapCommand[];
}

/** What one item is, for screen readers and for the details panel. */
function describe(item: MapItem, locale: Locale): { name: string; label: string; lines: string[] } {
  const t = messages[locale];
  if (item.kind === "role") {
    const name = item.role ? t.roles[item.role] : t.noRole;
    const members = item.members.map((m) => m.name).join(", ");
    const changes = item.members.filter((m) => m.review).map((m) => m.name);
    const changedLine = changes.length > 0 ? [t.review.changedMembers(changes.join(", "))] : [];
    return {
      name,
      label: [
        `${name}, ${t.members(item.members.length)}: ${members}.`,
        ...changedLine,
        item.open ? t.closeHint : t.openHint,
      ].join(" "),
      lines: [t.members(item.members.length), members, ...changedLine],
    };
  }
  const { node } = item;
  const symbol = symbolOf(node);
  const role = node.role ? t.roles[node.role] : t.noRole;
  // A symbol says its kind and its signature (T-114).
  const kind = symbol
    ? t.symbolTip(t.symbolKinds[symbol.kind], symbol.signature)
    : node.kind === "repo" || node.kind === "component" || node.kind === "package" || node.kind === "file"
      ? t.kinds[node.kind]
      : t.kinds.symbol;
  const hint = item.folded
    ? ` ${t.foldedHint}`
    : node.children > 0
      ? ` ${item.open ? t.closeHint : t.openHint}`
      : node.file
        ? ` ${t.codeHint}`
        : "";
  const changes = reviewLines(node.review, locale);
  const said = changes.map((line) => ` ${line}.`).join("");
  return {
    name: node.name,
    label: symbol
      ? `${node.name}, ${kind}, ${t.items(node.children)}.${said}${hint}`
      : `${node.name}, ${kind}, ${role}, ${t.items(node.children)}.${said}${hint}`,
    lines: [symbol ? kind : `${kind} · ${role}`, t.items(node.children), ...(node.file ? [node.file] : []), ...changes],
  };
}

function flowNodes(
  content: MapContent,
  placed: Layout,
  locale: Locale,
  onlyChanges: boolean,
  density: Density,
  debt: boolean,
  marks?: IntentMarks,
  perf?: PerfMarks,
  budgets?: Map<string, string[]>,
): Node[] {
  const nodes: Node[] = [];
  const ins = new Set(content.edges.map((edge) => edge.target));
  const outs = new Set(content.edges.map((edge) => edge.source));
  for (const item of content.items) {
    const sides: Sides = { in: ins.has(item.id), out: outs.has(item.id) };
    const quiet = onlyChanges && !changed(item);
    const at = placed.get(item.id);
    if (!at) continue;
    const common = {
      id: item.id,
      position: { x: at.x, y: at.y },
      width: at.width,
      height: at.height,
      ariaLabel: [
        describe(item, locale).label,
        ...(debt ? debtLine(item.kind === "role" ? item.debt : item.node.debt, locale) : []),
        ...intentLine(marks?.items.get(item.id), locale),
        ...perfLine(perf?.items.get(item.id), locale),
        ...(budgets?.get(item.id) ?? []).map((said) => `${said}.`),
      ].join(" "),
      draggable: true,
      connectable: false,
      selectable: true,
      // An open frame is moved by its title bar; its body lets the pointer through, to the
      // boxes it holds and to the board, which a drag there pans (ADR 0021).
      ...(item.open ? { dragHandle: ".tim-map-frame__header", className: "tim-map-node--frame" } : {}),
    };
    if (item.kind === "role") {
      nodes.push({
        ...common,
        type: "role",
        data: {
          group: item,
          locale,
          sides,
          quiet,
          density,
          debt,
          intent: marks?.items.get(item.id),
          perf: perf && (perf.items.get(item.id) ?? null),
          budgets: budgets?.get(item.id),
        },
      } satisfies RoleNode);
    } else {
      nodes.push({
        ...common,
        type: "box",
        parentId: item.parent.startsWith("@role/") || placed.has(item.parent) ? item.parent : undefined,
        data: {
          box: item,
          locale,
          sides,
          quiet,
          density,
          debt,
          intent: marks?.items.get(item.id),
          perf: perf && (perf.items.get(item.id) ?? null),
          budgets: budgets?.get(item.id),
        },
      } satisfies BoxNode);
    }
  }
  return nodes;
}

/** The colour the wires of `item` take while it is pointed at or focused. */
function wireColour(item: MapItem | undefined): Role | "neutral" | null {
  if (!item) return null;
  return (item.kind === "role" ? item.role : item.node.role) ?? "neutral";
}

/** The edges, as React Flow wants them: built once per layout. */
function flowEdges(
  content: MapContent,
  locale: Locale,
  onlyChanges: boolean,
  marks?: IntentMarks,
  covering?: readonly Obstacle[],
  perf?: PerfMarks,
): LineEdge[] {
  const t = messages[locale];
  const names = new Map(content.items.map((item) => [item.id, describe(item, locale).name]));
  return content.edges.map(
    (line) =>
      ({
        id: line.id,
        source: line.source,
        target: line.target,
        type: "line",
        data: {
          line,
          active: null,
          locale,
          quiet: onlyChanges && !line.review,
          forbidden: marks?.lines.get(line.id),
          ...(marks?.lines.has(line.id) ? { covering } : {}),
          ...(perf ? { traffic: { calls: perf.lines.get(line.id) ?? 0, most: perf.most } } : {}),
        },
        focusable: false,
        selectable: false,
        // Under every box: a line never crosses a box's text. Open frames have no fill.
        zIndex: 0,
        ariaLabel: [
          t.edgeLabel(
            names.get(line.source) ?? line.source,
            names.get(line.target) ?? line.target,
            line.edges,
            line.approximate,
          ),
          ...(line.review?.added ? [`${t.review.addedLong}.`] : []),
          ...(line.review?.cycle ? [`${t.review.cycleLong}.`] : []),
          ...(marks?.lines.has(line.id) ? [t.intent.forbiddenLine((marks.lines.get(line.id) ?? []).join(", "))] : []),
          ...(perf?.lines.has(line.id)
            ? [`${performanceMessages[locale].linkWords(perf.lines.get(line.id) ?? 0)}.`]
            : []),
        ].join(" "),
      }) satisfies LineEdge,
  );
}

/**
 * The edges of the item pointed at take its colour and come last, drawn over the others. The
 * others keep their objects: React Flow draws again only what changed, so that pointing at boxes
 * while panning stays cheap.
 */
function withActive(edges: LineEdge[], shown: MapItem | undefined): LineEdge[] {
  if (!shown) return edges;
  const colour = wireColour(shown);
  const touches = (edge: LineEdge) => edge.source === shown.id || edge.target === shown.id;
  const active = edges.filter(touches);
  if (active.length === 0) return edges;
  return [
    ...edges.filter((edge) => !touches(edge)),
    ...active.map((edge) => (edge.data ? { ...edge, data: { ...edge.data, active: colour } } : edge)),
  ];
}

function Canvas({
  view,
  locale,
  openRoles,
  focus,
  onToggle,
  onCloseAll,
  onLaidOut,
  onOpenCode,
  onDisplayed,
  commandsRef,
  pins = NO_PINS,
  onPins,
  onMarkReviewed,
  density = "comfortable",
  onlyChanges: filter = false,
  onToggleChanges,
  showDebt = false,
  onToggleDebt,
  onReveal,
  compareIntent = false,
  intent,
  onToggleIntent,
  onOpenWhiteboard,
  showPerformance = false,
  performance,
  onTogglePerformance,
  onOpenPerformance,
  budgets,
}: MapViewProps) {
  // Before the first review there is nothing to highlight: the map shows as it is (Q27).
  const onlyChanges = filter && view.review.since !== null;
  const [reviewDetails, setReviewDetails] = useState(false);
  const t = messages[locale];
  const content = useMemo(() => mapContent(view, openRoles, { focus }), [view, openRoles, focus]);
  const [placed, setPlaced] = useState<{ content: MapContent; layout: Layout } | undefined>();
  const shown = useRef<{ content: MapContent; layout: Layout } | undefined>(undefined);
  /** The next layout starts afresh, without the previous one. */
  const afresh = useRef(false);
  /** The item the palette's commands act on. */
  const [focused, setFocused] = useState<string | undefined>();
  /** Zoomed out: boxes show their name only (PLAN.md 4.10). Changes at the threshold only. */
  const [far, setFar] = useState(false);
  const root = useRef<HTMLElement>(null);
  const flow = useReactFlow();
  /** Where React Flow drags boxes, until the next layout; and which are selected. */
  const [moving, setMoving] = useState<ReadonlyMap<string, XYPosition>>(NOWHERE);
  const [selected, setSelected] = useState<ReadonlySet<string>>(NONE);
  /** What React Flow measured of each box: it moves only boxes it has measured. */
  const [measured, setMeasured] = useState<ReadonlyMap<string, { width?: number; height?: number }>>(UNMEASURED);
  /** The density of the layout on screen: another one starts afresh, every size changed. */
  const laidDensity = useRef(density);
  useEffect(() => {
    let current = true;
    // From the layout on screen: what did not change stays where it was (PLAN.md 4.8).
    // « Back to the roles » starts afresh: kept places would leave the gaps of the open frames.
    const previous = afresh.current || laidDensity.current !== density ? undefined : shown.current;
    afresh.current = false;
    laidDensity.current = density;
    void layout(content, previous && { content: previous.content, placed: previous.layout }, pins, density).then(
      (result) => {
        if (!current) return;
        shown.current = { content, layout: result };
        setPlaced(shown.current);
        // What was being dragged is now where the layout put it.
        setMoving(NOWHERE);
      },
    );
    return () => {
      current = false;
    };
  }, [content, pins, density]);
  // The map compared with the whiteboard: which item stands for which drawn component (ADR 0042).
  const marks = useMemo(
    () => (compareIntent && intent?.drawn && placed ? intentMarks(intent, placed.content, view) : undefined),
    [compareIntent, intent, placed, view],
  );
  // The Performances layer: what each item cost in the last run, and its lines' traffic (ADR 0051).
  const perf = useMemo(
    () => (showPerformance && performance && placed ? perfMarks(performance, placed.content, view) : undefined),
    [showPerformance, performance, placed, view],
  );
  // The budgets the last runs exceed, on the boxes that hold them, always (Q74, ADR 0052).
  const exceeded = useMemo(
    () => (budgets && placed ? budgetMarks(budgets, placed.content, view, locale) : undefined),
    [budgets, placed, view, locale],
  );
  const laidOut = useMemo(
    () =>
      placed
        ? flowNodes(placed.content, placed.layout, locale, onlyChanges, density, showDebt, marks, perf, exceeded)
        : [],
    [placed, locale, onlyChanges, density, showDebt, marks, perf, exceeded],
  );
  const nodes = useMemo(
    () =>
      laidOut.map((node) => {
        const at = moving.get(node.id);
        const chosen = selected.has(node.id);
        const size = measured.get(node.id);
        return at || chosen || size
          ? { ...node, position: at ?? node.position, selected: chosen, measured: size }
          : node;
      }),
    [laidOut, moving, selected, measured],
  );
  const onNodesChange = (changes: NodeChange[]) => {
    let nextMoving: Map<string, XYPosition> | undefined;
    let nextSelected: Set<string> | undefined;
    let nextMeasured: Map<string, { width?: number; height?: number }> | undefined;
    for (const change of changes) {
      if (change.type === "dimensions" && change.dimensions) {
        nextMeasured ??= new Map(measured);
        nextMeasured.set(change.id, change.dimensions);
      } else if (change.type === "position" && change.position) {
        nextMoving ??= new Map(moving);
        nextMoving.set(change.id, change.position);
      } else if (change.type === "select") {
        nextSelected ??= new Set(selected);
        if (change.selected) nextSelected.add(change.id);
        else nextSelected.delete(change.id);
      }
    }
    if (nextMoving) setMoving(nextMoving);
    if (nextSelected) setSelected(nextSelected);
    if (nextMeasured) setMeasured(nextMeasured);
  };
  /** The item the details are about: the last one pointed at or focused. */
  const [shownId, setShownId] = useState<string | undefined>();
  const shownItem = useMemo(
    () => (shownId ? placed?.content.items.find((item) => item.id === shownId) : undefined),
    [placed, shownId],
  );
  // What a forbidden line's word may lie over, once per layout, not at each frame of a pan.
  const covering = useMemo(
    () => (placed && marks && marks.lines.size > 0 ? coveringOf(placed, density) : undefined),
    [placed, marks, density],
  );
  const allEdges = useMemo(
    () => (placed ? flowEdges(placed.content, locale, onlyChanges, marks, covering, perf) : []),
    [placed, locale, onlyChanges, marks, covering, perf],
  );
  const edges = useMemo(() => withActive(allEdges, shownItem), [allEdges, shownItem]);
  const fitted = useRef(false);
  const closeAll = () => {
    afresh.current = true;
    // And fits the window again, as at the opening.
    fitted.current = false;
    onCloseAll();
  };
  /** What a click or Enter just opened: the next layout brings it into view. */
  const reveal = useRef<string | undefined>(undefined);
  /**
   * Room for the panels over the map, so that fitting the view puts no box under them: the
   * toolbar and the legend above, the summary of the review below (review n° 1 of Phase 5, m6).
   */
  const fitPadding = () => {
    const height = (selector: string) => root.current?.querySelector(selector)?.getBoundingClientRect().height ?? 0;
    const gap = 16;
    return {
      top: `${Math.max(height(".tim-toolbar"), height(".tim-legend")) + gap}px`,
      // Above React Flow's attribution; the review's banner is above the map, not on it.
      bottom: `${3 * gap}px`,
      left: `${gap}px`,
      right: `${gap}px`,
    } as const;
  };
  useEffect(() => {
    if (!placed) return;
    onLaidOut?.();
    // The first layout fits the window; later ones keep the view where the person left it.
    if (!fitted.current) {
      fitted.current = true;
      void flow.fitView({ padding: fitPadding(), duration: 0 });
    } else if (reveal.current && placed.layout.has(reveal.current)) {
      // What was asked for, in view; never closer than its real size.
      void flow.fitView({ nodes: [{ id: reveal.current }], padding: 0.2, duration: 0, maxZoom: 1 });
      reveal.current = undefined;
    }
  }, [placed, flow, onLaidOut]);

  /** The files last said to be drawn: said again only when they change. */
  const displayed = useRef("");
  useEffect(() => {
    if (!placed || !onDisplayed) return;
    const paths = displayedFiles(placed.content);
    const key = paths.join("\n");
    if (key === displayed.current) return;
    displayed.current = key;
    onDisplayed(paths);
  }, [placed, onDisplayed]);

  const items = useMemo(
    () => new Map((placed?.content ?? content).items.map((item) => [item.id, item])),
    [placed, content],
  );
  /** A view node, drawn or not: a command of the palette may name one inside a closed group. */
  const nodeOf = (id: string | undefined) => {
    const item = id ? items.get(id) : undefined;
    if (item?.kind === "box") return item.node;
    return id ? view.nodes.get(id) : undefined;
  };
  const codeOf = (id: string | undefined) => {
    const node = nodeOf(id);
    return node?.file ? { file: node.file, span: node.span } : undefined;
  };
  const openCode = (id: string | undefined) => {
    const code = codeOf(id);
    if (code) onOpenCode?.(code.file, code.span);
  };
  /** A click or Enter: open what can be opened, else go to its code. */
  const activate = (id: string | undefined) => {
    const item = id ? items.get(id) : undefined;
    const node = nodeOf(id);
    const action = item
      ? toggleOf(item)
      : node && node.children > 0
        ? ({ kind: "expand", id: node.id } as const)
        : undefined;
    if (action) {
      reveal.current = action.id;
      onToggle(action);
    } else openCode(id);
  };
  const run = (command: MapCommand) => {
    switch (command.name) {
      case "fitView":
        void flow.fitView({ padding: fitPadding(), duration: 0 });
        return;
      case "zoomIn":
        void flow.zoomIn({ duration: 0 });
        return;
      case "zoomOut":
        void flow.zoomOut({ duration: 0 });
        return;
      case "closeAll":
        closeAll();
        return;
      case "toggle":
        activate(command.target ?? focused);
        return;
      case "openCode":
        openCode(command.target ?? focused);
        return;
      case "move":
        moveBy(command.target ?? focused, command.direction);
        return;
      case "resetLayout":
        resetLayout();
        return;
      case "toggleChanges":
        if (view.review.since) onToggleChanges?.();
        return;
      case "toggleReviewDetails":
        setReviewDetails(!reviewDetails);
        return;
      case "reveal": {
        // Opened by the host's state first (main.tsx); centered once laid out, and selected.
        onReveal?.(command.target, command.path);
        setSelected(new Set([command.target]));
        setFocused(command.target);
        if (placed?.layout.has(command.target)) {
          void flow.fitView({ nodes: [{ id: command.target }], padding: 0.2, duration: 0, maxZoom: 1 });
        } else reveal.current = command.target;
        return;
      }
      case "toggleDebt":
        onToggleDebt?.();
        return;
      case "toggleIntent":
        onToggleIntent?.();
        return;
      case "togglePerformance":
        onTogglePerformance?.();
        return;
    }
  };
  /** Pins boxes where they are now, relative to their container; the newest last. */
  const pinAt = (moved: { id: string; x: number; y: number }[]) => {
    if (!onPins) return;
    const next: Record<string, Pins[string]> = { ...pins };
    for (const { id, x, y } of moved) {
      const item = items.get(id);
      if (!item) continue;
      delete next[id];
      next[id] = { parent: item.kind === "box" ? item.parent : ROOT, x: Math.round(x), y: Math.round(y) };
    }
    const ids = Object.keys(next);
    for (const id of ids.slice(0, Math.max(0, ids.length - MAX_PINS))) delete next[id];
    onPins(next);
  };
  /** Alt and an arrow, or the palette: one cell of the board in that direction. */
  const moveBy = (id: string | undefined, direction: Direction) => {
    const at = id ? (moving.get(id) ?? placed?.layout.get(id)) : undefined;
    if (!id || !at) return;
    const [dx, dy] = STEPS[direction];
    pinAt([{ id, x: at.x + dx * GRID, y: at.y + dy * GRID }]);
  };
  /** Back to the computed layout: every pin forgotten, laid out afresh. */
  const resetLayout = () => {
    afresh.current = true;
    onPins?.(NO_PINS);
  };
  const focusNode = (id: string) => {
    const node = root.current?.querySelector<HTMLElement>(`.react-flow__node[data-id="${CSS.escape(id)}"]`);
    node?.focus();
  };
  /**
   * The arrows, before React Flow, which would move a selected box with them: alone, to the
   * nearest box in their direction; with Alt, the box itself, one cell of the board.
   */
  const onKeyDownCapture = (event: KeyboardEvent<HTMLElement>) => {
    const target = event.target as HTMLElement;
    const direction = directionOf(event.key);
    if (!direction || !target.classList.contains("react-flow__node")) return;
    event.preventDefault();
    event.stopPropagation();
    const id = target.dataset.id;
    if (event.altKey) moveBy(id, direction);
    else if (id && placed) {
      const next = nearest(id, direction, boxesOf(placed));
      if (next) focusNode(next);
    }
  };
  const onKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    const target = event.target as HTMLElement;
    // + and -, anywhere on the map but in a button: the zoom, as the palette's commands.
    if (!(target instanceof HTMLButtonElement) && !event.ctrlKey && !event.metaKey && !event.altKey) {
      const zoom = ZOOM_KEYS[event.key];
      if (zoom) {
        event.preventDefault();
        run({ name: zoom });
        return;
      }
    }
    // Escape puts the details away (WCAG 1.4.13); the next item pointed at or focused shows them.
    if (event.key === "Escape" && shownId) {
      setShownId(undefined);
      return;
    }
    if (!target.classList.contains("react-flow__node")) return;
    const id = target.dataset.id;
    if (event.key === "Enter") {
      event.preventDefault();
      if (event.shiftKey) openCode(id);
      else activate(id);
    } else if (event.key === "Home" && placed) {
      event.preventDefault();
      const first = placed.content.items[0];
      if (first) focusNode(first.id);
    }
  };
  const onFocus = (event: FocusEvent<HTMLElement>) => {
    const id = event.target.dataset.id;
    if (!id) return;
    setFocused(id);
    setShownId(id);
  };
  // The palette's commands act on the item that has the focus (PLAN.md 4.9).
  useEffect(() => {
    if (!commandsRef) return;
    commandsRef.current = run;
    for (const command of commandsRef.pending.splice(0)) run(command);
    return () => {
      commandsRef.current = null;
    };
  });
  const details = shownId ? items.get(shownId) : undefined;
  const described = details && {
    ...describe(details, locale),
    lines: [
      ...describe(details, locale).lines,
      ...intentLine(marks?.items.get(details.id), locale),
      ...perfLine(perf?.items.get(details.id), locale),
      ...(exceeded?.get(details.id) ?? []).map((said) => `${said}.`),
    ],
  };
  const code = codeOf(shownId);
  return (
    <section
      ref={root}
      className={far ? "tim-map tim-map--far" : "tim-map"}
      aria-label={t.mapLabel}
      onKeyDownCapture={onKeyDownCapture}
      onKeyDown={onKeyDown}
      onFocus={onFocus}
    >
      {/* What the screen answers first: what changed since the review, in a banner above the
          map (five-second test n° 1, PLAN.md 5.3; ADR 0024). */}
      <ReviewSummaryPanel
        summary={view.review}
        locale={locale}
        onMarkReviewed={onMarkReviewed}
        onlyChanges={onlyChanges}
        onToggleChanges={onToggleChanges}
        detailsOpen={reviewDetails}
        onToggleDetails={() => setReviewDetails(!reviewDetails)}
      />
      {compareIntent && intent !== undefined && (
        <IntentPanel view={intent} locale={locale} onOpenWhiteboard={onOpenWhiteboard} />
      )}
      {showPerformance && performance !== undefined && (
        <PerfPanel view={performance} locale={locale} onOpenPerformance={onOpenPerformance} />
      )}
      <div className="tim-map__flow">
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          edgeTypes={edgeTypes}
          nodesDraggable={Boolean(onPins)}
          // A click that moves a little stays a click; a box follows from 4 px on.
          nodeDragThreshold={4}
          nodeClickDistance={4}
          onNodesChange={onNodesChange}
          onNodeDragStop={(_, __, dragged) =>
            pinAt(dragged.map((node) => ({ id: node.id, x: node.position.x, y: node.position.y })))
          }
          nodesConnectable={false}
          edgesFocusable={false}
          elementsSelectable
          minZoom={0.1}
          maxZoom={2}
          zoomOnDoubleClick={false}
          // Edges stay under every box, those inside an open box too: a line never crosses a name.
          zIndexMode="manual"
          onMove={(_, viewport) => setFar(viewport.zoom < FAR_ZOOM)}
          onNodeClick={(_, node) => activate(node.id)}
          onNodeDoubleClick={(_, node) => openCode(node.id)}
          onNodeMouseEnter={(_, node) => setShownId(node.id)}

          ariaLabelConfig={{
            // Each node's name says what Enter does with it; this, what every node shares.
            "node.a11yDescription.default": t.nodeHint,
            "node.a11yDescription.keyboardDisabled": t.nodeHint,
          }}
        >
          {/* The board: faint lines; the finer ones go when zoomed out, where they would blur. */}
          {!far && <Background id="minor" variant={BackgroundVariant.Lines} gap={16} color="var(--tim-grid-minor)" />}
          <Background id="major" variant={BackgroundVariant.Lines} gap={128} color="var(--tim-grid-major)" />
          <Panel position="top-left">
            <Toolbar
              label={t.mapTools}
              items={[
                { id: "fit", label: t.fitView, onSelect: () => run({ name: "fitView" }) },
                { id: "zoom-in", label: t.zoomIn, onSelect: () => run({ name: "zoomIn" }) },
                { id: "zoom-out", label: t.zoomOut, onSelect: () => run({ name: "zoomOut" }) },
                { id: "close", label: t.closeAll, onSelect: closeAll },
                { id: "reset", label: t.resetLayout, onSelect: resetLayout },
                ...(onToggleDebt
                  ? [{ id: "debt", label: t.review.debtLayer, pressed: showDebt, onSelect: onToggleDebt }]
                  : []),
                ...(onToggleIntent
                  ? [{ id: "intent", label: t.intent.layer, pressed: compareIntent, onSelect: onToggleIntent }]
                  : []),
                ...(onTogglePerformance
                  ? [
                      {
                        id: "performance",
                        label: performanceMessages[locale].layer,
                        pressed: showPerformance,
                        onSelect: onTogglePerformance,
                      },
                    ]
                  : []),
              ]}
            />
          </Panel>
          <Panel position="top-right">
            <EdgeLegend
              locale={locale}
              changes={view.review.since !== null}
              debt={showDebt}
              intent={compareIntent && intent?.drawn === true}
              performance={Boolean(perf)}
              budgets={Boolean(exceeded && exceeded.size > 0)}
            />
          </Panel>
          {described && (
            <Panel position="bottom-left">
              <aside className="tim-map-details" aria-label={described.name}>
                <div className="tim-map-details__name">{described.name}</div>
                {described.lines.map((line) => (
                  <div key={line} className="tim-map-details__line">
                    {line}
                  </div>
                ))}
                {code && (
                  <button type="button" className="tim-button tim-map-details__code" onClick={() => openCode(shownId)}>
                    {t.goToCode}
                  </button>
                )}
                {onPins && (
                  // Moving a box with a pointer, without dragging it (WCAG 2.5.7).
                  <div className="tim-map-details__move" role="group" aria-label={t.moveGroup}>
                    {MOVES.map((direction) => (
                      <button
                        key={direction}
                        type="button"
                        className="tim-button tim-map-details__step"
                        aria-label={t.moveTo[direction]}
                        title={t.moveTo[direction]}
                        data-contrast="icon"
                        onClick={() => moveBy(shownId, direction)}
                      >
                        <ArrowIcon direction={direction} />
                      </button>
                    ))}
                  </div>
                )}
              </aside>
            </Panel>
          )}
        </ReactFlow>
      </div>
    </section>
  );
}

/** Below this zoom, boxes show their name only. */
const FAR_ZOOM = 0.5;

/** The buttons of the details panel that move a box, in reading order. */
const MOVES: Direction[] = ["left", "up", "down", "right"];

/** A cell of the board, the step of a box moved from the keyboard. */
const GRID = 16;
const STEPS: Record<Direction, [number, number]> = { left: [-1, 0], right: [1, 0], up: [0, -1], down: [0, 1] };
/** At most this many pins: beyond, the oldest go (ADR 0021). */
const MAX_PINS = 5000;
const NO_PINS: Pins = {};
const NOWHERE: ReadonlyMap<string, XYPosition> = new Map();
const UNMEASURED: ReadonlyMap<string, { width?: number; height?: number }> = new Map();
const NONE: ReadonlySet<string> = new Set();

/** The keys of the zoom: `=` is `+` without Shift on many layouts. */
const ZOOM_KEYS: Partial<Record<string, "zoomIn" | "zoomOut">> = { "+": "zoomIn", "=": "zoomIn", "-": "zoomOut" };

/** The files of the boxes drawn, sorted: those of files and of symbols. */
export function displayedFiles(content: MapContent): string[] {
  const files = new Set<string>();
  for (const item of content.items) if (item.kind === "box" && item.node.file) files.add(item.node.file);
  return [...files].sort();
}

/** Where each drawn item is on the map, for the arrow keys. */
/** The height of a box's title bar (tim-box__header), on the board. */
const BOX_HEADER = 32;

/**
 * What the word of a forbidden line may lie over (labelSpot.ts): a closed box or role card hides
 * the line, its title bar most of all; an open frame shows what it holds, its title bar aside.
 */
function coveringOf(placed: { content: MapContent; layout: Layout }, density: Density): Obstacle[] {
  const boxes = boxesOf(placed);
  const obstacles: Obstacle[] = [];
  for (const item of placed.content.items) {
    const at = boxes.get(item.id);
    if (!at) continue;
    const metrics = METRICS[density];
    const header = item.open ? metrics.containerHeader : item.kind === "role" ? metrics.role.header : BOX_HEADER;
    obstacles.push({ ...at, height: Math.min(header, at.height), solid: true, title: true });
    if (!item.open) obstacles.push({ ...at, solid: true, title: false });
  }
  return obstacles;
}

function boxesOf(placed: { content: MapContent; layout: Layout }) {
  const positions = absolute(placed.content, placed.layout);
  const boxes = new Map<string, { x: number; y: number; width: number; height: number }>();
  for (const [id, at] of positions) {
    const size = placed.layout.get(id);
    if (size) boxes.set(id, { ...at, width: size.width, height: size.height });
  }
  return boxes;
}

export function MapView(props: MapViewProps) {
  return (
    <ReactFlowProvider>
      <Canvas {...props} />
    </ReactFlowProvider>
  );
}
