import type { HostMessage, PerfRun, PerfRuns, RegressionReport } from "./protocol/generated";

/** How often the runs are asked again while the panel shows: a run that ends shows soon. */
export const POLL_MS = 3_000;

/** What the feed needs: the daemon's two methods, and the panel's page. */
export interface PerformancePort {
  runs(): Promise<PerfRuns>;
  /** The latest batch against the main branch (ADR 0053). */
  regressions(): Promise<RegressionReport>;
  run(run: string): Promise<PerfRun | null>;
  post(message: HostMessage): unknown;
}

/**
 * What the Performances panel shows (ADR 0051): the runs, given again when they change, and the
 * run the page asks for, again when it ends. `null` runs: the daemon does not answer.
 */
export class PerformanceFeed {
  private shownRuns = "";
  private chosen: string | null = null;
  /** Answers come in order: an older one never replaces a newer one. */
  private asked = 0;

  constructor(private readonly port: PerformancePort) {}

  /** The page opened, or the daemon came back: everything again. */
  async opened(): Promise<void> {
    this.shownRuns = "";
    await this.poll();
    if (this.chosen) await this.choose(this.chosen);
  }

  /** The runs, posted when they changed; the run shown again when it ended since. */
  async poll(): Promise<void> {
    let runs: PerfRuns;
    try {
      runs = await this.port.runs();
    } catch {
      if (this.shownRuns !== "down") {
        this.shownRuns = "down";
        this.port.post({ type: "daemon", state: "unavailable", detail: null });
      }
      return;
    }
    const said = JSON.stringify(runs);
    if (said === this.shownRuns) return;
    const before = this.shownRuns;
    this.shownRuns = said;
    if (before === "down") this.port.post({ type: "daemon", state: "connected", detail: null });
    this.port.post({ type: "perfRuns", runs });
    try {
      this.port.post({ type: "perfRegressions", report: await this.port.regressions() });
    } catch {
      this.port.post({ type: "perfRegressions", report: null });
    }
    // The run shown has just ended, or changed: given again.
    if (this.chosen && before !== "" && before !== "down") await this.choose(this.chosen);
  }

  /** The page shows `run`. */
  async choose(run: string): Promise<void> {
    this.chosen = run;
    const asked = ++this.asked;
    let shown: PerfRun | null;
    try {
      shown = await this.port.run(run);
    } catch {
      shown = null;
    }
    if (asked === this.asked) this.port.post({ type: "perfRun", run: shown });
  }
}
