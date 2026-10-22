/**
 * The Performances panel (PLAN.md 8.7, ADR 0051): a run, its figures, and three tabs: the
 * timeline of each test (its spans by depth, the resources of the process below), the call
 * tree, as the developer tools of browsers show it, and the evolution of the runs.
 */
import { useMemo, useState, type KeyboardEvent } from "react";

import { Badge, EmptyState } from "../design/components";
import { ToneIcon } from "../design/icons";
import type { Locale } from "../i18n";
import type { PerfRun, PerfRuns, Regression, RegressionReport, RunSummary } from "../protocol";
import { Curves, type Curve } from "./Curves";
import { performanceMessages, type PerformanceMessages } from "./messages";
import {
  bytes,
  callTree,
  duration,
  evolution,
  resourcesDuring,
  share,
  testRows,
  timeline,
  type CallNode,
  type TestRow,
} from "./model";

export interface PerformanceProps {
  locale: Locale;
  /** `null`: no daemon. */
  runs: PerfRuns | null;
  /** The run shown; `undefined` while it comes, `null` when it is gone. */
  run: PerfRun | null | undefined;
  chosen: string | null;
  onChoose: (run: string) => void;
  /** « Run the tests », when no run is there yet: the host shows the commands first. */
  onRunTests?: () => void;
  /** Milliseconds since the epoch, for « 3 min ago ». */
  now: number;
  /** The tab shown first: the timeline unless said. */
  initialTab?: Tab;
  /** The latest batch against the main branch (ADR 0053). */
  regressions?: RegressionReport | null;
}

export type Tab = "timeline" | "tree" | "evolution";
const TABS: Tab[] = ["timeline", "tree", "evolution"];

export function Performance({
  locale,
  runs,
  run,
  chosen,
  onChoose,
  onRunTests,
  now,
  initialTab,
  regressions,
}: PerformanceProps) {
  const t = performanceMessages[locale];
  const [tab, setTab] = useState<Tab>(initialTab ?? "timeline");
  if (runs === null) {
    return <EmptyState title={t.noDaemonTitle} text={t.noDaemonText} />;
  }
  if (runs.runs.length === 0) {
    return (
      <EmptyState
        title={t.emptyTitle}
        text={t.emptyText}
        action={onRunTests ? { label: t.runTests, onSelect: onRunTests } : undefined}
      />
    );
  }
  const summary =
    run?.summary ?? runs.runs.find((r) => r.run === chosen) ?? runs.runs.find((r) => !r.smoke) ?? runs.runs[0];
  const onTabKey = (event: KeyboardEvent<HTMLButtonElement>) => {
    const at = TABS.indexOf(tab);
    const next =
      event.key === "ArrowRight"
        ? (at + 1) % TABS.length
        : event.key === "ArrowLeft"
          ? (at + TABS.length - 1) % TABS.length
          : -1;
    if (next < 0) return;
    event.preventDefault();
    const chosenTab = TABS[next] ?? "timeline";
    setTab(chosenTab);
    document.getElementById(`tim-perf-tab-${chosenTab}`)?.focus();
  };
  return (
    <main className="tim-perf">
      <header className="tim-perf__header">
        <h1 className="tim-perf__title">{t.title}</h1>
        <label className="tim-perf__choice">
          <span>{t.run}</span>
          <select value={summary?.run ?? ""} onChange={(event) => onChoose(event.target.value)}>
            {runs.runs.map((r) => (
              <option key={r.run} value={r.run}>
                {runLabel(r, locale, now)}
              </option>
            ))}
          </select>
        </label>
        {summary && <Figures summary={summary} locale={locale} />}
      </header>
      <div className="tim-perf__tabs" role="tablist" aria-label={t.title}>
        {TABS.map((name) => (
          <button
            key={name}
            id={`tim-perf-tab-${name}`}
            type="button"
            role="tab"
            className="tim-perf__tab"
            aria-selected={tab === name}
            aria-controls={`tim-perf-panel-${name}`}
            tabIndex={tab === name ? 0 : -1}
            onClick={() => setTab(name)}
            onKeyDown={onTabKey}
          >
            {t.tabs[name]}
          </button>
        ))}
      </div>
      <section
        id={`tim-perf-panel-${tab}`}
        role="tabpanel"
        aria-labelledby={`tim-perf-tab-${tab}`}
        className="tim-perf__panel"
      >
        {tab === "evolution" ? (
          <>
            <Regressions report={regressions} locale={locale} />
            <Evolution runs={runs.runs} of={run?.summary ?? null} locale={locale} />
          </>
        ) : run === null ? (
          <p className="tim-perf__note">{t.gone}</p>
        ) : run === undefined ? null : tab === "timeline" ? (
          <Timeline run={run} locale={locale} />
        ) : (
          <Tree run={run} locale={locale} />
        )}
        {run && run.spansLeftOut > 0 && tab !== "evolution" && (
          <p className="tim-perf__note">{t.leftOut(run.spansLeftOut)}</p>
        )}
      </section>
    </main>
  );
}

