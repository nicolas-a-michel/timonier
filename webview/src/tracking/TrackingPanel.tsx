/**
 * The panels of the tracking (PLAN.md 6.6, ADR 0031): the plan, the todo of the open session
 * and the journal of the sessions, as the daemon sends them whole at each change.
 */
import type { ReactNode } from "react";

import { Badge, EmptyState } from "../design/components";
import { ChangeIcon, ToneIcon } from "../design/icons";
import type { Locale } from "../i18n";
import type {
  ChangedBox,
  ChangedLink,
  DaemonState,
  MilestoneView,
  SessionChanges,
  SessionView,
  TaskState,
  TaskView,
  TodoItemView,
  TrackingView,
} from "../protocol";
import { Bridge, type BridgeActions } from "./Bridge";
import { trackingMessages, type TrackingMessages } from "./messages";

/** One part, or `all`: the view of the side bar, the plan and the todo beside the sessions. */
export type PanelKind = "plan" | "todo" | "sessions" | "all" | "bridge";

export interface DaemonInfo {
  state: DaemonState;
  detail: string | null;
}

export interface TrackingPanelProps {
  locale: Locale;
  panel: PanelKind;
  /** `undefined` until the daemon answered. */
  tracking: TrackingView | undefined;
  daemon: DaemonInfo | undefined;
  /** What the bridge's buttons and boxes do (ADR 0032). */
  actions?: BridgeActions;
}

export function TrackingPanel({ locale, panel, tracking, daemon, actions }: TrackingPanelProps) {
  const t = trackingMessages[locale];
  if (!tracking) return <Waiting t={t} daemon={daemon} />;
  const titles = new Map(tracking.milestones.flatMap((m) => m.tasks.map((task) => [task.id, task.title] as const)));
  return (
    <main className="tim-tracking" data-panel={panel}>
      {tracking.problems.map((problem) => (
        <p key={problem.file} className="tim-tracking__problem" data-contrast="icon">
          <ToneIcon tone="attention" />
          <span>
            <strong>{t.problem(problem.file)}</strong> {t.problemText}
            <span className="tim-tracking__detail">{problem.message}</span>
          </span>
        </p>
      ))}
      {panel === "plan" && <Plan t={t} milestones={tracking.milestones} />}
      {panel === "todo" && <Todo t={t} tracking={tracking} titles={titles} locale={locale} />}
      {panel === "sessions" && <Sessions t={t} tracking={tracking} titles={titles} locale={locale} />}
      {panel === "bridge" && <Bridge t={t} locale={locale} tracking={tracking} actions={actions} />}
      {panel === "all" && (
        // Two columns side by side when the side bar is wide enough, one under the other
        // otherwise (tracking.css).
        <div className="tim-tracking__columns">
          <div className="tim-tracking__column">
            <Heading text={t.planLabel}>
              <Plan t={t} milestones={tracking.milestones} />
            </Heading>
            <Heading text={t.todo}>
              <Todo t={t} tracking={tracking} titles={titles} locale={locale} />
            </Heading>
          </div>
          <div className="tim-tracking__column">
            <Heading text={t.sessionsLabel}>
              <Sessions t={t} tracking={tracking} titles={titles} locale={locale} />
            </Heading>
          </div>
        </div>
      )}
    </main>
  );
}

/** A part of the view under its header, as VSCode heads the views of a side bar. */
function Heading({ text, children }: { text: string; children: ReactNode }) {
  return (
    <div className="tim-tracking__part">
      <h2 className="tim-tracking__heading">{text}</h2>
      {children}
    </div>
  );
}

function Waiting({ t, daemon }: { t: TrackingMessages; daemon: DaemonInfo | undefined }) {
  if (daemon?.state === "unavailable") {
    return <EmptyState title={t.unavailableTitle} text={daemon.detail ?? ""} live />;
  }
  if (daemon?.state === "restarting") {
    return <EmptyState title={t.restartingTitle} text={t.restartingText} detail={daemon.detail} live />;
  }
  return <EmptyState title={t.startingTitle} text={t.startingText} live />;
}

// The plan --------------------------------------------------------------------------------------

