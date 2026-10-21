/** The Performances panel's model (PLAN.md 8.7, ADR 0051). */
import { describe, expect, it } from "vitest";

import type { PerfRun, RunSummary, SpanView, TestView } from "../protocol";
import { bytes, callTree, duration, evolution, resourcesDuring, share, testRows, timeline } from "./model";

const summary: RunSummary = {
  run: "R-3",
  startedAtMs: 0,
  command: ["sh", "run-tests.sh"],
  finished: true,
  traced: true,
  snapshot: 4,
  commit: "0123456789abcdef",
  branch: "main",
  exitCode: 0,
  wallUs: 2_000_000,
  cpuUs: 1_500_000,
  instructions: null,
  cpuSource: "cpu_time",
  memoryBytes: 31_000_000,
  memoryWholeTree: true,
  tests: 2,
  failed: 1,
  spans: 4,
  batch: null,
  fingerprint: null,
  worktree: null,
  smoke: false,
};

const test = (ord: number, name: string, durationUs: number): TestView => ({
  ord,
  name,
  classname: "demo",
  status: ord === 1 ? "failed" : "passed",
  durationUs,
  file: null,
  line: null,
});

const span = (id: string, fields: Partial<SpanView>): SpanView => ({
  trace: "t",
  id,
  parent: null,
  test: 0,
  name: id,
  startNs: null,
  durationNs: 0,
  status: "unset",
  synthesized: false,
  node: null,
  function: null,
  file: null,
  line: null,
  ...fields,
});

/** Test 0 dated by its JUnit; test 1 not, its span dates it. */
const run: PerfRun = {
  summary,
  tests: [test(0, "parses", 100), test(1, "renders", 50)],
  spans: [
    span("root0", { synthesized: true, startNs: 1_000, durationNs: 100_000, name: "parses" }),
    span("root1", { synthesized: true, test: 1, durationNs: 50_000, name: "renders" }),
    span("load", { startNs: 11_000, durationNs: 60_000, node: "rust:demo::f" }),
    span("draw", { parent: "load", startNs: 21_000, durationNs: 20_000, node: "rust:demo::g" }),
    // Its parent was in another process: under the test's root.
    span("again", { parent: "gone", startNs: 81_000, durationNs: 10_000, node: "rust:demo::f", name: "load" }),
    span("paint", { test: 1, startNs: 200_000, durationNs: 80_000 }),
  ],
  spansLeftOut: 0,
  samples: [
    { atMs: 0, memoryBytes: 10, cpuUs: 0 },
    { atMs: 100, memoryBytes: 40, cpuUs: 500 },
    { atMs: 200, memoryBytes: 20, cpuUs: 900 },
  ],
  nodes: { "rust:demo::f": "f", "rust:demo::g": "g" },
};

describe("the Performances panel's model", () => {
  it("puts each span under its test, its orphans under the root", () => {
    const [parses, renders] = testRows(run);
    expect(parses?.root?.children.map((c) => c.span.id)).toEqual(["load", "again"]);
    expect(parses?.root?.children[0]?.children.map((c) => c.span.id)).toEqual(["draw"]);
    expect(parses?.root?.children[0]?.children[0]?.depth).toBe(2);
    expect(parses?.startNs).toBe(1_000);
    // Undated by the JUnit: its spans date it, and widen it to cover them.
    expect(renders?.startNs).toBe(200_000);
    expect(renders?.durationNs).toBe(80_000);
  });

  it("lays a test's spans out as rows of bars on its own time", () => {
    const [parses] = testRows(run);
    if (!parses) throw new Error("no test");
    const rows = timeline(parses);
    expect(rows.map((r) => r.map((b) => b.node.span.id))).toEqual([["root0"], ["load", "again"], ["draw"]]);
    expect(rows[0]?.[0]).toMatchObject({ left: 0, width: 1 });
    expect(rows[1]?.[0]?.left).toBeCloseTo(0.1);
    expect(rows[1]?.[0]?.width).toBeCloseTo(0.6);
    expect(rows[2]?.[0]?.left).toBeCloseTo(0.2);
  });

  it("merges the call tree's levels by name and node, with their own time", () => {
    const tree = callTree(testRows(run));
    expect(tree.map((t) => t.name)).toEqual(["parses", "renders"]);
    const load = tree[0]?.children[0];
    expect(load).toMatchObject({ name: "load", node: "rust:demo::f", calls: 2, totalNs: 70_000 });
    // 60 µs less the 20 µs of draw, and the 10 µs of the second call.
    expect(load?.selfNs).toBe(50_000);
    expect(load?.windows).toEqual([
      [11_000, 71_000],
      [81_000, 91_000],
    ]);
    expect(tree[0]?.selfNs).toBe(30_000);
  });

  it("says the process's resources over a stretch, the sample around a short one", () => {
    expect(resourcesDuring(run.samples, [[50_000_000, 150_000_000]])).toEqual({ memoryBytes: 40, cpuUs: 500 });
    expect(resourcesDuring(run.samples, [[110_000_000, 120_000_000]])).toEqual({ memoryBytes: 20, cpuUs: 400 });
    expect(resourcesDuring([], [[0, 1]])).toBeNull();
  });

  it("draws the evolution as the median of each commit's runs, from the oldest", () => {
    const older = { ...summary, run: "R-2", commit: null, wallUs: 1_000_000 };
    const running = { ...summary, run: "R-4", finished: false, wallUs: null };
    const again = { ...summary, run: "R-5", wallUs: 4_000_000, cpuUs: 2_500_000 };
    const twice = { ...summary, run: "R-6", wallUs: 3_000_000, cpuUs: 2_000_000 };
    // Not comparable: another command, a modified tree of its own.
    const smoke = { ...summary, run: "R-7", command: ["sh", "smoke.sh"] };
    const modified = { ...summary, run: "R-8", worktree: "f00d" };
    expect(evolution([modified, smoke, twice, again, running, summary, older], twice)).toEqual([
      { run: "R-2", label: "R-2", wallS: 1, cpuS: 1.5, failed: 1, runs: 1 },
      { run: "R-6", label: "0123456", wallS: 3, cpuS: 2, failed: 1, runs: 3 },
      { run: "R-8", label: "0123456*", wallS: 2, cpuS: 1.5, failed: 1, runs: 1 },
    ]);
  });

  it("writes durations and sizes as people read them", () => {
    expect(duration(400, "en")).toBe("0.4 µs");
    expect(duration(12_000, "en")).toBe("12 µs");
    expect(duration(4_200_000, "fr")).toBe("4,2 ms");
    expect(duration(1_250_000_000, "en")).toBe("1.25 s");
    expect(bytes(31_000_000, "en")).toBe("31 MB");
    expect(bytes(1_500, "fr")).toBe("1,5 ko");
    expect(share(5, 0)).toBe(0);
    expect(share(15, 10)).toBe(1);
  });
});
