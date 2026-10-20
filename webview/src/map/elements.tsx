/**
 * What React Flow draws for the map (PLAN.md 4.7): role groups, boxes, open containers and
 * edges, from the design system (ADR 0020).
 */
import {
  BaseEdge,
  EdgeLabelRenderer,
  getBezierPath,
  Handle,
  Position,
  type Edge,
  type EdgeProps,
  type Node,
  type NodeProps,
} from "@xyflow/react";

import { Badge, ComponentBox } from "../design/components";
import { ChangeIcon, RoleIcon } from "../design/icons";
import { messages, type Locale } from "../i18n";
import type { Density, NodeDebt, Role } from "../protocol";
import { METRICS } from "./layout";
import { debtLevel, type MapBox, type MapEdge, type MapGroup } from "./mapModel";
import { LinkBadge, primaryChange, ReviewBadge } from "./review";

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

/**
 * Out of the filter « only the changes »: drawn, but quieter (ADR 0024). `debt`: the layer of
 * the review debt is on (ADR 0030).
 */
export type Quiet = { quiet: boolean; debt?: boolean };

/** The words of a review debt: how many files to read again, and whether some are untracked. */
function DebtWords({ debt, locale, short }: { debt: NodeDebt; locale: Locale; short?: boolean }) {
  const t = messages[locale].review;
  return (
    <span className="tim-debt">
      <span className="tim-debt__files">{short ? t.debtShort(debt.files) : t.debtFiles(debt.files)}</span>
      {debt.untracked > 0 && (
        <span className="tim-debt__untracked">
          <span className="tim-debt__hatch" aria-hidden="true" />
          {t.untracked}
        </span>
      )}
    </span>
  );
}

/**
 * How many members changed since the last review, in a title: the shape and the number, the
 * words on hover; the node's name says which ones.
 */
function Changes({ count, locale }: { count: number; locale: Locale }) {
  if (count === 0) return null;
  return (
    <span className="tim-map-changes" title={messages[locale].review.changes(count)}>
      <Badge
        tone="attention"
        icon={<ChangeIcon kind="modified" />}
        label={messages[locale].review.changedCount(count)}
      />
    </span>
  );
}

export type RoleNode = Node<{ group: MapGroup; locale: Locale; sides: Sides; density: Density } & Quiet, "role">;

/** A role: closed, a card that lists its members; open, the frame around them. */
export function RoleElement({ data }: NodeProps<RoleNode>) {
  const { group, locale, sides, quiet, density } = data;
  const debt = data.debt && group.debt ? group.debt : null;
  const { pinY, role } = METRICS[density];
  const t = messages[locale];
  const label = group.role ? t.roles[group.role] : t.noRole;
  if (group.open) {
    return (
      <>
        <div
          className={quiet ? "tim-map-frame tim-map--quiet" : "tim-map-frame"}
          data-contrast="outline"
          data-role={group.role ?? undefined}
        >
          <div className="tim-map-frame__header" data-contrast="icon">
            <RoleIcon role={group.role} />
            <span className="tim-map-frame__title">{label}</span>
            <span className="tim-map-frame__meta">{t.members(group.members.length)}</span>
            {data.debt ? (
              debt && <DebtWords debt={debt} locale={locale} short />
            ) : (
              <Changes count={group.changes} locale={locale} />
            )}
          </div>
        </div>
        <Ends y={pinY.frame} sides={sides} />
      </>
    );
  }
  const shown = group.members.slice(0, role.lines);
  const more = group.members.length - shown.length;
  return (
    <>
      <div
        className={quiet ? "tim-map-role tim-map--quiet" : "tim-map-role"}
        data-contrast="outline"
        data-role={group.role ?? undefined}
        data-debt={debtLevel(debt) || undefined}
      >
        <div className="tim-map-role__title" data-contrast="icon">
          <RoleIcon role={group.role} />
          <span className="tim-map-role__label">{label}</span>
          {data.debt ? (
            debt && <DebtWords debt={debt} locale={locale} short />
          ) : (
            <Changes count={group.changes} locale={locale} />
          )}
        </div>
        <ul className="tim-map-role__members">
          {shown.map((member) => {
            const change = member.review ? primaryChange(member.review) : null;
            return (
              <li key={member.id}>
                <span className="tim-map-role__member">{member.name}</span>
                {/* The card's name says which members changed: the shape only repeats it. */}
                {change && (
                  <span className="tim-map-role__change" data-contrast="icon">
                    <ChangeIcon kind={change} />
                  </span>
                )}
              </li>
            );
          })}
          {more > 0 && <li className="tim-map-role__more">{t.moreMembers(more)}</li>}
        </ul>
      </div>
      <Ends y={pinY.role} sides={sides} />
    </>
  );
}

