/**
 * The panels of the tracking (PLAN.md 6.6, ADR 0031): what each says of the plan, the todo and
 * the journal, in words and shapes, and what it teaches when there is nothing yet.
 */
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { NO_REVIEW, type TaskView, type TrackingView } from "../protocol";
import { SAMPLE_TRACKING } from "./sample";
import { currentMilestone } from "./Bridge";
import { TrackingPanel, when, type PanelKind } from "./TrackingPanel";

const EMPTY: TrackingView = {
  milestones: [],
  open: null,
  todo: [],
  sessions: [],
  sessionsTotal: 0,
  problems: [],
  review: NO_REVIEW,
  debt: null,
  sessionsSinceReview: 0,
  look: [],
  configured: false,
};

function render(panel: PanelKind, tracking: TrackingView | undefined = SAMPLE_TRACKING, locale: "en" | "fr" = "en") {
  return renderToStaticMarkup(
    <TrackingPanel locale={locale} panel={panel} tracking={tracking} daemon={{ state: "connected", detail: null }} />,
  );
}

describe("the plan panel", () => {
  const html = render("plan");

  it("shows each milestone with its computed progress, in words and as a bar", () => {
    expect(html).toContain("Phase 6 — MCP, init and tracking");
    expect(html).toContain("2 of 5 done");
    expect(html).toContain('role="progressbar"');
    expect(html).toContain('aria-label="Phase 6 — MCP, init and tracking: 2 of 5 tasks done"');
    expect(html).toContain('aria-valuenow="2"');
    expect(html).toContain('style="width:40%"');
  });

  it("folds a milestone that is done, and leaves the others open", () => {
    const [done, current] = html.split('<details class="tim-milestone"').slice(1);
    expect(done?.startsWith(">")).toBe(true);
    expect(current?.startsWith(' open="">')).toBe(true);
  });

  it("says each state with a shape and words, never a colour alone", () => {
    for (const state of ["todo", "inProgress", "blocked", "done", "dropped"]) {
      expect(html).toContain(`<svg class="tim-icon tim-state" data-state="${state}"`);
    }
    expect(html).toContain('<span class="tim-task__state">in progress</span>');
    expect(html).toContain('<span class="tim-task__state">blocked</span>');
    expect(html).toContain('<span class="tim-task__state">dropped</span>');
  });

  it("unfolds the criterion and the notes of a task that has them", () => {
    expect(html).toContain("Done when</span> Heat by component on the map, as a layer");
    expect(html).toContain("Waits for the bridge.");
  });

  it("teaches what fills it when there is no plan", () => {
    const empty = render("plan", EMPTY);
    expect(empty).toContain("No plan yet");
    expect(empty).toContain("plan_add_task");
  });
});

describe("the todo panel", () => {
  const html = render("todo");

  it("shows the open session's todo, done and to do, with its task", () => {
    expect(html).toContain("Cache the extraction of unchanged files");
    expect(html).toContain("Session in progress: Cache the extraction of unchanged files");
    expect(html).toContain("by claude-code");
    expect(html).toContain("2 of 4 done");
    expect(html).toContain('<li class="tim-todo__item" data-done="true"');
    expect(html).toContain('<span class="tim-visually-hidden">to do, </span>');
    expect(html).toContain("T-6 · Extraction cache");
  });

  it("says how a session opens when none is", () => {
    const empty = render("todo", { ...SAMPLE_TRACKING, open: null, todo: [] });
    expect(empty).toContain("No session open");
    expect(empty).toContain("session_start");
  });
});

