import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { PerformanceFeed } from "../../performanceFeed";
import { BudgetFeed, PerfMapFeed } from "../../perfMapFeed";
import type {
  BudgetsView,
  HostMessage,
  PerfMap,
  PerfRun,
  PerfRuns,
  RegressionReport,
  RunSummary,
} from "../../protocol/generated";

const summary = (run: string, finished = true, traced = true): RunSummary => ({
  run,
  startedAtMs: 0,
  command: ["true"],
  finished,
  traced,
  snapshot: 1,
  commit: null,
  branch: null,
  exitCode: 0,
  wallUs: 1,
  cpuUs: 1,
  instructions: null,
  cpuSource: "cpu_time",
  memoryBytes: 1,
  memoryWholeTree: false,
  tests: 0,
  failed: 0,
  spans: 0,
  batch: null,
  fingerprint: null,
  worktree: null,
  smoke: false,
});

const runOf = (run: string): PerfRun => ({
  summary: summary(run),
  tests: [],
  spans: [],
  spansLeftOut: 0,
  samples: [],
  nodes: {},
});

const settle = () => new Promise((resolve) => setImmediate(resolve));

const report: RegressionReport = {
  batch: "B-1",
  commit: null,
  reference: [],
  source: "cpu_time",
  checked: 0,
  regressions: [],
  note: "no reference yet",
  noteKind: "no_reference",
  main: "main",
};

describe("the Performances panel's feed (ADR 0051)", () => {
  it("gives the runs once, again when they change, and the run asked for", async () => {
    const posted: HostMessage[] = [];
    let runs: PerfRuns = { runs: [summary("R-1", false)] };
    const feed = new PerformanceFeed({
      runs: () => Promise.resolve(runs),
      run: (run) => Promise.resolve(runOf(run)),
      regressions: () => Promise.resolve(report),
      post: (m) => posted.push(m),
    });
    await feed.opened();
    await feed.poll();
    assert.deepEqual(
      posted.map((m) => m.type),
      ["perfRuns", "perfRegressions"],
      "the same runs: not posted twice",
    );
    await feed.choose("R-1");
    assert.deepEqual(posted.at(-1), { type: "perfRun", run: runOf("R-1") });
    // The run shown ends: the runs again, and the run.
    runs = { runs: [summary("R-1")] };
    await feed.poll();
    assert.deepEqual(
      posted.slice(-3).map((m) => m.type),
      ["perfRuns", "perfRegressions", "perfRun"],
    );
  });

  it("says when the daemon is gone, and when it is back", async () => {
    const posted: HostMessage[] = [];
    let up = false;
    const feed = new PerformanceFeed({
      runs: () => (up ? Promise.resolve({ runs: [] }) : Promise.reject(new Error("down"))),
      run: () => Promise.resolve(null),
      regressions: () => Promise.resolve(report),
      post: (m) => posted.push(m),
    });
    await feed.opened();
    await feed.poll();
    assert.deepEqual(posted, [{ type: "daemon", state: "unavailable", detail: null }]);
    up = true;
    await feed.poll();
    assert.deepEqual(posted.slice(1), [
      { type: "daemon", state: "connected", detail: null },
      { type: "perfRuns", runs: { runs: [] } },
      { type: "perfRegressions", report },
    ]);
  });
});

