/**
 * The bridge (spec « Experience and design », ADR 0032): the home view. It answers three
 * questions in a few seconds: where the plan stands, what happened since my last review, where
 * I should look.
 */
import { useState } from "react";

import { ChangeIcon, ToneIcon } from "../design/icons";
import type { Locale } from "../i18n";
import type { AgentChoice, LookItem, MilestoneView, ReviewBox, TrackingView } from "../protocol";
import type { TrackingMessages } from "./messages";
import { when } from "./TrackingPanel";

export interface BridgeActions {
  /** Show a box on the map (Q46): what leads to it, from the top. */
  onReveal: (target: string, path: string[]) => void;
  onOpenMap: () => void;
  /** The whiteboard (ADR 0037), where the developer draws the architecture they want. */
  onOpenWhiteboard?: () => void;
  /** The Performances panel (ADR 0051). */
  onOpenPerformance?: () => void;
  onMarkReviewed: () => void;
  /** `timonier init` for the agents chosen, without a terminal (T-118). */
  onConfigure: (agents?: string[]) => void;
}

/** How the setup from the bridge goes (T-118). */
export interface Setup {
  state: "idle" | "running" | "done" | "failed";
  /** What init said last, when it failed. */
  detail: string | null;
}

interface BridgeProps {
  t: TrackingMessages;
  locale: Locale;
  tracking: TrackingView;
  actions: BridgeActions | undefined;
  agents?: AgentChoice[];
  setup?: Setup;
}

export function Bridge({ t, locale, tracking, actions, agents, setup }: BridgeProps) {
  return (
    <section aria-label={t.bridgeLabel} className="tim-bridge">
      {!tracking.configured && (
        <div className="tim-bridge__setup" role="note">
          <p className="tim-bridge__setup-title">{t.notConfiguredTitle}</p>
          <p className="tim-tracking__quiet">{t.notConfiguredText}</p>
          {actions && <SetUp t={t} agents={agents} setup={setup} onConfigure={actions.onConfigure} />}
        </div>
      )}
      <PlanStatus t={t} tracking={tracking} />
      <SinceReview t={t} locale={locale} tracking={tracking} actions={actions} />
      <WhereToLook t={t} items={tracking.look} actions={actions} />
    </section>
  );
}

/**
 * The agents to set the project up for (T-118): each one a box to tick, those found on this
 * machine ticked; « Set up » runs `timonier init` for them, and says how it went.
 */
function SetUp({
  t,
  agents,
  setup = { state: "idle", detail: null },
  onConfigure,
}: {
  t: TrackingMessages;
  agents: AgentChoice[] | undefined;
  setup?: Setup;
  onConfigure: (agents?: string[]) => void;
}) {
  const [chosen, setChosen] = useState<string[] | null>(null);
  const ticked = chosen ?? (agents ?? []).filter((a) => a.found).map((a) => a.key);
  const running = setup.state === "running";
  const toggle = (key: string) => setChosen(ticked.includes(key) ? ticked.filter((k) => k !== key) : [...ticked, key]);
  return (
    <>
      {agents && (
        <fieldset className="tim-bridge__agents" disabled={running}>
          <legend className="tim-bridge__agents-legend">{t.agentsLegend}</legend>
          {agents.map((agent) => (
            <label key={agent.key} className="tim-bridge__agent">
              <input type="checkbox" checked={ticked.includes(agent.key)} onChange={() => toggle(agent.key)} />
              <span>{agent.label}</span>
              {!agent.found && <span className="tim-tracking__quiet"> · {t.agentNotFound}</span>}
            </label>
          ))}
        </fieldset>
      )}
      <div className="tim-bridge__actions">
        <button
          type="button"
          className="tim-button tim-button--primary"
          disabled={running || (agents !== undefined && ticked.length === 0)}
          onClick={() => onConfigure(agents ? ticked : undefined)}
        >
          {running ? t.configuring : t.configure}
        </button>
      </div>
      <p className="tim-tracking__quiet" role="status">
        {setup.state === "failed" ? t.configureFailed(setup.detail) : setup.state === "done" ? t.configureDone : ""}
      </p>
    </>
  );
}

