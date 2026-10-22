/**
 * What the Performances panel shows of a run (PLAN.md 8.7, ADR 0051): each test and the tree of
 * its spans, laid out on its own time; the call tree, its levels merged as the developer tools
 * of browsers merge them; the resources of the process over a stretch of time; the evolution of
 * the runs. Pure: the components draw what this computes.
 */
import type { Locale } from "../i18n";
import type { PerfRun, RunSummary, Sample, SpanView, TestView } from "../protocol";

/** A span and those it called, in the order they started. */
export interface SpanNode {
  span: SpanView;
  children: SpanNode[];
  /** 0 for a test's root. */
  depth: number;
}

/** A test, its root span and when it ran, from the run's start. */
export interface TestRow {
  test: TestView;
  root: SpanNode | null;
  /** `null` when neither the JUnit nor its spans date it. */
  startNs: number | null;
  durationNs: number;
}

const byStart = (a: SpanNode, b: SpanNode) => (a.span.startNs ?? 0) - (b.span.startNs ?? 0);

/**
 * The tests of a run with their trees. A span whose parent is not in the run, but which names
 * its test, goes under the test's root (ADR 0048); a root the JUnit does not date spans its
 * spans (ADR 0047).
 */
export function testRows(run: PerfRun): TestRow[] {
  const nodes = new Map<string, SpanNode>();
  for (const span of run.spans) nodes.set(span.id, { span, children: [], depth: 0 });
  const roots = new Map<number, SpanNode>();
  for (const node of nodes.values()) {
    if (node.span.synthesized && node.span.test !== null) roots.set(node.span.test, node);
  }
  for (const node of nodes.values()) {
    if (node.span.synthesized) continue;
    const parent = node.span.parent ? nodes.get(node.span.parent) : undefined;
    const holder = parent ?? (node.span.test !== null ? roots.get(node.span.test) : undefined);
    holder?.children.push(node);
  }
  const settle = (node: SpanNode, depth: number) => {
    node.depth = depth;
    node.children.sort(byStart);
    for (const child of node.children) settle(child, depth + 1);
  };
  return run.tests.map((test) => {
    const root = roots.get(test.ord) ?? null;
    if (root) settle(root, 0);
    let startNs = root?.span.startNs ?? null;
    let durationNs = root?.span.durationNs ?? test.durationUs * 1000;
    if (root && startNs === null && root.children.length > 0) {
      const starts = root.children.map((c) => c.span.startNs ?? 0);
      const ends = root.children.map((c) => (c.span.startNs ?? 0) + c.span.durationNs);
      startNs = Math.min(...starts);
      durationNs = Math.max(durationNs, Math.max(...ends) - startNs);
    }
    return { test, root, startNs, durationNs };
  });
}

/** A bar of the timeline: where on its row, in parts of the window. */
export interface Bar {
  node: SpanNode;
  /** 0 to 1. */
  left: number;
  width: number;
}

/**
 * A test's spans as bars, one row per depth, over the test's own time; a span the run does not
 * date sits at its parent's start.
 */
export function timeline(row: TestRow): Bar[][] {
  const rows: Bar[][] = [];
  if (!row.root) return rows;
  const start = row.startNs ?? row.root.span.startNs ?? 0;
  const length = Math.max(row.durationNs, 1);
  const walk = (node: SpanNode, parentStart: number) => {
    const at = node.span.startNs ?? parentStart;
    const left = Math.min(Math.max((at - start) / length, 0), 1);
    const width = Math.min(Math.max(node.span.durationNs / length, 0), 1 - left);
    (rows[node.depth] ??= []).push({ node, left, width });
    for (const child of node.children) walk(child, at);
  };
  walk(row.root, start);
  return rows;
}

/** A level of the call tree: the spans of one name and node under the same path, merged. */
export interface CallNode {
  /** Unique along the tree: the path of names. */
  key: string;
  name: string;
  node: string | null;
  calls: number;
  totalNs: number;
  /** Total less what it called. */
  selfNs: number;
  /** The stretches its spans ran, from the run's start, for the resources at its level. */
  windows: [number, number][];
  children: CallNode[];
}

/** The call tree of the tests, each test's root at the top, its levels merged by name and node. */
export function callTree(rows: TestRow[]): CallNode[] {
  const merge = (into: CallNode[], node: SpanNode, path: string, at: number | null) => {
    const name = node.span.name;
    const key = `${path}/${name}@${node.span.node ?? ""}`;
    let level = into.find((c) => c.key === key);
    if (!level) {
      level = { key, name, node: node.span.node, calls: 0, totalNs: 0, selfNs: 0, windows: [], children: [] };
      into.push(level);
    }
    const start = node.span.startNs ?? at;
    level.calls += 1;
    level.totalNs += node.span.durationNs;
    level.selfNs += Math.max(node.span.durationNs - node.children.reduce((sum, c) => sum + c.span.durationNs, 0), 0);
    if (start !== null) level.windows.push([start, start + node.span.durationNs]);
    for (const child of node.children) merge(level.children, child, key, start);
  };
  const top: CallNode[] = [];
  for (const row of rows) if (row.root) merge(top, row.root, "", row.startNs);
  const order = (levels: CallNode[]) => {
    levels.sort((a, b) => b.totalNs - a.totalNs || a.name.localeCompare(b.name));
    for (const level of levels) order(level.children);
  };
  order(top);
  return top;
}

