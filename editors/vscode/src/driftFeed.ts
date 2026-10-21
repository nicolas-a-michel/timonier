import type { DriftView, HostMessage } from "./protocol/generated";

/** How long after a change of the code or of the whiteboard the drift is asked again. */
export const REFRESH_DELAY_MS = 500;

/** What the feed needs: the daemon's `intent/drift`, and the map's page. */
export interface DriftPort {
  fetch(): Promise<DriftView>;
  post(message: HostMessage): unknown;
}

/**
 * The drift of the code from the whiteboard, for the map while it compares them (ADR 0042):
 * asked when the comparison starts, and again shortly after the map shows a new revision or
 * `intent.json` changes; `null` to the page when the daemon cannot answer.
 */
export class DriftFeed {
  private on = false;
  private timer: ReturnType<typeof setTimeout> | undefined;
  /** Answers come in order: an older one never replaces a newer one. */
  private asked = 0;
  /** What the page was last given: for the tests. */
  last: DriftView | null | undefined;

  constructor(
    private readonly port: DriftPort,
    private readonly timers: Pick<typeof globalThis, "setTimeout" | "clearTimeout"> = globalThis,
  ) {}

  /** The page turned the comparison on or off. */
  async compare(on: boolean): Promise<void> {
    this.on = on;
    if (this.timer !== undefined) this.timers.clearTimeout(this.timer);
    this.timer = undefined;
    if (on) await this.refresh();
  }

  /** The code or the whiteboard changed: the drift is asked again shortly, if compared. */
  changed(): void {
    if (!this.on) return;
    if (this.timer !== undefined) this.timers.clearTimeout(this.timer);
    this.timer = this.timers.setTimeout(() => {
      this.timer = undefined;
      void this.refresh();
    }, REFRESH_DELAY_MS);
  }

  /** The page reloaded: it says again whether it compares. */
  dispose(): void {
    this.on = false;
    if (this.timer !== undefined) this.timers.clearTimeout(this.timer);
  }

  private async refresh(): Promise<void> {
    const asked = ++this.asked;
    let drift: DriftView | null;
    try {
      drift = await this.port.fetch();
    } catch {
      drift = null;
    }
    if (this.on && asked === this.asked) {
      this.last = drift;
      this.port.post({ type: "drift", drift });
    }
  }
}