describe("the sessions panel", () => {
  const html = render("sessions");

  it("lists the sessions, the open one first, unfolded and live", () => {
    const objectives = [...html.matchAll(/<span class="tim-session__objective">([^<]+)</g)].map((m) => m[1]);
    expect(objectives).toEqual([
      "Cache the extraction of unchanged files",
      "Count the review debt of each box",
      "Record what changes with no session open",
    ]);
    expect(html).toContain("in progress");
    expect(html).toContain("Components touched (since its start, live)");
    expect(html.split('<details class="tim-session" open="">').length - 1).toBe(1);
  });

  it("says the objective, the tasks, the components touched, the dependencies, the todo and the tests", () => {
    expect(html).toContain("The map shows the files to read again");
    expect(html).toContain("12 files");
    expect(html).toContain("timonier-daemon › debt");
    expect(html).toContain('data-change="added"');
    expect(html).toContain("timonier-daemon → timonier-store");
    // The journal keeps the budgets the last runs exceeded at a session's end (ADR 0052).
    expect(html).toContain("The budgets of the whiteboard are checked on the last runs when it ends.");
    expect(html).toContain("N-2 on Store: p95 7.40 ms &gt; 5 ms");
    expect(html).toContain("No budget of the whiteboard exceeded when it ended.");
  });

  it("leaves the open session's items to the Todo panel, and lists those of the others", () => {
    expect(html).toContain("2 of 4 done · the items are under Todo");
    expect(html).not.toContain("Drop entries of a changed pack version");
    expect(html).toContain("Compute the debt windows");
  });

  it("says when a session's changes were not recorded", () => {
    expect(html).toContain("Not recorded for this session.");
  });

  it("says when only the latest sessions came", () => {
    expect(render("sessions", { ...SAMPLE_TRACKING, sessionsTotal: 80 })).toContain("The latest 3 of 80 sessions.");
  });

  it("teaches what lands there when there is nothing yet", () => {
    expect(render("sessions", EMPTY)).toContain("No session yet");
  });
});

describe("the whole view (ADR 0031)", () => {
  it("holds the plan and the todo in one column, the sessions in the other, each under its heading", () => {
    const html = render("all");
    const columns = html.split('<div class="tim-tracking__column">').slice(1);
    expect(columns).toHaveLength(2);
    expect(columns[0]).toContain('<h2 class="tim-tracking__heading">Plan</h2>');
    expect(columns[0]).toContain('<h2 class="tim-tracking__heading">Todo</h2>');
    expect(columns[0]).toContain("2 of 5 done");
    expect(columns[0]).toContain("Key the cache by content hash");
    expect(columns[1]).toContain('<h2 class="tim-tracking__heading">Sessions</h2>');
    expect(columns[1]).toContain("Count the review debt of each box");
  });
});

describe("the bridge (ADR 0032)", () => {
  const html = render("bridge");

  it("says where the plan stands: the milestone under way, its progress, the session", () => {
    expect(html).toContain("Phase 6 — MCP, init and tracking");
    expect(html).toContain("2 of 5 done");
    expect(html).toContain("1 in progress · 1 blocked");
    expect(html).toContain("Session in progress: Cache the extraction of unchanged files · 2 of 4 done");
  });

  it("says what happened since the review, and offers the map and the review mark", () => {
    expect(html).toMatch(/Since your review of [^<]*2026/);
    expect(html).toContain("1 new box · 3 modified · 1 dependency added");
    expect(html).toContain("2 agent sessions · 14 files to read again, 2 untracked");
    const actions = {
      onReveal: () => undefined,
      onOpenMap: () => undefined,
      onOpenWhiteboard: () => undefined,
      onMarkReviewed: () => undefined,
      onConfigure: () => undefined,
    };
    const withActions = renderToStaticMarkup(
      <TrackingPanel locale="en" panel="bridge" tracking={SAMPLE_TRACKING} daemon={undefined} actions={actions} />,
    );
    expect(withActions).toContain(">Open Map</button>");
    expect(withActions).toContain(">Mark as reviewed</button>");
    expect(withActions).toContain(">Whiteboard</button>");
  });

  it("says where to look: the alerts in words, then the boxes, untracked marked", () => {
    const look = html.slice(html.indexOf("Where to look"));
    expect(look.indexOf("Blocked: T-7 timonier init")).toBeLessThan(look.indexOf("timonier-daemon"));
    expect(look).toContain('data-untracked="true"');
    expect(look).toContain("9 files to read again · untracked");
    expect(look).toContain("webview › map");
  });

  it("offers to set the project up while it is not (Q52), and says nothing of it after", () => {
    expect(html).not.toContain("not set up for your agents");
    const fresh = renderToStaticMarkup(
      <TrackingPanel
        locale="en"
        panel="bridge"
        tracking={EMPTY}
        daemon={undefined}
        actions={{
          onReveal: () => undefined,
          onOpenMap: () => undefined,
          onMarkReviewed: () => undefined,
          onConfigure: () => undefined,
        }}
      />,
    );
    expect(fresh).toContain("This project is not set up for your agents");
    expect(fresh).toContain(">Set up</button>");
    expect(fresh.indexOf("not set up")).toBeLessThan(fresh.indexOf("No review yet"));
  });

  it("offers the agents to set up, those found ticked, and says how the setup went (T-118)", () => {
    const agents = [
      { key: "claude", label: "Claude Code", found: true, why: "~/.claude exists" },
      { key: "cursor", label: "Cursor", found: false, why: null },
    ];
    const actions = {
      onReveal: () => undefined,
      onOpenMap: () => undefined,
      onMarkReviewed: () => undefined,
      onConfigure: () => undefined,
    };
    const offered = renderToStaticMarkup(
      <TrackingPanel
        locale="en"
        panel="bridge"
        tracking={EMPTY}
        daemon={undefined}
        actions={actions}
        agents={agents}
      />,
    );
    expect(offered).toContain("<legend");
    expect(offered).toContain("Agents to set up");
    expect(offered).toMatch(/<input type="checkbox" checked=""\/><span>Claude Code<\/span>/);
    expect(offered).toMatch(/<input type="checkbox"\/><span>Cursor<\/span><span[^>]*> · not found here<\/span>/);
    const failed = renderToStaticMarkup(
      <TrackingPanel
        locale="fr"
        panel="bridge"
        tracking={EMPTY}
        daemon={undefined}
        actions={actions}
        agents={agents}
        setup={{ state: "failed", detail: "cannot write AGENTS.md" }}
      />,
    );
    expect(failed).toContain("La configuration a échoué : cannot write AGENTS.md");
  });

  it("teaches what comes before the first review, and when nothing is to read", () => {
    const empty = render("bridge", EMPTY);
    expect(empty).toContain("No review yet");
    expect(empty).toContain("Nothing to read again.");
    expect(empty).toContain("No agent session open.");
  });

  it("speaks French", () => {
    const fr = render("bridge", SAMPLE_TRACKING, "fr");
    expect(fr).toContain("Où regarder");
    expect(fr).toContain("14 fichiers à relire, dont 2 non suivis");
  });
});

