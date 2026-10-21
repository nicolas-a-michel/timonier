/**
 * A tracking as an agent leaves it halfway through a session: for the tests and the demo page
 * (`demo.html?tracking=plan`).
 */
import { NO_REVIEW, type ReviewBox, type SessionView, type TaskView, type TrackingView } from "../protocol";

/** Nothing tracked yet, no review. */
export const EMPTY_TRACKING: TrackingView = {
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

function task(id: string, title: string, status: TaskView["status"], extra: Partial<TaskView> = {}): TaskView {
  return { id, title, criterion: null, status, sessions: [], notes: [], ...extra };
}

function reviewed(id: string, name: string, within: string | null = null): ReviewBox {
  return { id, kind: id.startsWith("pkg:") ? "package" : "component", name, role: null, within };
}

const box = (id: string, name: string, within: string | null = null) => ({ id, name, within });

const open: SessionView = {
  id: "S-2026-10-20-3",
  agent: "claude-code",
  objective: "Cache the extraction of unchanged files",
  tasks: ["T-6"],
  open: true,
  startedAt: "2026-10-20T13:05:00Z",
  endedAt: null,
  summary: null,
  todo: [
    { id: "D-1", text: "Key the cache by content hash", task: "T-6", done: true },
    { id: "D-2", text: "Store the extractions in SQLite", task: "T-6", done: true },
    { id: "D-3", text: "Drop entries of a changed pack version", task: "T-6", done: false },
    { id: "D-4", text: "Measure the reindexing time", task: null, done: false },
  ],
  changes: {
    files: 7,
    added: [],
    modified: [box("pkg:cargo/timonier-daemon", "timonier-daemon"), box("pkg:cargo/timonier-core", "timonier-core")],
    removed: [],
    linked: [],
    unlinked: [],
  },
  budgets: [],
};

const ended: SessionView = {
  id: "S-2026-10-20-2",
  agent: "claude-code",
  objective: "Count the review debt of each box",
  tasks: ["T-5"],
  open: false,
  startedAt: "2026-10-20T09:40:00Z",
  endedAt: "2026-10-20T12:10:00Z",
  summary: "The map shows the files to read again, in three levels, untracked ones hatched.",
  todo: [
    { id: "D-1", text: "Compute the debt windows", task: "T-5", done: true },
    { id: "D-2", text: "Draw the layer", task: "T-5", done: true },
  ],
  changes: {
    files: 12,
    added: [box("component:dir/crates/timonier-daemon/src/debt", "debt", "timonier-daemon")],
    modified: [box("pkg:cargo/timonier-daemon", "timonier-daemon"), box("pkg:npm/@timonier/webview", "webview")],
    removed: [],
    linked: [
      {
        from: box("pkg:cargo/timonier-daemon", "timonier-daemon"),
        to: box("pkg:cargo/timonier-store", "timonier-store"),
      },
    ],
    unlinked: [],
  },
  budgets: ["N-2 on Store: p95 7.40 ms > 5 ms"],
};

const older: SessionView = {
  ...ended,
  id: "S-2026-10-19-4",
  objective: "Record what changes with no session open",
  tasks: ["T-4"],
  startedAt: "2026-10-19T15:00:00Z",
  endedAt: "2026-10-19T16:20:00Z",
  summary: "Untracked gaps are computed at each session start.",
  changes: null,
  budgets: [],
};

export const SAMPLE_TRACKING: TrackingView = {
  milestones: [
    {
      id: "M-1",
      title: "Phase 5 — Review diff",
      done: 3,
      of: 3,
      tasks: [
        task("T-1", "Snapshot diff by component", "done"),
        task("T-2", "Mark as reviewed", "done"),
        task("T-3", "Five-second test", "done"),
      ],
    },
    {
      id: "M-2",
      title: "Phase 6 — MCP, init and tracking",
      done: 2,
      of: 5,
      tasks: [
        task("T-4", "Untracked changes", "done"),
        task("T-5", "Review debt layer", "done", {
          criterion: "Heat by component on the map, as a layer",
          notes: [{ at: "2026-10-20T12:00:00Z", session: "S-2026-10-20-2", text: "Levels agreed in Q44." }],
        }),
        task("T-6", "Extraction cache", "inProgress", { sessions: ["S-2026-10-20-3"] }),
        task("T-7", "timonier init", "blocked", {
          notes: [{ at: "2026-10-20T13:00:00Z", session: "S-2026-10-20-3", text: "Waits for the bridge." }],
        }),
        task("T-8", "The bridge", "todo"),
        task("T-9", "Zed panel", "dropped"),
      ],
    },
  ],
  open: open.id,
  todo: open.todo,
  sessions: [open, ended, older],
  sessionsTotal: 3,
  problems: [],
  review: {
    since: { seq: 40, timestampMs: Date.UTC(2026, 9, 20, 8, 30) },
    added: [reviewed("component:dir/crates/timonier-daemon/src/debt", "debt", "timonier-daemon")],
    modified: [
      reviewed("pkg:cargo/timonier-core", "timonier-core"),
      reviewed("pkg:cargo/timonier-daemon", "timonier-daemon"),
      reviewed("pkg:npm/@timonier/webview", "webview"),
    ],
    removed: [],
    dependenciesAdded: [
      {
        src: reviewed("pkg:cargo/timonier-daemon", "timonier-daemon"),
        dst: reviewed("pkg:cargo/timonier-store", "timonier-store"),
      },
    ],
    dependenciesRemoved: [],
    cycles: [],
    fans: [],
  },
  configured: true,
  debt: { files: 14, of: 220, untracked: 2 },
  sessionsSinceReview: 2,
  look: [
    { kind: "blocked", task: "T-7", title: "timonier init" },
    {
      kind: "debt",
      item: reviewed("pkg:cargo/timonier-daemon", "timonier-daemon"),
      debt: { files: 9, of: 41, untracked: 2 },
      path: [],
    },
    {
      kind: "debt",
      item: reviewed("component:dir/webview/src/map", "map", "webview"),
      debt: { files: 4, of: 22, untracked: 0 },
      path: ["pkg:npm/@timonier/webview"],
    },
    {
      kind: "debt",
      item: reviewed("pkg:cargo/timonier-core", "timonier-core"),
      debt: { files: 1, of: 30, untracked: 0 },
      path: [],
    },
  ],
};
