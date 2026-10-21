import type { MessageConnection } from "vscode-jsonrpc/node";

import type { DaemonStatus, DaemonSupervisor } from "./daemon";
import { METHODS, type HostMessage, type TrackingSnapshot, type WebviewMessage } from "./protocol/generated";
import type { Log } from "./viewRelay";

/** Where the relay finds its daemon: a supervisor, or nothing when no folder is open. */
export type TrackingSource =
  | Pick<DaemonSupervisor, "status" | "client" | "onDidChangeState" | "onDidConnect" | "onDidReceiveTracking">
  | { readonly status: DaemonStatus; readonly client: undefined };

/** A page of a panel: where its messages go. */
type Post = (message: HostMessage) => Thenable<boolean>;

/**
 * The relay between the panels of the tracking and the daemon (ADR 0031): one subscription for
 * every panel, made when the first one opens and made again on each new connection. The daemon
 * sends the whole state at each change; each page gets the last one when it opens.
 */
export class TrackingRelay {
  private readonly disposables: { dispose(): unknown }[] = [];
  private readonly pages = new Set<Post>();
  private readonly watchers = new Set<(tracking: TrackingSnapshot) => void>();
  private readonly shownBy = new Map<string, number>();
  private latest: TrackingSnapshot | undefined;
  private connection: MessageConnection | undefined;
  /** A panel opened once: the subscription is kept, and made again after a restart. */
  private wanted = false;
  private chain: Promise<void> = Promise.resolve();

  constructor(
    private readonly daemon: TrackingSource,
    private readonly log: Log,
  ) {
    if ("onDidChangeState" in daemon) {
      this.disposables.push(
        daemon.onDidChangeState((status) => this.broadcast(daemonMessage(status))),
        daemon.onDidConnect((connection) => void this.serial(() => this.subscribe(connection))),
        daemon.onDidReceiveTracking((update) => this.receiveUpdate(update)),
      );
    }
  }

  /** The state last received. */
  get state(): TrackingSnapshot | undefined {
    return this.latest;
  }

  /** The revision a panel last said it shows. */
  shown(panel: string): number | undefined {
    return this.shownBy.get(panel);
  }

  /** `listener` gets the tracking now, if known, then at each change; the subscription is made
   * if it was not. For what shows outside the pages (the badge of the bridge). */
  watch(listener: (tracking: TrackingSnapshot) => void): { dispose(): void } {
    this.watchers.add(listener);
    if (this.latest) listener(this.latest);
    if (!this.wanted) {
      this.wanted = true;
      void this.serial(() => this.subscribe(this.daemon.client));
    }
    return { dispose: () => this.watchers.delete(listener) };
  }

  /** A page that said `ready`: it gets the daemon's state and the tracking, then each change. */
  attach(post: Post): { dispose(): void } {
    this.pages.add(post);
    void send(post, daemonMessage(this.daemon.status));
    if (this.latest) void send(post, trackingMessage(this.latest));
    if (!this.wanted) {
      this.wanted = true;
      void this.serial(() => this.subscribe(this.daemon.client));
    }
    return { dispose: () => this.pages.delete(post) };
  }

  /** A message from a panel's page, besides `ready`. */
  receive(message: WebviewMessage): void {
    if (message.type !== "trackingShown") return;
    this.shownBy.set(message.panel, message.revision);
  }

  /** Closes the subscription, after what is running. */
  dispose(): Promise<void> {
    for (const disposable of this.disposables) disposable.dispose();
    this.pages.clear();
    this.watchers.clear();
    return this.serial(async () => {
      const { connection, latest } = this;
      this.wanted = false;
      if (connection && latest) {
        await connection.sendRequest(METHODS.UNSUBSCRIBE, { subscription: latest.subscription }).catch(() => undefined);
      }
    });
  }

  private serial(task: () => Promise<void>): Promise<void> {
    const run = this.chain.then(task);
    this.chain = run.catch(() => undefined);
    return run;
  }

  private async subscribe(connection: MessageConnection | undefined): Promise<void> {
    if (!connection || !this.wanted || connection === this.connection) return;
    let snapshot: TrackingSnapshot;
    try {
      snapshot = await connection.sendRequest<TrackingSnapshot>(METHODS.TRACKING_SUBSCRIBE, null);
    } catch (error) {
      this.log.warn(`tracking/subscribe failed: ${String(error)}`);
      return;
    }
    this.connection = connection;
    this.latest = snapshot;
    this.broadcast(trackingMessage(snapshot));
    for (const watcher of this.watchers) watcher(snapshot);
  }

  private receiveUpdate(update: TrackingSnapshot): void {
    const latest = this.latest;
    // Another subscription's (a daemon before a restart), or one already overtaken.
    if (this.connection !== this.daemon.client || !latest) return;
    if (update.subscription !== latest.subscription || update.revision <= latest.revision) return;
    this.latest = update;
    this.broadcast(trackingMessage(update));
    for (const watcher of this.watchers) watcher(update);
  }

  private broadcast(message: HostMessage): void {
    for (const post of this.pages) void send(post, message);
  }
}

function trackingMessage(snapshot: TrackingSnapshot): HostMessage {
  return { type: "tracking", revision: snapshot.revision, tracking: snapshot.tracking };
}

function daemonMessage(status: DaemonStatus): HostMessage {
  return {
    type: "daemon",
    state: status.state,
    detail: status.detail,
    ...(status.progress ? { progress: status.progress } : {}),
  };
}

/** A message to a page; one that went away has nothing to say. */
async function send(post: Post, message: HostMessage): Promise<void> {
  await Promise.resolve(post(message)).catch(() => undefined);
}
