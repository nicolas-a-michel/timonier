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
  ViewportPortal,
  type Edge,
  type EdgeProps,
  type Node,
  type NodeProps,
} from "@xyflow/react";
import { useMemo } from "react";

import { Badge, ComponentBox } from "../design/components";
import { ChangeIcon, IntentIcon, RoleIcon, SymbolIcon } from "../design/icons";
import { messages, type Locale } from "../i18n";
import type { Density, NodeDebt, Role } from "../protocol";
import { METRICS } from "./layout";
import type { IntentMark } from "./intent";
import { trafficWidth, type PerfMark } from "./perf";
import { performanceMessages } from "../performance/messages";
import { duration } from "../performance/model";
import { labelSpot, type Obstacle } from "./labelSpot";
import { debtLevel, symbolOf, type MapBox, type MapEdge, type MapGroup } from "./mapModel";
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
export type Quiet = {
  quiet: boolean;
  debt?: boolean;
  intent?: IntentMark;
  /** The Performances layer is on (ADR 0051): what the item cost, `null` when nothing measured. */
  perf?: PerfMark | null;
  /** The budgets it holds that the last runs exceed, in words (ADR 0052). */
  budgets?: string[];
};

/** A budget the last runs exceed (Q74, ADR 0052): the violation's shape and word, in the title
 * bar; what it says, on hover and in the item's name. */
function BudgetFlag({ said, locale }: { said: string[] | undefined; locale: Locale }) {
  if (!said || said.length === 0) return null;
  return (
    <span className="tim-budget-mark" title={said.join(" · ")}>
      <Badge tone="violation" label={performanceMessages[locale].budgetMark} />
    </span>
  );
}

/** What an item cost in the run (ADR 0051, Q76): its cumulative time; its own time and calls when there is room. */
function PerfWords({ mark, locale, short }: { mark: PerfMark; locale: Locale; short?: boolean }) {
  const t = performanceMessages[locale];
  const time = duration(mark.totalNs, locale);
  const words = t.boxWords(time, duration(mark.selfNs, locale), mark.calls);
  return (
    <span className="tim-perf-words" title={words}>
      <span className="tim-perf-words__time">{short ? t.boxShort(time) : words}</span>
    </span>
  );
}

/**
 * What a box says of the whiteboard (ADR 0042): an icon in its title bar, which always has room
 * and stays at every zoom (review n° 3 of Phase 7, I-1); its words in its name, its tooltip and
 * the details panel.
 */
function IntentFlag({ mark, locale }: { mark: IntentMark | undefined; locale: Locale }) {
  if (!mark || (mark.drawn.length === 0 && mark.within.length === 0)) return null;
  const t = messages[locale].intent;
  const kind = mark.drawn.length > 0 ? "drawn" : "within";
  const said = [
    ...(mark.drawn.length > 0 ? [t.drawnBadge(mark.drawn.join(", "))] : []),
    ...(mark.within.length > 0 ? [t.withinBadge(mark.within.join(", "))] : []),
  ].join(" · ");
  return (
    <span className={`tim-intent-mark tim-intent-mark--${kind}`} data-contrast="icon" title={said} aria-hidden="true">
      <IntentIcon kind={kind} />
    </span>
  );
}

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
            <IntentFlag mark={data.intent} locale={locale} />
            <BudgetFlag said={data.budgets} locale={locale} />
            {data.perf && <PerfWords mark={data.perf} locale={locale} short />}
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
          <IntentFlag mark={data.intent} locale={locale} />
          <BudgetFlag said={data.budgets} locale={locale} />
          {data.perf && <PerfWords mark={data.perf} locale={locale} short />}
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
                {/* Holds a drawn component (ADR 0042): its icon in the list, as the changes are;
                    the card's name says which member holds which, and the tooltip too. */}
                {(() => {
                  const holds = data.intent?.members.find((m) => m.id === member.id);
                  return (
                    holds && (
                      <span
                        className={`tim-map-role__change tim-intent-mark tim-intent-mark--${holds.drawn ? "drawn" : "within"}`}
                        data-contrast="icon"
                        title={(holds.drawn ? t.intent.drawnBadge : t.intent.withinBadge)(holds.names.join(", "))}
                      >
                        <IntentIcon kind={holds.drawn ? "drawn" : "within"} />
                      </span>
                    )
                  );
                })()}
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
  const symbol = symbolOf(node);
  if (box.open) {
    return (
      <>
        <div
          className={quiet ? "tim-map-frame tim-map-frame--box tim-map--quiet" : "tim-map-frame tim-map-frame--box"}
          data-contrast="outline"
          data-role={symbol ? undefined : (node.role ?? undefined)}
          data-symbol={symbol?.kind}
          title={symbol ? t.symbolTip(t.symbolKinds[symbol.kind], symbol.signature) : undefined}
        >
          <div className="tim-map-frame__header" data-contrast="icon">
            {symbol ? <SymbolIcon kind={symbol.kind} /> : <RoleIcon role={node.role} />}
            <span className="tim-map-frame__title">{node.name}</span>
            <span className="tim-map-frame__meta">
              {symbol ? t.symbolKinds[symbol.kind] : node.role ? t.roles[node.role] : t.noRole} ·{" "}
              {t.items(node.children)}
            </span>
            {data.debt ? debt && <DebtWords debt={debt} locale={locale} short /> : flag}
            <IntentFlag mark={data.intent} locale={locale} />
            <BudgetFlag said={data.budgets} locale={locale} />
            {data.perf && <PerfWords mark={data.perf} locale={locale} short />}
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
        flag={
          (!data.debt && flag) || data.intent || data.budgets ? (
            <>
              {data.debt ? undefined : flag}
              <IntentFlag mark={data.intent} locale={locale} />
              <BudgetFlag said={data.budgets} locale={locale} />
            </>
          ) : undefined
        }
        badges={box.folded ? <Badge tone="neutral" label={t.folded(node.children)} /> : undefined}
        debt={
          debt ? (
            <DebtWords debt={debt} locale={locale} />
          ) : data.perf ? (
            <PerfWords mark={data.perf} locale={locale} />
          ) : undefined
        }
        debtLevel={debtLevel(debt)}
        symbol={symbol ?? undefined}
      />
      <Ends y={pinY.box} sides={sides} />
    </>
  );
}

