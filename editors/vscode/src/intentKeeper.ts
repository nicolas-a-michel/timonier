import type { HostMessage, Intent, IntentRead, IntentWritten } from "./protocol/generated";

/** What the keeper needs: the daemon's two methods, and the page. */
export interface IntentPort {
  read(): Promise<IntentRead>;
  write(intent: Intent): Promise<IntentWritten>;
  post(message: HostMessage): unknown;
}

/** How long after a gesture the board is written: the gestures of a moment go together. */
export const SAVE_DELAY_MS = 250;
/** How long before trying again when the daemon could not write it. */
export const RETRY_DELAY_MS = 2000;

/** The daemon refused the board itself, not the moment: trying again would not help. */
const isRefusal = (error: unknown) =>
  typeof error === "object" && error !== null && (error as { code?: unknown }).code === -32602;

const message = (error: unknown) => (error instanceof Error ? error.message : String(error));

/**
 * Keeps the whiteboard in `.timonier/intent.json` (Q66, ADR 0039): writes it shortly after each
 * gesture, tries again while the daemon cannot, gives the page the board when it opens, and the
 * file's when it changes outside the whiteboard (a `git checkout`, a hand edit).
 */
export class IntentKeeper {
  /** The board the page last sent, or the file last read. */
  private latest: Intent | null | undefined;
  /** A gesture not written yet. */
  private pending = false;
  /** The file as last written or read, to tell its own writes from other changes. */
  private known: string | undefined;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private writing: Promise<void> | undefined;
  private disposed = false;

  constructor(
    private readonly port: IntentPort,
    private readonly timers: Pick<typeof globalThis, "setTimeout" | "clearTimeout"> = globalThis,
  ) {}

  /** The page is ready: it gets the board, as it was left or as the file holds it. */
  async opened(): Promise<void> {
    if (this.latest !== undefined) {
      this.port.post({ type: "intent", intent: this.latest, problem: null });
      return;
    }
    await this.load();
  }

  /** The daemon is there (again): a page still waiting gets its board, what is pending goes. */
  async connected(): Promise<void> {
    if (this.latest === undefined) await this.load();
    else if (this.pending) this.schedule(0);
  }

  /** A gesture on the whiteboard: the board is written shortly. */
  changed(intent: Intent): void {
    this.latest = intent;
    this.pending = true;
    this.schedule(SAVE_DELAY_MS);
  }

  /** The file changed: if not by the whiteboard, the page gets it. Local gestures win. */
  async fileChanged(): Promise<void> {
    if (this.pending || this.writing) return;
    let read: IntentRead;
    try {
      read = await this.port.read();
    } catch {
      return;
    }
    if (this.pending || this.writing || JSON.stringify(read.intent) === this.known) return;
    this.take(read);
  }

  /** Writes what is pending now: the panel closes. */
  async flush(): Promise<void> {
    if (this.timer !== undefined) this.timers.clearTimeout(this.timer);
    this.timer = undefined;
    await this.writing;
    if (this.pending) await this.save();
  }

  dispose(): void {
    this.disposed = true;
    if (this.timer !== undefined) this.timers.clearTimeout(this.timer);
  }

  private async load(): Promise<void> {
    try {
      this.take(await this.port.read());
    } catch (error) {
      // No daemon yet: the page waits for it, with the reason.
      this.port.post({ type: "intentSaved", saved: false, detail: message(error) });
    }
  }

  private take(read: IntentRead): void {
    this.known = JSON.stringify(read.intent);
    this.latest = read.problem === null ? read.intent : undefined;
    this.port.post({ type: "intent", intent: read.intent, problem: read.problem });
  }

  private schedule(delay: number): void {
    if (this.disposed) return;
    if (this.timer !== undefined) this.timers.clearTimeout(this.timer);
    this.timer = this.timers.setTimeout(() => {
      this.timer = undefined;
      void this.save();
    }, delay);
  }

  private async save(): Promise<void> {
    if (this.writing) {
      // One write at a time; the gesture that came meanwhile goes after it.
      await this.writing;
      if (this.pending) this.schedule(0);
      return;
    }
    const intent = this.latest;
    if (!intent) return;
    this.pending = false;
    this.writing = (async () => {
      try {
        const written = await this.port.write(intent);
        this.known = JSON.stringify(written.intent);
        if (!this.pending) this.port.post({ type: "intentSaved", saved: true, detail: null });
      } catch (error) {
        this.port.post({ type: "intentSaved", saved: false, detail: message(error) });
        if (!isRefusal(error) && !this.pending) {
          this.pending = true;
          this.schedule(RETRY_DELAY_MS);
        }
      }
    })();
    try {
      await this.writing;
    } finally {
      this.writing = undefined;
    }
  }
}