function Plan({ t, milestones }: { t: TrackingMessages; milestones: MilestoneView[] }) {
  if (milestones.length === 0) return <EmptyState title={t.noPlanTitle} text={t.noPlanText} />;
  return (
    <section aria-label={t.planLabel} className="tim-tracking__list">
      {milestones.map((milestone) => (
        // A milestone done folds away; the others stay open.
        <details
          key={milestone.id}
          className="tim-milestone"
          open={milestone.of === 0 || milestone.done < milestone.of}
        >
          <summary className="tim-milestone__summary">
            <Chevron />
            <span className="tim-milestone__title">{milestone.title}</span>
            <span className="tim-milestone__count">{t.progress(milestone.done, milestone.of)}</span>
            <Progress
              done={milestone.done}
              of={milestone.of}
              label={t.progressLabel(milestone.title, milestone.done, milestone.of)}
            />
          </summary>
          {milestone.tasks.length === 0 ? (
            <p className="tim-tracking__quiet">{t.emptyMilestone}</p>
          ) : (
            <ul className="tim-tasks">
              {milestone.tasks.map((task) => (
                <Task key={task.id} t={t} task={task} />
              ))}
            </ul>
          )}
        </details>
      ))}
    </section>
  );
}

/** What says a summary folds and unfolds: it turns when open (tracking.css). */
function Chevron() {
  return (
    <svg
      className="tim-icon tim-chevron"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      aria-hidden="true"
    >
      <path d="M6 4l4 4-4 4" />
    </svg>
  );
}

/** A bar whose length is the share of tasks done; its name says it in words. */
function Progress({ done, of, label }: { done: number; of: number; label: string }) {
  const share = of === 0 ? 0 : Math.round((done / of) * 100);
  return (
    <span
      className="tim-progress"
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={of}
      aria-valuenow={done}
    >
      <span className="tim-progress__fill" style={{ width: `${share}%` }} data-contrast="fill" />
    </span>
  );
}

function Task({ t, task }: { t: TrackingMessages; task: TaskView }) {
  const detail = task.criterion !== null || task.notes.length > 0;
  const head = (
    <>
      <StateIcon state={task.status} />
      <span className="tim-task__title">{task.title}</span>
      <span className="tim-task__state">{t.states[task.status]}</span>
      <span className="tim-task__id">{task.id}</span>
    </>
  );
  return (
    <li className="tim-task" data-state={task.status}>
      {detail ? (
        <details>
          <summary className="tim-task__head" data-contrast="icon">
            {head}
            <Chevron />
          </summary>
          <div className="tim-task__detail">
            {task.criterion && (
              <p>
                <span className="tim-tracking__term">{t.criterion}</span> {task.criterion}
              </p>
            )}
            {task.notes.length > 0 && (
              <>
                <p className="tim-tracking__term">{t.notes}</p>
                <ul className="tim-notes">
                  {task.notes.map((note) => (
                    <li key={`${note.at}-${note.text}`}>
                      {note.text} <span className="tim-tracking__quiet">{note.session}</span>
                    </li>
                  ))}
                </ul>
              </>
            )}
          </div>
        </details>
      ) : (
        <div className="tim-task__head" data-contrast="icon">
          {head}
        </div>
      )}
    </li>
  );
}

/**
 * A shape per state, never a colour alone: a ring to do, a half disc in progress, a triangle
 * blocked (attention), a ticked disc done, a struck ring dropped.
 */
const STATE_SHAPES: Record<TaskState, ReactNode> = {
  todo: <circle cx="8" cy="8" r="5" />,
  inProgress: (
    <>
      <circle cx="8" cy="8" r="5" />
      <path d="M8 3a5 5 0 0 1 0 10z" fill="currentColor" />
    </>
  ),
  blocked: (
    <>
      <path d="M8 2.2l6 10.8H2z" />
      <path d="M8 6.5v3M8 11.3v.1" />
    </>
  ),
  done: (
    <>
      <circle cx="8" cy="8" r="6" />
      <path d="M5.2 8.2l1.9 1.9 3.7-4" />
    </>
  ),
  dropped: (
    <>
      <circle cx="8" cy="8" r="5" />
      <path d="M4.5 11.5l7-7" />
    </>
  ),
};

function StateIcon({ state }: { state: TaskState }) {
  return (
    <svg
      className="tim-icon tim-state"
      data-state={state}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      aria-hidden="true"
    >
      {STATE_SHAPES[state]}
    </svg>
  );
}

