/**
 * What React Flow draws for the whiteboard (ADR 0036 to 0038): the ink of a stroke, and the
 * typed shapes. A drawn box looks like a box of the map, with a dashed line that says it is
 * intended; an arrow says its policy by its line and its word, never by its colour alone.
 */
import {
  BaseEdge,
  EdgeLabelRenderer,
  Handle,
  NodeResizer,
  Position,
  type Edge,
  type EdgeProps,
  type Node,
  type NodeProps,
  type ResizeParams,
} from "@xyflow/react";
import { useMemo } from "react";

import type { BoxShape, BudgetShape, NoteShape, Policy, Rect, Stroke, ZoneShape } from "./board";
import { MIN_SIZE } from "./board";
import { inkPath, INK_PAD } from "./ink";

export type StrokeNode = Node<{ stroke: Stroke; erasing: boolean }, "stroke">;
type Common = {
  /** The words the shape shows when it has none of its own (« Unnamed component »). */
  placeholder: string;
  /** The first click of the arrow tool chose this one. */
  pending: boolean;
  /** Can be resized: selected, with the selection tool. */
  resizable: boolean;
  onResized: (id: string, rect: ResizeParams) => void;
};
export type BoxNode = Node<Common & { shape: BoxShape }, "box">;
export type ZoneNode = Node<Common & { shape: ZoneShape }, "zone">;
export type NoteNode = Node<Common & { shape: NoteShape; on: string | null }, "note">;
export type BudgetNode = Node<
  Common & { shape: BudgetShape; on: string | null; lines: string[]; title: string },
  "budget"
>;
export type WhiteboardNode = StrokeNode | BoxNode | ZoneNode | NoteNode | BudgetNode;

/** A stroke on the board: its ink, in a box just larger than its points. */
function StrokeInk({ data, width, height }: NodeProps<StrokeNode>) {
  const d = useMemo(
    () => inkPath(data.stroke.points, data.stroke.pen === true, { dx: INK_PAD, dy: INK_PAD }),
    [data.stroke],
  );
  return (
    <svg className="tim-stroke" data-erasing={data.erasing || undefined} width={width} height={height} aria-hidden>
      <path d={d} data-contrast="fill" />
    </svg>
  );
}

/** Arrows need ends to hang from: one handle each way, out of sight, never dragged. */
function Ends() {
  return (
    <>
      <Handle type="target" position={Position.Left} isConnectable={false} className="tim-wb-end" />
      <Handle type="source" position={Position.Right} isConnectable={false} className="tim-wb-end" />
    </>
  );
}

function Resizer({ id, data }: { id: string; data: Common }) {
  return (
    <NodeResizer
      isVisible={data.resizable}
      minWidth={MIN_SIZE.width}
      minHeight={MIN_SIZE.height}
      handleClassName="tim-wb-resize"
      lineClassName="tim-wb-resize-line"
      onResizeEnd={(_, rect) => data.onResized(id, rect)}
    />
  );
}

function BoxShapeNode({ id, data }: NodeProps<BoxNode>) {
  const { shape } = data;
  return (
    <div className="tim-wb-box" data-pending={data.pending || undefined} data-contrast="outline">
      <Resizer id={id} data={data} />
      <Ends />
      <strong className="tim-wb-box__name" data-empty={!shape.name || undefined}>
        {shape.name || data.placeholder}
      </strong>
      {shape.role && <span className="tim-wb-box__role">{shape.role}</span>}
    </div>
  );
}

function ZoneShapeNode({ id, data }: NodeProps<ZoneNode>) {
  const { shape } = data;
  return (
    <div className="tim-wb-zone" data-pending={data.pending || undefined} data-contrast="outline">
      <Resizer id={id} data={data} />
      <Ends />
      <span className="tim-wb-zone__name" data-empty={!shape.name || undefined}>
        {shape.name || data.placeholder}
      </span>
    </div>
  );
}

function NoteShapeNode({ data }: NodeProps<NoteNode>) {
  const { shape } = data;
  return (
    <div className="tim-wb-note" data-contrast="outline">
      <Ends />
      {data.on && <span className="tim-wb-note__on">{data.on}</span>}
      <p className="tim-wb-note__text" data-empty={!shape.text || undefined}>
        {shape.text || data.placeholder}
      </p>
    </div>
  );
}