describe("the map's Performances layer (ADR 0051)", () => {
  const layer = (run: string | null): PerfMap => ({ run, nodes: [], links: [], ancestors: {} });
  const timers = {
    setInterval: (() => 1) as unknown as typeof globalThis.setInterval,
    clearInterval: (() => undefined) as unknown as typeof globalThis.clearInterval,
  };

  it("is built when the layer comes on, and again when a run with traces ends", async () => {
    const posted: HostMessage[] = [];
    let built = "R-1";
    let runs: PerfRuns = { runs: [summary("R-1")] };
    const feed = new PerfMapFeed(
      {
        fetch: () => Promise.resolve(layer(built)),
        runs: () => Promise.resolve(runs),
        post: (m) => posted.push(m),
      },
      timers,
    );
    await feed.poll();
    assert.equal(posted.length, 0, "off: nothing asked");
    await feed.show(true);
    assert.deepEqual(posted, [{ type: "perfMap", perf: layer("R-1") }]);
    // A run still going, or one without traces, changes nothing.
    runs = { runs: [summary("R-3", false), summary("R-2", true, false), summary("R-1")] };
    await feed.poll();
    assert.equal(posted.length, 1);
    built = "R-4";
    runs = { runs: [summary("R-4"), summary("R-1")] };
    await feed.poll();
    await settle();
    assert.deepEqual(posted.at(-1), { type: "perfMap", perf: layer("R-4") });
    // A smoke test of `doctor` is not what the map shows: asked for again, never (relecture
    // n° 3, I-1).
    const shown = posted.length;
    runs = { runs: [{ ...summary("R-5"), smoke: true }, summary("R-4"), summary("R-1")] };
    for (let i = 0; i < 5; i++) await feed.poll();
    await settle();
    assert.equal(posted.length, shown);
  });

  it("is asked for once when no listed run has traces, not at every tick (relecture n° 4, m-5)", async () => {
    const posted: HostMessage[] = [];
    // Fifty runs without traces hide the older one the layer is of.
    const runs: PerfRuns = { runs: [summary("R-60", true, false), summary("R-59", true, false)] };
    const feed = new PerfMapFeed(
      {
        fetch: () => Promise.resolve(layer("R-9")),
        runs: () => Promise.resolve(runs),
        post: (m) => posted.push(m),
      },
      timers,
    );
    await feed.show(true);
    for (let i = 0; i < 5; i++) await feed.poll();
    await settle();
    assert.ok(posted.length <= 2, `asked ${posted.length} times`);
  });

  it("comes back after the daemon failed once (relecture n° 5, m-1)", async () => {
    const posted: HostMessage[] = [];
    let fails = false;
    let runs: PerfRuns = { runs: [summary("R-1")] };
    const feed = new PerfMapFeed(
      {
        fetch: () => (fails ? Promise.reject(new Error("busy")) : Promise.resolve(layer(runs.runs[0]?.run ?? null))),
        runs: () => Promise.resolve(runs),
        post: (m) => posted.push(m),
      },
      timers,
    );
    await feed.show(true);
    runs = { runs: [summary("R-2"), summary("R-1")] };
    fails = true;
    await feed.poll();
    assert.deepEqual(posted.at(-1), { type: "perfMap", perf: null });
    fails = false;
    await feed.poll();
    await settle();
    assert.deepEqual(posted.at(-1), { type: "perfMap", perf: layer("R-2") });
  });

  it("gives null when the daemon cannot answer, and nothing once off", async () => {
    const posted: HostMessage[] = [];
    const feed = new PerfMapFeed(
      {
        fetch: () => Promise.reject(new Error("down")),
        runs: () => Promise.reject(new Error("down")),
        post: (m) => posted.push(m),
      },
      timers,
    );
    await feed.show(true);
    assert.deepEqual(posted, [{ type: "perfMap", perf: null }]);
    await feed.show(false);
    await feed.poll();
    assert.equal(posted.length, 1);
  });
});

describe("the budgets for the map (Q74, ADR 0052)", () => {
  const view = (state: "ok" | "exceeded"): BudgetsView => ({
    checks: [
      {
        id: "N-1",
        attachedTo: "C-1",
        on: "Parser",
        state,
        boxes: ["pkg:cargo/parser"],
        limits: [{ kind: "p95_ms", limit: 5, measured: state === "ok" ? 2 : 7, exceeded: state === "exceeded" }],
        runs: ["R-1"],
      },
    ],
    ancestors: { "pkg:cargo/parser": [] },
  });
  const timers = {
    setInterval: (() => 1) as unknown as typeof globalThis.setInterval,
    clearInterval: (() => undefined) as unknown as typeof globalThis.clearInterval,
  };

  it("are given when the map opens, and again when a run ends or the whiteboard changes", async () => {
    const posted: HostMessage[] = [];
    let state: "ok" | "exceeded" = "ok";
    let runs: PerfRuns = { runs: [summary("R-1")] };
    const feed = new BudgetFeed(
      { fetch: () => Promise.resolve(view(state)), runs: () => Promise.resolve(runs), post: (m) => posted.push(m) },
      timers,
    );
    await feed.changed();
    assert.equal(posted.length, 0, "no map: nothing asked");
    await feed.start();
    assert.deepEqual(posted, [{ type: "budgets", budgets: view("ok") }]);
    await feed.poll();
    const after = posted.length;
    await feed.poll();
    assert.equal(posted.length, after, "the same newest run: not asked again");
    state = "exceeded";
    runs = { runs: [summary("R-2"), summary("R-1")] };
    await feed.poll();
    assert.deepEqual(posted.at(-1), { type: "budgets", budgets: view("exceeded") });
    await feed.changed();
    assert.equal(posted.length, after + 2, "the whiteboard changed: asked again");
    // The daemon fails once: asked again at the next tick (relecture n° 5, m-1).
    let failing = true;
    const flaky = new BudgetFeed(
      {
        fetch: () => (failing ? Promise.reject(new Error("busy")) : Promise.resolve(view("ok"))),
        runs: () => Promise.resolve(runs),
        post: (m) => posted.push(m),
      },
      timers,
    );
    await flaky.start();
    await flaky.poll();
    failing = false;
    await flaky.poll();
    assert.deepEqual(posted.at(-1), { type: "budgets", budgets: view("ok") });
    flaky.stop();
    const before = posted.length;
    // A smoke test of `doctor` judges no budget: nothing asked (relecture n° 4, m-2).
    runs = { runs: [{ ...summary("R-3"), smoke: true }, summary("R-2"), summary("R-1")] };
    await feed.poll();
    assert.equal(posted.length, before);
    feed.stop();
    await feed.changed();
    assert.equal(posted.length, before);
  });
});