export type LineEdge = Edge<
  {
    line: MapEdge;
    active: Role | "neutral" | null;
    locale: Locale;
    forbidden?: string[];
    /** What the forbidden word may lie over, computed once per layout (labelSpot.ts). */
    covering?: readonly Obstacle[];
    /** The Performances layer: the calls this line carries, and the busiest line's (ADR 0051). */
    traffic?: { calls: number; most: number };
  } & Quiet,
  "line"
>;

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
  const forbidden = data?.forbidden ?? null;
  const spot = useMemo(() => {
    if (!forbidden || !data) return undefined;
    const said = messages[data.locale].intent.forbiddenMark;
    // The label's size, generous: the badge's icon, its word, the review's icon and the padding.
    const size = { width: 48 + said.length * 8 + (review ? 24 : 0), height: 26 };
    return labelSpot(path, data.covering ?? [], size);
  }, [forbidden, data, review, path]);
  const classes = ["tim-edge", approximate ? "tim-edge--approximate" : "tim-edge--confirmed"];
  if (forbidden && !active) classes.push("tim-edge--forbidden");
  else if (review && !active) classes.push("tim-edge--review");
  if (active) classes.push("tim-edge--active");
  if (quiet && !active) classes.push("tim-edge--quiet");
  return (
    <>
      {active && <path d={path} className="tim-edge-casing" aria-hidden="true" />}
      <BaseEdge
        id={id}
        path={path}
        className={classes.join(" ")}
        // Thickness is the measured traffic's, and only its (ADR 0021, 0051).
        style={
          data?.traffic && data.traffic.calls > 0
            ? { strokeWidth: `calc(var(--tim-edge-width) * ${trafficWidth(data.traffic.calls, data.traffic.most)})` }
            : undefined
        }
        data-role={active && active !== "neutral" ? active : undefined}
        // Quieted by the filter, it steps back: its contrast is no longer its job.
        data-contrast={quiet && !active ? undefined : "stroke"}
      />
      {forbidden && data && (
        // Crossing a forbidden edge of the whiteboard (ADR 0042): the shape and the word, as on it,
        // over the boxes (the viewport's portal comes after them), where they cover least
        // (labelSpot): a word that a box would hide says nothing (review n° 6 of Phase 7).
        <ViewportPortal>
          <div
            className="tim-edge-flag tim-edge-flag--forbidden nodrag nopan"
            aria-hidden="true"
            style={{
              transform: `translate(-50%, -50%) translate(${spot?.x ?? labelX}px, ${spot?.y ?? labelY}px)`,
            }}
          >
            <Badge tone="violation" label={messages[data.locale].intent.forbiddenMark} />
            {/* New since the review too: both marks (review n° 3 of Phase 7, m-6); the review's as
                its icon alone, so that the label stays narrow and clear of the boxes beside it
                (review n° 5, m-1). Its words are in the line's name and the legend. */}
            {review && (
              <span className="tim-edge-flag__review" data-contrast="icon">
                <ChangeIcon kind={review.cycle ? "cycle" : "added"} />
              </span>
            )}
          </div>
        </ViewportPortal>
      )}
      {review && !forbidden && data && (
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