/**
 * The milestone where the work is (review n° 2 of Phase 6): the one with a task in progress;
 * else the last one with a task done, unless it is finished, then the next one not finished; else
 * the first not finished. A task left over in an old milestone does not bring it back.
 */
export function currentMilestone(milestones: MilestoneView[]): MilestoneView | undefined {
  // As the hook counts (.claude/hooks/session-start.mjs): a milestone whose tasks are all done or
  // dropped is finished.
  const unfinished = (m: MilestoneView) => m.done < m.of;
  const underway = milestones.find((m) => m.tasks.some((t) => t.status === "inProgress"));
  if (underway) return underway;
  const last = milestones.findLastIndex((m) => m.done > 0);
  if (last >= 0) {
    const reached = milestones[last];
    if (reached && unfinished(reached)) return reached;
    const next = milestones.slice(last + 1).find(unfinished);
    if (next) return next;
  }
  return milestones.find(unfinished) ?? milestones.at(-1);
}

function PlanStatus({ t, tracking }: { t: TrackingMessages; tracking: TrackingView }) {
  const milestone = currentMilestone(tracking.milestones);
  const session = tracking.sessions.find((s) => s.id === tracking.open);
  const todoDone = tracking.todo.filter((item) => item.done).length;
  return (
    <div className="tim-bridge__part">
      <h2 className="tim-tracking__heading">{t.planHeading}</h2>
      {milestone ? (
        <>
          <p className="tim-bridge__line">
            <span className="tim-milestone__title">{milestone.title}</span>
            <span className="tim-milestone__count">{t.progress(milestone.done, milestone.of)}</span>
          </p>
          <span
            className="tim-progress"
            role="progressbar"
            aria-label={t.progressLabel(milestone.title, milestone.done, milestone.of)}
            aria-valuemin={0}
            aria-valuemax={milestone.of}
            aria-valuenow={milestone.done}
          >
            <span
              className="tim-progress__fill"
              style={{ width: `${milestone.of === 0 ? 0 : Math.round((milestone.done / milestone.of) * 100)}%` }}
              data-contrast="fill"
            />
          </span>
          <Underway t={t} milestone={milestone} />
        </>
      ) : (
        <p className="tim-tracking__quiet">{t.noPlanTitle}</p>
      )}
      <p className="tim-tracking__quiet">
        {session
          ? `${t.currentSession(session.objective)} · ${t.todoProgress(todoDone, tracking.todo.length)}`
          : t.noOpenSession}
      </p>
    </div>
  );
}

function Underway({ t, milestone }: { t: TrackingMessages; milestone: MilestoneView }) {
  const count = (state: string) => milestone.tasks.filter((task) => task.status === state).length;
  const words = t.underway(count("inProgress"), count("blocked"));
  return words ? <p className="tim-tracking__quiet">{words}</p> : null;
}

