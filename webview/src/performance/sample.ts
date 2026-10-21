/**
 * A run as the daemon gives it, for the development page and the accessibility tests
 * (`demo.html?performance=1`): four tests of a parser, one failed, one not dated, with nested
 * spans and the resources sampled every 100 ms.
 */
import type { PerfRun, PerfRuns, RegressionReport, RunSummary, SpanView, TestView } from "../protocol";

const NOW = Date.UTC(2026, 9, 21, 14, 0, 0);

function summary(n: number, wallUs: number, cpuUs: number, failed: number, commit: string): RunSummary {
  return {
    run: `R-${n}`,
    startedAtMs: NOW - (13 - n) * 3_600_000,
    command: ["cargo", "nextest", "run", "--profile", "timonier"],
    finished: true,
    traced: true,
    snapshot: 40 + n,
    commit,
    branch: "main",
    exitCode: failed > 0 ? 100 : 0,
    wallUs,
    cpuUs,
    instructions: null,
    cpuSource: "cpu_time",
    memoryBytes: 182_000_000 + n * 1_500_000,
    memoryWholeTree: true,
    tests: 4,
    failed,
    spans: 11,
    batch: `B-${n}`,
    fingerprint: "a1b2c3d4e5f60718",
    worktree: null,
    smoke: false,
  };
}

export const SAMPLE_RUNS: PerfRuns = {
  runs: [
    summary(12, 2_840_000, 4_120_000, 1, "9f4c2ab1e07d"),
    summary(11, 2_310_000, 3_400_000, 0, "51d0e9c3a2bb"),
    summary(10, 2_290_000, 3_380_000, 0, "51d0e9c3a2bb"),
    summary(9, 2_420_000, 3_510_000, 0, "e1a77f0b9c41"),
    summary(8, 2_150_000, 3_190_000, 0, "0c3b8d2e5f17"),
    summary(7, 2_200_000, 3_250_000, 0, "77ab01c9de23"),
  ],
};

const test = (ord: number, name: string, durationUs: number, status: TestView["status"]): TestView => ({
  ord,
  name,
  classname: "timonier-core::parser",
  status,
  durationUs,
  file: "crates/timonier-core/src/parser.rs",
  line: 40 + ord * 12,
});

let next = 0;
function span(fields: Partial<SpanView> & { name: string; test: number }): SpanView {
  next++;
  return {
    trace: `t${fields.test}`,
    id: `s${next}`,
    parent: null,
    startNs: null,
    durationNs: 0,
    status: "unset",
    synthesized: false,
    node: null,
    function: null,
    file: null,
    line: null,
    ...fields,
  };
}

const ms = 1_000_000;
const roots = [
  span({ name: "parses_a_module", test: 0, synthesized: true, startNs: 120 * ms, durationNs: 640 * ms }),
  span({ name: "resolves_imports", test: 1, synthesized: true, startNs: 800 * ms, durationNs: 910 * ms }),
  span({
    name: "renders_the_view",
    test: 2,
    synthesized: true,
    startNs: 1_760 * ms,
    durationNs: 420 * ms,
    status: "error",
  }),
  span({ name: "keeps_positions", test: 3, synthesized: true, durationNs: 95 * ms }),
];
const parse = span({
  name: "parse",
  test: 0,
  startNs: 140 * ms,
  durationNs: 520 * ms,
  node: "rust:timonier_core::parser::parse",
});
const tokens = span({
  name: "tokens",
  test: 0,
  parent: parse.id,
  startNs: 150 * ms,
  durationNs: 180 * ms,
  node: "rust:timonier_core::lexer::tokens",
});
const tree = span({
  name: "tree",
  test: 0,
  parent: parse.id,
  startNs: 340 * ms,
  durationNs: 290 * ms,
  node: "rust:timonier_core::parser::tree",
});
const apply = span({
  name: "apply_all",
  test: 1,
  startNs: 830 * ms,
  durationNs: 610 * ms,
  node: "rust:timonier_store::Store::apply_all",
});
const resolve = span({
  name: "resolve",
  test: 1,
  startNs: 1_460 * ms,
  durationNs: 220 * ms,
  node: "rust:timonier_index::resolve::resolve",
});
const write = span({
  name: "write",
  test: 1,
  parent: apply.id,
  startNs: 860 * ms,
  durationNs: 480 * ms,
  node: "rust:timonier_store::write::apply",
});
const view = span({
  name: "load",
  test: 2,
  startNs: 1_790 * ms,
  durationNs: 300 * ms,
  node: "rust:timonier_daemon::view::load",
  status: "error",
});

export const SAMPLE_RUN: PerfRun = {
  summary: SAMPLE_RUNS.runs[0] ?? summary(12, 2_840_000, 4_120_000, 1, "9f4c2ab1e07d"),
  tests: [
    test(0, "parses_a_module", 640_000, "passed"),
    test(1, "resolves_imports", 910_000, "passed"),
    test(2, "renders_the_view", 420_000, "failed"),
    test(3, "keeps_positions", 95_000, "passed"),
  ],
  spans: [...roots, parse, tokens, tree, apply, resolve, write, view],
  spansLeftOut: 0,
  samples: Array.from({ length: 29 }, (_, i) => ({
    atMs: i * 100,
    memoryBytes: Math.round(60_000_000 + 120_000_000 * Math.sin((i / 28) * Math.PI) + (i % 3) * 4_000_000),
    cpuUs: i * 140_000 + (i > 8 ? 60_000 : 0),
  })),
  nodes: {
    "rust:timonier_core::parser::parse": "parse",
    "rust:timonier_core::lexer::tokens": "tokens",
    "rust:timonier_core::parser::tree": "tree",
    "rust:timonier_store::Store::apply_all": "apply_all",
    "rust:timonier_index::resolve::resolve": "resolve",
    "rust:timonier_store::write::apply": "apply",
    "rust:timonier_daemon::view::load": "load",
  },
};

/** When the sample runs happened, for « 3 h ago ». */
export const SAMPLE_NOW = NOW;

/** The regressions of the latest sample batch: one confirmed, one probable (ADR 0053). */
export const SAMPLE_REGRESSIONS: RegressionReport = {
  batch: "B-12",
  commit: "9f4c2ab1e07d",
  reference: ["51d0e9c3a2bb", "e1a77f0b9c41", "0c3b8d2e5f17"],
  source: "cpu_time",
  checked: 14,
  regressions: [
    {
      key: "node:rust:timonier_core::parser::parse",
      label: "parse",
      unit: "ns",
      status: "confirmed",
      verdict: { reference: 380e6, candidate: 520e6, change: 0.368, pValue: 0.0039, threshold: 19e6, regressed: true },
    },
    {
      key: "test:timonier-core::parser::renders_the_view",
      label: "test renders_the_view",
      unit: "us",
      status: "probable",
      verdict: { reference: 360e3, candidate: 420e3, change: 0.167, pValue: 0.0079, threshold: 18e3, regressed: true },
    },
  ],
  note: null,
  noteKind: null,
  main: "main",
};