// The todo -------------------------------------------------------------------------------------

interface WithSessions {
  t: TrackingMessages;
  tracking: TrackingView;
  titles: Map<string, string>;
  locale: Locale;
}

function Todo({ t, tracking, titles, locale }: WithSessions) {
  const session = tracking.sessions.find((s) => s.id === tracking.open);
  if (!session) return <EmptyState title={t.noSessionTitle} text={t.noSessionText} />;
  return (
    <section aria-label={t.todoLabel} className="tim-tracking__list">
      <header className="tim-session__head">
        <p className="tim-tracking__quiet">{t.currentSession(session.objective)}</p>
        <p className="tim-tracking__quiet">
          {t.by(session.agent)} · {when(session.startedAt, locale)}
        </p>
      </header>
      <TodoList t={t} items={tracking.todo} titles={titles} empty={t.emptyTodo} />
    </section>
  );
}

function TodoList({
  t,
  items,
  titles,
  empty,
}: {
  t: TrackingMessages;
  items: TodoItemView[];
  titles: Map<string, string>;
  empty: string;
}) {
  if (items.length === 0) return <p className="tim-tracking__quiet">{empty}</p>;
  const done = items.filter((item) => item.done).length;
  return (
    <>
      <p className="tim-todo__count">{t.todoProgress(done, items.length)}</p>
      <ul className="tim-todo">
        {items.map((item) => (
          <li key={item.id} className="tim-todo__item" data-done={item.done} data-contrast="icon">
            <svg
              className="tim-icon tim-todo__box"
              viewBox="0 0 16 16"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.5"
              aria-hidden="true"
            >
              <rect x="2.5" y="2.5" width="11" height="11" rx="2" />
              {item.done && <path d="M5 8.2l2 2 4-4.4" />}
            </svg>
            <span className="tim-visually-hidden">{`${item.done ? t.itemDone : t.itemOpen}, `}</span>
            <span className="tim-todo__text">
              {item.text}
              {item.task && (
                <span className="tim-todo__task">
                  {item.task}
                  {titles.has(item.task) ? ` · ${titles.get(item.task)}` : ""}
                </span>
              )}
            </span>
          </li>
        ))}
      </ul>
    </>
  );
}

// The journal ----------------------------------------------------------------------------------

function Sessions({ t, tracking, titles, locale }: WithSessions) {
  if (tracking.sessions.length === 0) {
    return <EmptyState title={t.noSessionsTitle} text={t.noSessionsText} />;
  }
  return (
    <section aria-label={t.sessionsLabel} className="tim-tracking__list">
      {tracking.sessions.map((session, index) => (
        <Session key={session.id} t={t} session={session} titles={titles} locale={locale} first={index === 0} />
      ))}
      {tracking.sessionsTotal > tracking.sessions.length && (
        <p className="tim-tracking__quiet">{t.latest(tracking.sessions.length, tracking.sessionsTotal)}</p>
      )}
    </section>
  );
}

