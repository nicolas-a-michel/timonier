import type { BudgetsView, HostMessage, PerfMap, PerfRuns } from "./protocol/generated";

/** How often the runs are asked while the layer shows: a run that ends shows soon. */
export const PERF_POLL_MS = 3_000;

/** What the feed needs: the daemon's `perf/map` and `perf/runs`, and the map's page. */
export interface PerfMapPort {
  fetch(): Promise<PerfMap>;
  runs(): Promise<PerfRuns>;
  post(message: HostMessage): unknown;
}

/**
 * The map's Performances layer (ADR 0051): asked when the layer comes on, and again when a run
 * with traces ends; `null` to the page when the daemon cannot answer.
 */
export class PerfMapFeed {
  private on = false;
  private timer: ReturnType<typeof setInterval> | undefined;
  /** The newest finished run with traces the layer was built from. */
  private newest: string | null | undefined;
  /** The newest such run the list of runs last named (relecture n° 4, m-5). */
  private listed: string | null | undefined;
  /** Answers come in order: an older one never replaces a newer one. */
  private asked = 0;
  /** What the page was last given: for the tests. */
  last: PerfMap | null | undefined;

  constructor(
    private readonly port: PerfMapPort,
    private readonly timers: Pick<typeof globalThis, "setInterval" | "clearInterval"> = globalThis,
  ) {}

  /** The page turned the layer on or off. */
  async show(on: boolean): Promise<void> {
    this.on = on;
    if (this.timer !== undefined) this.timers.clearInterval(this.timer);
    this.timer = undefined;
    this.newest = undefined;
    this.listed = undefined;
    if (!on) return;
    this.timer = this.timers.setInterval(() => void this.poll(), PERF_POLL_MS);
    await this.refresh();
  }

  /** A run may have ended: the layer is built again when the newest one changed. */
  async poll(): Promise<void> {
    if (!this.on) return;
    let runs: PerfRuns;
    try {
      runs = await this.port.runs();
    } catch {
      return;
    }
    // The smoke tests of `doctor` are not what the map shows (relecture n° 3, I-1).
    const newest = runs.runs.find((r) => r.finished && r.traced && !r.smoke)?.run ?? null;
    // A change of the list, to a run the layer is not of: when none of the listed runs has
    // traces, the layer is of an older one, and must not be asked for at every tick.
    const changed = newest !== this.listed && newest !== this.newest;
    this.listed = newest;
    if (changed) await this.refresh();
  }

  dispose(): void {
    this.on = false;
    if (this.timer !== undefined) this.timers.clearInterval(this.timer);
  }

  private async refresh(): Promise<void> {
    const asked = ++this.asked;
    let perf: PerfMap | null;
    let failed = false;
    try {
      perf = await this.port.fetch();
    } catch {
      perf = null;
      failed = true;
    }
    if (asked !== this.asked || !this.on) return;
    // Not built: the next tick asks again (relecture n° 5, m-1).
    this.newest = failed ? undefined : (perf?.run ?? null);
    if (failed) this.listed = undefined;
    this.last = perf;
    this.port.post({ type: "perfMap", perf });
  }
}

/** What the budgets' feed needs: the daemon's `perf/budgets` and `perf/runs`, and the map. */
export interface BudgetPort {
  fetch(): Promise<BudgetsView | null>;
  runs(): Promise<PerfRuns>;
  post(message: HostMessage): unknown;
}

/**
 * The whiteboard's budgets for the map, always while it shows (Q74, ADR 0052): given when its page
 * opens, and again when a run with traces ends or the whiteboard changes; `null` without budgets
 * or daemon.
 */
export class BudgetFeed {
  private timer: ReturnType<typeof setInterval> | undefined;
  private newest: string | null | undefined;
  private asked = 0;
  /** What the page was last given: for the tests. */
  last: BudgetsView | null | undefined;

  constructor(
    private readonly port: BudgetPort,
    private readonly timers: Pick<typeof globalThis, "setInterval" | "clearInterval"> = globalThis,
  ) {}

  /** The map's page opened: the budgets now, and while it shows. */
  async start(): Promise<void> {
    this.stop();
    this.timer = this.timers.setInterval(() => void this.poll(), PERF_POLL_MS);
    await this.refresh();
  }

  /** The page is gone (hidden, closed). */
  stop(): void {
    if (this.timer !== undefined) this.timers.clearInterval(this.timer);
    this.timer = undefined;
    this.newest = undefined;
  }

  /** The whiteboard changed: its budgets again. */
  async changed(): Promise<void> {
    if (this.timer !== undefined) await this.refresh();
  }

  async poll(): Promise<void> {
    if (this.timer === undefined) return;
    let runs: PerfRuns;
    try {
      runs = await this.port.runs();
    } catch {
      return;
    }
    // The smoke tests of `doctor` are not what the map shows (relecture n° 3, I-1).
    const newest = runs.runs.find((r) => r.finished && r.traced && !r.smoke)?.run ?? null;
    if (newest !== this.newest) {
      this.newest = newest;
      await this.refresh();
    }
  }

  private async refresh(): Promise<void> {
    const asked = ++this.asked;
    let budgets: BudgetsView | null;
    try {
      budgets = await this.port.fetch();
    } catch {
      budgets = null;
      // Not judged: the next tick asks again (relecture n° 5, m-1).
      this.newest = undefined;
    }
    if (asked !== this.asked) return;
    this.last = budgets;
    this.port.post({ type: "budgets", budgets });
  }
}