function BudgetShapeNode({ data }: NodeProps<BudgetNode>) {
  return (
    <div className="tim-wb-note tim-wb-budget" data-contrast="outline">
      <Ends />
      <span className="tim-wb-budget__title">
        <GaugeIcon />
        {data.title}
      </span>
      {data.on && <span className="tim-wb-note__on">{data.on}</span>}
      {data.lines.length > 0 ? (
        <ul className="tim-wb-budget__limits">
          {data.lines.map((line) => (
            <li key={line}>{line}</li>
          ))}
        </ul>
      ) : (
        <p className="tim-wb-note__text" data-empty>
          {data.placeholder}
        </p>
      )}
    </div>
  );
}

/** A budget is a measure: a gauge, beside its title. */
function GaugeIcon() {
  return (
    <svg className="tim-icon" viewBox="0 0 16 16" width="14" height="14" aria-hidden>
      <path d="M2.5 11a5.5 5.5 0 1 1 11 0" fill="none" stroke="currentColor" strokeWidth="1.5" />
      <path d="M8 11l3-4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    </svg>
  );
}

export const nodeTypes = {
  stroke: StrokeInk,
  box: BoxShapeNode,
  zone: ZoneShapeNode,
  note: NoteShapeNode,
  budget: BudgetShapeNode,
};

/** An arrow between two boxes or zones, from border to border. */
export type ArrowEdge = Edge<{ policy: Policy; from: Rect; to: Rect; mark: string | null }, "arrow">;
/** The dotted line from a note or a budget to what it is attached to. */
export type AttachEdge = Edge<{ from: Rect; to: Rect }, "attach">;
export type WhiteboardEdge = ArrowEdge | AttachEdge;

/** Where the line from the middle of `rect` toward `toward` leaves it. */
export function border(rect: Rect, toward: { x: number; y: number }): { x: number; y: number } {
  const cx = rect.x + rect.width / 2;
  const cy = rect.y + rect.height / 2;
  const dx = toward.x - cx;
  const dy = toward.y - cy;
  if (dx === 0 && dy === 0) return { x: cx, y: cy };
  const scale = Math.min(
    dx === 0 ? Infinity : rect.width / 2 / Math.abs(dx),
    dy === 0 ? Infinity : rect.height / 2 / Math.abs(dy),
  );
  return { x: cx + dx * scale, y: cy + dy * scale };
}

const middle = (rect: Rect) => ({ x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 });

/** The two ends of a line between two rectangles, on their borders. */
export function ends(from: Rect, to: Rect) {
  return { start: border(from, middle(to)), end: border(to, middle(from)) };
}

/** The head of an arrow at `end`, coming from `start`: a closed triangle. */
export function head(start: { x: number; y: number }, end: { x: number; y: number }, size = 10): string {
  const angle = Math.atan2(end.y - start.y, end.x - start.x);
  const left = { x: end.x - size * Math.cos(angle - Math.PI / 7), y: end.y - size * Math.sin(angle - Math.PI / 7) };
  const right = { x: end.x - size * Math.cos(angle + Math.PI / 7), y: end.y - size * Math.sin(angle + Math.PI / 7) };
  return `M${end.x},${end.y} L${left.x},${left.y} L${right.x},${right.y} Z`;
}

function Arrow({ id, data, selected }: EdgeProps<ArrowEdge>) {
  if (!data) return null;
  const { start, end } = ends(data.from, data.to);
  const label = { x: (start.x + end.x) / 2, y: (start.y + end.y) / 2 };
  return (
    <>
      <BaseEdge
        id={id}
        path={`M${start.x},${start.y} L${end.x},${end.y}`}
        className={`tim-wb-arrow tim-wb-arrow--${data.policy}${selected ? " tim-wb-arrow--selected" : ""}`}
        interactionWidth={16}
      />
      <path d={head(start, end)} className={`tim-wb-arrow__head tim-wb-arrow__head--${data.policy}`} />
      {data.mark && (
        <EdgeLabelRenderer>
          <span
            className={`tim-wb-arrow__mark tim-wb-arrow__mark--${data.policy}`}
            style={{ transform: `translate(-50%, -50%) translate(${label.x}px, ${label.y}px)` }}
          >
            {data.policy === "forbidden" ? "⊘ " : "! "}
            {data.mark}
          </span>
        </EdgeLabelRenderer>
      )}
    </>
  );
}

function Attach({ id, data }: EdgeProps<AttachEdge>) {
  if (!data) return null;
  const { start, end } = ends(data.from, data.to);
  return <BaseEdge id={id} path={`M${start.x},${start.y} L${end.x},${end.y}`} className="tim-wb-attach" />;
}

export const edgeTypes = { arrow: Arrow, attach: Attach };