export type BoxNode = Node<{ box: MapBox; locale: Locale; sides: Sides; density: Density } & Quiet, "box">;

/** A component, package, file or symbol: closed, a box; open, the frame around its contents. */
export function BoxElement({ data }: NodeProps<BoxNode>) {
  const { box, locale, sides, quiet, density } = data;
  const { pinY } = METRICS[density];
  const t = messages[locale];
  const { node } = box;
  const flag = node.review ? <ReviewBadge review={node.review} locale={locale} /> : undefined;
  const debt = data.debt && node.debt ? node.debt : null;
  if (box.open) {
    return (
      <>
        <div
          className={quiet ? "tim-map-frame tim-map-frame--box tim-map--quiet" : "tim-map-frame tim-map-frame--box"}
          data-contrast="outline"
          data-role={node.role ?? undefined}
        >
          <div className="tim-map-frame__header" data-contrast="icon">
            <RoleIcon role={node.role} />
            <span className="tim-map-frame__title">{node.name}</span>
            <span className="tim-map-frame__meta">
              {node.role ? t.roles[node.role] : t.noRole} · {t.items(node.children)}
            </span>
            {data.debt ? debt && <DebtWords debt={debt} locale={locale} short /> : flag}
          </div>
        </div>
        <Ends y={pinY.frame} sides={sides} />
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
        dimmed={quiet}
        flag={data.debt ? undefined : flag}
        badges={box.folded ? <Badge tone="neutral" label={t.folded(node.children)} /> : undefined}
        debt={debt ? <DebtWords debt={debt} locale={locale} /> : undefined}
        debtLevel={debtLevel(debt)}
      />
      <Ends y={pinY.box} sides={sides} />
    </>
  );
}

export type LineEdge = Edge<{ line: MapEdge; active: Role | "neutral" | null; locale: Locale } & Quiet, "line">;

/**
 * An edge, a wire from the right of a box to the left of another: dashed and paler when
 * tree-sitter alone saw it (spec « Précision affichée »). The wires of the item pointed at or
 * focused take its colour. New since the last review, or in a cycle added, it takes the colour
 * of attention and a mark halfway (ADR 0024).
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
  const [path, labelX, labelY] = getBezierPath({
    sourceX,
    sourceY,
    targetX,
    targetY,
    sourcePosition,
    targetPosition,
  });
  const approximate = data?.line.approximate ?? true;
  const active = data?.active ?? null;
  const review = data?.line.review ?? null;
  const quiet = data?.quiet ?? false;
  const classes = ["tim-edge", approximate ? "tim-edge--approximate" : "tim-edge--confirmed"];
  if (review && !active) classes.push("tim-edge--review");
  if (active) classes.push("tim-edge--active");
  if (quiet && !active) classes.push("tim-edge--quiet");
  return (
    <>
      {active && <path d={path} className="tim-edge-casing" aria-hidden="true" />}
      <BaseEdge
        id={id}
        path={path}
        className={classes.join(" ")}
        data-role={active && active !== "neutral" ? active : undefined}
        // Quieted by the filter, it steps back: its contrast is no longer its job.
        data-contrast={quiet && !active ? undefined : "stroke"}
      />
      {review && data && (
        <EdgeLabelRenderer>
          <div
            className="tim-edge-flag nodrag nopan"
            aria-hidden="true"
            style={{ transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)` }}
          >
            <LinkBadge review={review} locale={data.locale} />
          </div>
        </EdgeLabelRenderer>
      )}
    </>
  );
}

export const nodeTypes = { role: RoleElement, box: BoxElement };
export const edgeTypes = { line: LineElement };