function runLabel(run: RunSummary, locale: Locale, now: number): string {
  const t = performanceMessages[locale];
  const where = [run.branch, run.commit?.slice(0, 7)].filter(Boolean).join(" ") || null;
  const state = !run.finished ? t.running : !run.traced ? t.withoutTraces : run.smoke ? t.smoke : null;
  return [t.runOption(run.run, where, t.ago(now - run.startedAtMs)), state].filter(Boolean).join(" · ");
}

function Figures({ summary, locale }: { summary: RunSummary; locale: Locale }) {
  const t = performanceMessages[locale];
  const number = new Intl.NumberFormat(locale, { maximumFractionDigits: 2 });
  const figures: [string, string][] = [];
  if (summary.wallUs !== null) figures.push([t.wall, duration(summary.wallUs * 1000, locale)]);
  if (summary.cpuUs !== null) {
    const cpu = duration(summary.cpuUs * 1000, locale);
    figures.push([
      t.cpu(summary.cpuSource),
      summary.instructions !== null ? `${t.instructions(number.format(summary.instructions / 1e9))} · ${cpu}` : cpu,
    ]);
  }
  if (summary.memoryBytes !== null)
    figures.push([t.memory(summary.memoryWholeTree), bytes(summary.memoryBytes, locale)]);
  figures.push([t.tabs.timeline, `${t.tests(summary.tests, summary.failed)} · ${t.spans(summary.spans)}`]);
  return (
    <dl className="tim-perf__figures">
      {figures.map(([name, value]) => (
        <div
          key={name}
          className="tim-perf__figure"
          data-tone={name === t.tabs.timeline && summary.failed > 0 ? "violation" : undefined}
        >
          <dt>{name === t.tabs.timeline ? t.testList : name}</dt>
          <dd>{value}</dd>
        </div>
      ))}
    </dl>
  );
}

function Timeline({ run, locale }: { run: PerfRun; locale: Locale }) {
  const t = performanceMessages[locale];
  const rows = useMemo(() => testRows(run), [run]);
  const [picked, setPicked] = useState(0);
  const row = rows[picked] ?? rows[0];
  const longest = Math.max(1, ...rows.map((r) => r.durationNs));
  return (
    <div className="tim-perf__timeline">
      <ul className="tim-perf__tests" aria-label={t.testList}>
        {rows.map((r, i) => {
          const time = duration(r.durationNs, locale);
          return (
            <li key={r.test.ord}>
              <button
                type="button"
                className="tim-perf__test"
                aria-current={i === picked}
                data-status={r.test.status}
                onClick={() => setPicked(i)}
              >
                <span className="tim-perf__test-name">
                  {r.test.name}
                  {r.test.status === "failed" || r.test.status === "error" ? (
                    // In shape and word, not in colour alone (relecture n° 1, I2).
                    <>
                      {" "}
                      <Badge tone="violation" label={t.status[r.test.status]} />
                    </>
                  ) : (
                    <span className="tim-visually-hidden">, {t.status[r.test.status]},</span>
                  )}
                </span>
                <span className="tim-perf__test-time">
                  {time}
                  {r.startNs === null && <span className="tim-perf__undated"> · {t.undated}</span>}
                </span>
                <span className="tim-perf__share" aria-hidden="true">
                  <span style={{ width: `${share(r.durationNs, longest) * 100}%` }} />
                </span>
              </button>
            </li>
          );
        })}
      </ul>
      {row && <TestSpans row={row} run={run} locale={locale} />}
    </div>
  );
}