function SinceReview({
  t,
  locale,
  tracking,
  actions,
}: {
  t: TrackingMessages;
  locale: Locale;
  tracking: TrackingView;
  actions: BridgeActions | undefined;
}) {
  const { review, debt } = tracking;
  const since = review.since;
  const counts = [
    [review.added.length, t.newBoxes],
    [review.modified.length, t.modifiedBoxes],
    [review.removed.length, t.removedBoxes],
    [review.dependenciesAdded.length, t.newDependencies],
    [review.dependenciesRemoved.length, t.goneDependencies],
  ] as const;
  const changes = counts.filter(([count]) => count > 0).map(([count, words]) => words(count));
  const work = [
    tracking.sessionsSinceReview > 0 ? t.sessionsSince(tracking.sessionsSinceReview) : "",
    debt ? t.toRead(debt.files, debt.untracked) : "",
  ].filter(Boolean);
  return (
    <div className="tim-bridge__part">
      <h2 className="tim-tracking__heading">
        {since ? t.sinceReview(when(new Date(since.timestampMs).toISOString(), locale)) : t.noReviewHeading}
      </h2>
      {since ? (
        <>
          <p>{changes.length > 0 ? changes.join(" · ") : t.nothingChanged}</p>
          {work.length > 0 && <p className="tim-tracking__quiet">{work.join(" · ")}</p>}
        </>
      ) : (
        <>
          <p className="tim-tracking__quiet">{t.noReviewText}</p>
          {work.length > 0 && <p className="tim-tracking__quiet">{work.join(" · ")}</p>}
        </>
      )}
      {actions && (
        <div className="tim-bridge__actions">
          <button type="button" className="tim-button tim-button--primary" onClick={actions.onOpenMap}>
            {t.openMap}
          </button>
          {actions.onOpenWhiteboard && (
            <button type="button" className="tim-button" onClick={actions.onOpenWhiteboard}>
              {t.openWhiteboard}
            </button>
          )}
          <button type="button" className="tim-button" onClick={actions.onMarkReviewed}>
            {t.markReviewed}
          </button>
          {actions.onOpenPerformance && (
            <button type="button" className="tim-button" onClick={actions.onOpenPerformance}>
              {t.openPerformance}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

function WhereToLook({
  t,
  items,
  actions,
}: {
  t: TrackingMessages;
  items: LookItem[];
  actions: BridgeActions | undefined;
}) {
  return (
    <div className="tim-bridge__part">
      <h2 className="tim-tracking__heading">{t.lookHeading}</h2>
      {items.length === 0 ? (
        <p className="tim-tracking__quiet">{t.lookEmpty}</p>
      ) : (
        <ul className="tim-plain tim-look">
          {items.map((item) => (
            <li key={keyOf(item)}>
              <Look t={t} item={item} actions={actions} />
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function keyOf(item: LookItem): string {
  switch (item.kind) {
    case "problem":
      return `problem-${item.file}`;
    case "blocked":
      return `blocked-${item.task}`;
    case "cycle":
      return `cycle-${item.boxes.map((b) => b.id).join()}`;
    case "debt":
      return `debt-${item.item.id}`;
  }
}

function named(item: ReviewBox): string {
  return item.within ? `${item.within} › ${item.name}` : item.name;
}

/** One thing to look at: an alert in words, or a box that opens the map on itself. */
function Look({ t, item, actions }: { t: TrackingMessages; item: LookItem; actions: BridgeActions | undefined }) {
  switch (item.kind) {
    case "problem":
      return (
        <p className="tim-look__item" data-contrast="icon">
          <ToneIcon tone="attention" />
          <span>{t.lookProblem(item.file)}</span>
        </p>
      );
    case "blocked":
      return (
        <p className="tim-look__item" data-contrast="icon">
          <ToneIcon tone="attention" />
          <span>{t.lookBlocked(item.task, item.title)}</span>
        </p>
      );
    case "cycle": {
      const first = item.boxes[0];
      return (
        <BoxButton
          label={t.lookCycle([...item.boxes.map(named), first ? named(first) : ""].join(" → "))}
          detail={null}
          hint={t.revealHint}
          onSelect={first ? () => actions?.onReveal(first.id, item.path) : undefined}
          icon={<ChangeIcon kind="cycle" />}
        />
      );
    }
    case "debt":
      return (
        <BoxButton
          label={named(item.item)}
          detail={`${t.lookDebt(item.debt.files)}${item.debt.untracked > 0 ? ` · ${t.untracked}` : ""}`}
          hint={t.revealHint}
          onSelect={() => actions?.onReveal(item.item.id, item.path)}
          untracked={item.debt.untracked > 0}
        />
      );
  }
}

function BoxButton({
  label,
  detail,
  hint,
  onSelect,
  icon,
  untracked = false,
}: {
  label: string;
  detail: string | null;
  hint: string;
  onSelect: (() => void) | undefined;
  icon?: React.ReactNode;
  untracked?: boolean;
}) {
  return (
    <button
      type="button"
      className="tim-look__box"
      data-untracked={untracked}
      onClick={onSelect}
      title={hint}
      data-contrast={icon ? "icon" : undefined}
    >
      {icon}
      <span className="tim-look__name">{label}</span>
      {detail && <span className="tim-look__detail">{detail}</span>}
    </button>
  );
}