function Session({
  t,
  session,
  titles,
  locale,
  first,
}: {
  t: TrackingMessages;
  session: SessionView;
  titles: Map<string, string>;
  locale: Locale;
  first: boolean;
}) {
  const changes = session.changes;
  const meta = [t.by(session.agent), when(session.startedAt, locale)];
  if (changes) meta.push(t.files(changes.files));
  return (
    // The open session, or the latest, comes unfolded.
    <details className="tim-session" open={session.open || first}>
      <summary className="tim-session__summary">
        <Chevron />
        <span className="tim-session__objective">{session.objective}</span>
        {session.open && (
          <span className="tim-session__open" data-contrast="icon">
            <ChangeIcon kind="modified" />
            {t.open}
          </span>
        )}
        <span className="tim-tracking__quiet tim-session__meta">{meta.join(" · ")}</span>
      </summary>
      <div className="tim-session__body">
        {session.summary && <Part title={t.summary}>{<p>{session.summary}</p>}</Part>}
        <Part title={t.tasks}>
          {session.tasks.length === 0 ? (
            <p className="tim-tracking__quiet">{t.noTasks}</p>
          ) : (
            <ul className="tim-plain">
              {session.tasks.map((task) => (
                <li key={task}>
                  <span className="tim-todo__task">{task}</span> {titles.get(task) ?? ""}
                </li>
              ))}
            </ul>
          )}
        </Part>
        <Changes t={t} changes={changes} live={session.open} />
        <Part title={t.todo}>
          {session.open ? (
            // Its items are in the Todo panel, live: here, only how far it is.
            <p className="tim-tracking__quiet">
              {t.todoProgress(session.todo.filter((item) => item.done).length, session.todo.length)} · {t.seeTodo}
            </p>
          ) : (
            <TodoList t={t} items={session.todo} titles={titles} empty={t.emptyTodo} />
          )}
        </Part>
        <Part title={t.tests}>
          {session.budgets.length > 0 ? (
            // The budgets the last runs exceeded when it ended (ADR 0052): a violation, said so.
            <ul className="tim-plain tim-session__budgets">
              {session.budgets.map((said) => (
                <li key={said} className="tim-session__budget">
                  <Badge tone="violation" label={t.budgetExceeded} />
                  <span>{said}</span>
                </li>
              ))}
            </ul>
          ) : (
            <p className="tim-tracking__quiet">{session.open ? t.testsAtEnd : t.noTests}</p>
          )}
        </Part>
      </div>
    </details>
  );
}

function Part({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="tim-session__part">
      <h3 className="tim-tracking__term">{title}</h3>
      {children}
    </div>
  );
}

function Changes({ t, changes, live }: { t: TrackingMessages; changes: SessionChanges | null; live: boolean }) {
  const title = live ? `${t.components} (${t.live.toLowerCase()})` : t.components;
  if (!changes) {
    return (
      <Part title={title}>
        <p className="tim-tracking__quiet">{live ? t.noChanges : t.unknownChanges}</p>
      </Part>
    );
  }
  const boxes: [ChangedBox, "added" | "modified" | "removed"][] = [
    ...changes.added.map((b) => [b, "added"] as [ChangedBox, "added"]),
    ...changes.modified.map((b) => [b, "modified"] as [ChangedBox, "modified"]),
    ...changes.removed.map((b) => [b, "removed"] as [ChangedBox, "removed"]),
  ];
  const links: [ChangedLink, "linked" | "unlinked"][] = [
    ...changes.linked.map((l) => [l, "linked"] as [ChangedLink, "linked"]),
    ...changes.unlinked.map((l) => [l, "unlinked"] as [ChangedLink, "unlinked"]),
  ];
  return (
    <>
      <Part title={title}>
        {boxes.length === 0 ? (
          <p className="tim-tracking__quiet">{changes.files > 0 ? t.files(changes.files) : t.noChanges}</p>
        ) : (
          <ul className="tim-plain tim-changes">
            {boxes.map(([item, change]) => (
              <li key={`${change}-${item.id}`} data-change={change} data-contrast="icon">
                <ChangedIcon change={change} />
                <span className="tim-changes__name">{named(item)}</span>
                <span className="tim-tracking__quiet">{t[change]}</span>
              </li>
            ))}
          </ul>
        )}
      </Part>
      {links.length > 0 && (
        <Part title={t.dependencies}>
          <ul className="tim-plain tim-changes">
            {links.map(([link, change]) => (
              <li key={`${change}-${link.from.id}-${link.to.id}`} data-change={change}>
                <span className="tim-changes__name">
                  {named(link.from)} → {named(link.to)}
                </span>
                <span className="tim-tracking__quiet">{t[change]}</span>
              </li>
            ))}
          </ul>
        </Part>
      )}
    </>
  );
}

/** The marks of the review diff on the map (ADR 0024): a star new, a dot modified; a struck
 * ring removed. */
function ChangedIcon({ change }: { change: "added" | "modified" | "removed" }) {
  if (change !== "removed") return <ChangeIcon kind={change} />;
  return <StateIcon state="dropped" />;
}

/** A box's name, with the box that holds it when it has one. */
function named(item: ChangedBox): string {
  return item.within ? `${item.within} › ${item.name}` : item.name;
}

/** A date for a person, in the IDE's language and the local time. */
export function when(at: string, locale: Locale): string {
  const date = new Date(at);
  if (Number.isNaN(date.getTime())) return at;
  return new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(date);
}