function TestSpans({ row, run, locale }: { row: TestRow; run: PerfRun; locale: Locale }) {
  const t = performanceMessages[locale];
  const bars = timeline(row);
  const start = row.startNs;
  const resources = useMemo(() => {
    if (start === null) return null;
    const fromMs = start / 1e6;
    const toMs = (start + row.durationNs) / 1e6;
    const inside = run.samples.filter((s) => s.atMs >= fromMs - 100 && s.atMs <= toMs + 100);
    return inside.length > 1 ? inside : null;
  }, [run.samples, start, row.durationNs]);
  const curves = useMemo<Curve[] | null>(() => {
    if (!resources) return null;
    const first = resources[0]?.cpuUs ?? 0;
    return [
      {
        label: t.memoryAxis,
        values: resources.map((s) => s.memoryBytes),
        color: "--tim-perf-series-1",
        scale: "bytes",
        format: (v) => bytes(v, locale),
      },
      {
        label: t.cpuAxis,
        values: resources.map((s) => s.cpuUs - first),
        color: "--tim-perf-series-2",
        scale: "cpu",
        format: (v) => duration(v * 1000, locale),
      },
    ];
  }, [resources, t, locale]);
  const xs = useMemo(() => resources?.map((s) => s.atMs) ?? [], [resources]);
  const formatX = useMemo(() => (ms: number) => duration(ms * 1e6, locale), [locale]);
  return (
    <div className="tim-perf__spans">
      <h2 className="tim-perf__heading">{t.spansOf(row.test.name)}</h2>
      {bars.length <= 1 && <p className="tim-perf__note">{t.noSpans}</p>}
      {/* An image of the tree: the call tree says it in words, for screen readers. */}
      <div className="tim-flame" role="img" aria-label={t.spansOf(row.test.name)}>
        {bars.map((line, depth) => (
          <div key={depth} className="tim-flame__row">
            {line.map((bar) => {
              const name = run.nodes[bar.node.span.node ?? ""] ?? null;
              const time = duration(bar.node.span.durationNs, locale);
              return (
                <div
                  key={bar.node.span.id}
                  className="tim-flame__bar"
                  data-status={bar.node.span.status}
                  data-root={depth === 0 || undefined}
                  title={t.barTitle(bar.node.span.name, time, name, bar.node.span.status === "error")}
                  style={{ left: `${bar.left * 100}%`, width: `max(${bar.width * 100}%, 2px)` }}
                >
                  <span className="tim-flame__label">
                    {bar.node.span.status === "error" && (
                      <span className="tim-flame__error">
                        <ToneIcon tone="violation" /> {t.spanError}{" "}
                      </span>
                    )}
                    {bar.node.span.name} <span className="tim-flame__time">{time}</span>
                  </span>
                </div>
              );
            })}
          </div>
        ))}
      </div>
      <h2 className="tim-perf__heading">{t.resources}</h2>
      {curves ? (
        <Curves label={t.resources} xs={xs} xLabel={t.time} formatX={formatX} curves={curves} />
      ) : (
        <p className="tim-perf__note">{t.noResources}</p>
      )}
    </div>
  );
}