/** The resources of the process while some stretches ran: its peak memory, the CPU it used. */
export interface Resources {
  memoryBytes: number;
  cpuUs: number;
}

/**
 * The process's resources over `windows` (nanoseconds from the run's start), from the samples
 * of the run (every 100 ms; ADR 0045): `null` without samples, or when none falls in them. A
 * stretch shorter than the sampling takes the sample around it.
 */
export function resourcesDuring(samples: Sample[], windows: [number, number][]): Resources | null {
  if (samples.length === 0 || windows.length === 0) return null;
  let memoryBytes = 0;
  let cpuUs = 0;
  let found = false;
  for (const [from, to] of windows) {
    const fromMs = from / 1e6;
    const toMs = to / 1e6;
    let inside = samples.filter((s) => s.atMs >= fromMs && s.atMs <= toMs);
    if (inside.length === 0) {
      const after = samples.find((s) => s.atMs >= toMs) ?? samples[samples.length - 1];
      inside = after ? [after] : [];
    }
    if (inside.length === 0) continue;
    found = true;
    memoryBytes = Math.max(memoryBytes, ...inside.map((s) => s.memoryBytes));
    const before = [...samples].reverse().find((s) => s.atMs < fromMs);
    const last = inside[inside.length - 1];
    if (last) cpuUs += Math.max(last.cpuUs - (before?.cpuUs ?? inside[0]?.cpuUs ?? 0), 0);
  }
  return found ? { memoryBytes, cpuUs } : null;
}

/** A point of the evolution: a commit, or a modified tree over it. */
export interface Point {
  /** The newest run of the point. */
  run: string;
  /** The commit's short name, or the run's when there is none; « * » for a modified tree. */
  label: string;
  wallS: number;
  cpuS: number;
  failed: number;
  runs: number;
}

function median(values: number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 1 ? (sorted[middle] ?? 0) : ((sorted[middle - 1] ?? 0) + (sorted[middle] ?? 0)) / 2;
}

/**
 * The evolution (spec « Views »): one point per commit, the median of its finished runs, from the
 * oldest. Only the runs comparable with `of` count: the same command, machine and traces; a
 * modified tree is a point of its own (Q77).
 */
export function evolution(runs: RunSummary[], of: RunSummary | null = null): Point[] {
  const finished = runs.filter((r) => r.finished && r.wallUs !== null);
  const like = of ?? finished[0];
  if (!like) return [];
  const command = JSON.stringify(like.command);
  const groups = new Map<string, RunSummary[]>();
  // From the oldest: a point sits where its first run came.
  for (const r of [...finished].reverse()) {
    if (JSON.stringify(r.command) !== command || r.fingerprint !== like.fingerprint || r.traced !== like.traced) {
      continue;
    }
    const key = `${r.commit ?? r.run}\u0000${r.worktree ?? ""}`;
    groups.set(key, [...(groups.get(key) ?? []), r]);
  }
  return [...groups.values()].map((group) => {
    const newest = group[group.length - 1] ?? group[0];
    const label = newest?.commit ? newest.commit.slice(0, 7) : (newest?.run ?? "");
    return {
      run: newest?.run ?? "",
      label: newest?.worktree ? `${label}*` : label,
      wallS: median(group.map((r) => (r.wallUs ?? 0) / 1e6)),
      cpuS: median(group.map((r) => (r.cpuUs ?? 0) / 1e6)),
      failed: median(group.map((r) => r.failed)),
      runs: group.length,
    };
  });
}

/** A duration for people: « 0,4 µs », « 12 µs », « 4,2 ms », « 1,25 s ». */
export function duration(ns: number, locale: Locale): string {
  const number = (value: number, digits: number) =>
    new Intl.NumberFormat(locale, { maximumFractionDigits: digits }).format(value);
  if (ns < 1_000) return `${number(ns / 1_000, 1)} µs`;
  if (ns < 1_000_000) return `${number(ns / 1_000, ns < 10_000 ? 1 : 0)} µs`;
  if (ns < 1_000_000_000) return `${number(ns / 1_000_000, ns < 10_000_000 ? 1 : 0)} ms`;
  return `${number(ns / 1_000_000_000, 2)} s`;
}

/** A size for people, in powers of 1000 as the system monitors show them: « 31 MB ». */
export function bytes(value: number, locale: Locale): string {
  const number = (v: number) => new Intl.NumberFormat(locale, { maximumFractionDigits: v < 10 ? 1 : 0 }).format(v);
  const unit = locale === "fr" ? ["o", "ko", "Mo", "Go"] : ["B", "kB", "MB", "GB"];
  let size = value;
  let at = 0;
  while (size >= 1000 && at < unit.length - 1) {
    size /= 1000;
    at++;
  }
  return `${number(size)} ${unit[at]}`;
}

/** A share of a whole, for the bars of the call tree: 0 to 1. */
export function share(part: number, whole: number): number {
  return whole > 0 ? Math.min(Math.max(part / whole, 0), 1) : 0;
}

/** A share for people: « 34 % », « < 1 % » for a part too small to round to one. */
export function percent(fraction: number, locale: Locale): string {
  const format = (value: number) =>
    new Intl.NumberFormat(locale, { style: "percent", maximumFractionDigits: 0 }).format(value);
  return fraction > 0 && fraction < 0.005 ? `< ${format(0.01)}` : format(fraction);
}
