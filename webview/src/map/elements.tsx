/**
 * What React Flow draws for the map (PLAN.md 4.7): role groups, boxes, open containers and
 * edges, from the design system (ADR 0020).
 */
import {
  BaseEdge,
  getBezierPath,
  Handle,
  Position,
  type Edge,
  type EdgeProps,
  type Node,
  type NodeProps,
} from "@xyflow/react";

import { Badge, ComponentBox } from "../design/components";
import { RoleIcon } from "../design/icons";
import { messages, type Locale } from "../i18n";
import type { Role } from "../protocol";
import { PIN_Y, ROLE } from "./layout";
import type { MapBox, MapEdge, MapGroup } from "./mapModel";

/** Which sides of an item edges leave from (right) or come into (left). */
export interface Sides {
  in: boolean;
  out: boolean;
}

/**
 * Where edges start and end, as the pins of a node editor: one entry on the left, one exit on
 * the right, level with the title bar, drawn over the box. Shown only where an edge is; unknown
 * to screen readers, as the edge's own name says where it goes.
 */
function Ends({ y, sides }: { y: number; sides: Sides }) {
  return (
    <>
      <Handle
        type="target"
        position={Position.Left}
        isConnectable={false}
        className={sides.in ? "tim-map-handle tim-map-handle--on" : "tim-map-handle"}
        style={{ top: y }}
        aria-hidden="true"
        data-contrast={sides.in ? "fill" : undefined}
      />
      <Handle
        type="source"
        position={Position.Right}
        isConnectable={false}
        className={sides.out ? "tim-map-handle tim-map-handle--on" : "tim-map-handle"}
        style={{ top: y }}
        aria-hidden="true"
        data-contrast={sides.out ? "fill" : undefined}
      />
    </>
  );
}

export type RoleNode = Node<{ group: MapGroup; locale: Locale; sides: Sides }, "role">;

/** A role: closed, a card that lists its members; open, the frame around them. */
export function RoleElement({ data }: NodeProps<RoleNode>) {
  const { group, locale, sides } = data;
  const t = messages[locale];
  const label = group.role ? t.roles[group.role] : t.noRole;
  if (group.open) {
    return (
      <>
        <div className="tim-map-frame" data-contrast="outline" data-role={group.role ?? undefined}>
          <div className="tim-map-frame__header" data-contrast="icon">
            <RoleIcon role={group.role} />
            <span className="tim-map-frame__title">{label}</span>
            <span className="tim-map-frame__meta">{t.members(group.members.length)}</span>
          </div>
        </div>
        <Ends y={PIN_Y.frame} sides={sides} />
      </>
    );
  }
  const shown = group.members.slice(0, ROLE.lines);
  const more = group.members.length - shown.length;
  return (
    <>
      <div className="tim-map-role" data-contrast="outline" data-role={group.role ?? undefined}>
        <div className="tim-map-role__title" data-contrast="icon">
          <RoleIcon role={group.role} />
          <span>{label}</span>
        </div>
        <ul className="tim-map-role__members">
          {shown.map((member) => (
            <li key={member.id}>{member.name}</li>
          ))}
          {more > 0 && <li className="tim-map-role__more">{t.moreMembers(more)}</li>}
        </ul>
      </div>
      <Ends y={PIN_Y.role} sides={sides} />
    </>
  );
}

export type BoxNode = Node<{ box: MapBox; locale: Locale; sides: Sides }, "box">;

/** A component, package, file or symbol: closed, a box; open, the frame around its contents. */
export function BoxElement({ data }: NodeProps<BoxNode>) {
  const { box, locale, sides } = data;
  const t = messages[locale];
  const { node } = box;
  if (box.open) {
    return (
      <>
        <div className="tim-map-frame tim-map-frame--box" data-contrast="outline" data-role={node.role ?? undefined}>
          <div className="tim-map-frame__header" data-contrast="icon">
            <RoleIcon role={node.role} />
            <span className="tim-map-frame__title">{node.name}</span>
            <span className="tim-map-frame__meta">
              {node.role ? t.roles[node.role] : t.noRole} · {t.items(node.children)}
            </span>
          </div>
        </div>
        <Ends y={PIN_Y.frame} sides={sides} />
      </>
    );
  }
  return (
    <>
      <ComponentBox
        name={node.name}
        role={node.role}
        items={node.children}
        locale={locale}
        focusable={false}
        badges={box.folded ? <Badge tone="neutral" label={t.folded(node.children)} /> : undefined}
      />
      <Ends y={PIN_Y.box} sides={sides} />
    </>
  );
}

export type LineEdge = Edge<{ line: MapEdge; active: Role | "neutral" | null }, "line">;

/**
 * An edge, a wire from the right of a box to the left of another: dashed and paler when
 * tree-sitter alone saw it (spec « Précision affichée »). The wires of the item pointed at or
 * focused take its colour.
 */
export function LineElement({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  data,
}: EdgeProps<LineEdge>) {
  const [path] = getBezierPath({ sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition });
  const approximate = data?.line.approximate ?? true;
  const active = data?.active ?? null;
  const classes = ["tim-edge", approximate ? "tim-edge--approximate" : "tim-edge--confirmed"];
  if (active) classes.push("tim-edge--active");
  return (
    <>
      {active && <path d={path} className="tim-edge-casing" aria-hidden="true" />}
      <BaseEdge
        id={id}
        path={path}
        className={classes.join(" ")}
        data-role={active && active !== "neutral" ? active : undefined}
        data-contrast="stroke"
      />
    </>
  );
}

export const nodeTypes = { role: RoleElement, box: BoxElement };
export const edgeTypes = { line: LineElement };