function Tree({ run, locale }: { run: PerfRun; locale: Locale }) {
  const t = performanceMessages[locale];
  const tree = useMemo(() => callTree(testRows(run)), [run]);
  const [open, setOpen] = useState<Set<string>>(() => new Set());
  const whole = Math.max(1, ...tree.map((n) => n.totalNs));
  const toggle = (key: string) =>
    setOpen((was) => {
      const next = new Set(was);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  const rows: { node: CallNode; level: number }[] = [];
  const walk = (nodes: CallNode[], level: number) => {
    for (const node of nodes) {
      rows.push({ node, level });
      if (open.has(node.key)) walk(node.children, level + 1);
    }
  };
  walk(tree, 1);
  return (
    <>
      <table className="tim-calltree" role="treegrid" aria-label={t.callTree}>
        <thead>
          <tr>
            <th scope="col">{t.columns.name}</th>
            <th scope="col">{t.columns.total}</th>
            <th scope="col">{t.columns.self}</th>
            <th scope="col">{t.columns.calls}</th>
            <th scope="col">{t.columns.memory}</th>
            <th scope="col">{t.columns.cpu}</th>
          </tr>
        </thead>
        <tbody>
          {rows.map(({ node, level }) => {
            const resources = resourcesDuring(run.samples, node.windows);
            const expandable = node.children.length > 0;
            return (
              <tr key={node.key} aria-level={level} aria-expanded={expandable ? open.has(node.key) : undefined}>
                <td>
                  <span className="tim-calltree__name" style={{ paddingInlineStart: `${(level - 1) * 1.1}em` }}>
                    {expandable ? (
                      <button
                        type="button"
                        className="tim-calltree__toggle"
                        aria-expanded={open.has(node.key)}
                        onClick={() => toggle(node.key)}
                      >
                        <span aria-hidden="true">{open.has(node.key) ? "▾" : "▸"}</span> {node.name}
                      </button>
                    ) : (
                      <span className="tim-calltree__leaf">{node.name}</span>
                    )}
                    {node.node && <span className="tim-calltree__node">{run.nodes[node.node] ?? node.node}</span>}
                  </span>
                </td>
                <td>
                  <span className="tim-calltree__total">
                    <span className="tim-perf__share" aria-hidden="true">
                      <span style={{ width: `${share(node.totalNs, whole) * 100}%` }} />
                    </span>
                    {duration(node.totalNs, locale)}
                  </span>
                </td>
                <td>{duration(node.selfNs, locale)}</td>
                <td>{node.calls}</td>
                <td>{resources ? bytes(resources.memoryBytes, locale) : "—"}</td>
                <td>{resources ? duration(resources.cpuUs * 1000, locale) : "—"}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
      <p className="tim-perf__note">{t.resourcesNote}</p>
    </>
  );
}

/** What a regression is on, in the page's language: the daemon names the run and tests in English. */
function regressionLabel(r: Regression, t: PerformanceMessages): string {
  if (r.key === "run") return t.theRun;
  if (r.key.startsWith("test:")) return t.testNamed(r.label.replace(/^test /, ""));
  return r.label;
}

/** The regressions of the latest batch (ADR 0053): probable in the tone of attention, confirmed
 * in that of a violation, each with its shape and its word. */
function Regressions({ report, locale }: { report: RegressionReport | null | undefined; locale: Locale }) {
  const t = performanceMessages[locale];
  if (!report) return null;
  return (
    <section className="tim-perf__regressions" aria-label={t.regressionsTitle}>
      <h2 className="tim-perf__heading">{t.regressionsTitle}</h2>
      {report.noteKind ? (
        <p className="tim-perf__note">{t.regressionNote[report.noteKind](report.main ?? "main")}</p>
      ) : report.regressions.length === 0 ? (
        <p className="tim-perf__note">{t.noRegression(report.checked)}</p>
      ) : (
        <ul className="tim-plain tim-perf__regression-list">
          {report.regressions.map((r) => (
            <li key={r.key} className="tim-perf__regression">
              <Badge tone={r.status === "confirmed" ? "violation" : "attention"} label={t.regressionStatus[r.status]} />
              <span>{t.regressionWords(regressionLabel(r, t), r.verdict.change, r.verdict.pValue)}</span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function Evolution({ runs, of, locale }: { runs: RunSummary[]; of: RunSummary | null; locale: Locale }) {
  const t = performanceMessages[locale];
  const points = useMemo(() => evolution(runs, of), [runs, of]);
  const xs = useMemo(() => points.map((_, i) => i), [points]);
  const curves = useMemo<Curve[]>(
    () => [
      {
        label: t.points.wall,
        values: points.map((p) => p.wallS),
        color: "--tim-perf-series-1",
        scale: "s",
        format: (v) => duration(v * 1e9, locale),
      },
      {
        label: t.points.cpu,
        values: points.map((p) => p.cpuS),
        color: "--tim-perf-series-2",
        scale: "s",
        format: (v) => duration(v * 1e9, locale),
      },
    ],
    [points, t, locale],
  );
  const formatX = useMemo(() => (i: number) => points[i]?.label ?? "", [points]);
  if (points.length === 0) return <p className="tim-perf__note">{t.emptyText}</p>;
  return (
    <>
      <h2 className="tim-perf__heading">{t.evolution}</h2>
      <Curves label={t.evolution} xs={xs} xLabel={t.points.commit} formatX={formatX} curves={curves} height={200} />
      <p className="tim-perf__note">{t.evolutionNote}</p>
    </>
  );
}