describe("the milestone where the work is (review n° 2 of Phase 6)", () => {
  const milestone = (id: string, statuses: TaskView["status"][]) => ({
    id,
    title: id,
    done: statuses.filter((s) => s === "done").length,
    of: statuses.filter((s) => s !== "dropped").length,
    tasks: statuses.map((status, i) => ({
      id: `${id}-${i}`,
      title: "t",
      criterion: null,
      status,
      sessions: [],
      notes: [],
    })),
  });

  it("is the one with a task in progress", () => {
    expect(currentMilestone([milestone("M-1", ["todo"]), milestone("M-2", ["done", "inProgress"])])?.id).toBe("M-2");
  });

  it("is not an old one where a task was left to do", () => {
    const old = milestone("M-1", ["done", "todo"]);
    expect(currentMilestone([old, milestone("M-2", ["done", "todo"])])?.id).toBe("M-2");
    // The last one reached is finished: the next one.
    expect(currentMilestone([old, milestone("M-2", ["done"]), milestone("M-3", ["todo"])])?.id).toBe("M-3");
    // Nothing done yet: the first not finished.
    expect(currentMilestone([milestone("M-1", ["todo"]), milestone("M-2", ["todo"])])?.id).toBe("M-1");
  });
});

describe("every panel", () => {
  it("names a tracking file that does not read, and what to do", () => {
    const broken = {
      ...EMPTY,
      problems: [{ file: "plan.json", message: "expected value at line 1 column 3" }],
    };
    const html = render("plan", broken);
    expect(html).toContain(".timonier/plan.json cannot be read");
    expect(html).toContain("restore it with git");
    expect(html).toContain("expected value at line 1 column 3");
  });

  it("waits for the daemon, then says why it cannot start", () => {
    const waiting = renderToStaticMarkup(
      <TrackingPanel locale="en" panel="todo" tracking={undefined} daemon={{ state: "starting", detail: null }} />,
    );
    expect(waiting).toContain("Starting Timonier");
    const unavailable = renderToStaticMarkup(
      <TrackingPanel
        locale="en"
        panel="todo"
        tracking={undefined}
        daemon={{ state: "unavailable", detail: "no timonier program found" }}
      />,
    );
    expect(unavailable).toContain("no timonier program found");
  });

  it("speaks French when the IDE does", () => {
    expect(render("plan", SAMPLE_TRACKING, "fr")).toContain("2 sur 5 faites");
    expect(render("todo", SAMPLE_TRACKING, "fr")).toContain("par claude-code");
    expect(render("sessions", SAMPLE_TRACKING, "fr")).toContain("Composants touchés");
  });

  it("writes dates in the IDE's language", () => {
    expect(when("2026-10-20T13:05:00Z", "fr")).toMatch(/2026/);
    expect(when("not a date", "en")).toBe("not a date");
  });
});
