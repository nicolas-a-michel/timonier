import type * as vscode from "vscode";
import { Emitter, type MessageConnection } from "vscode-jsonrpc/node";

import type { DaemonStatus, DaemonSupervisor } from "./daemon";
import {
  METHODS,
  type HostMessage,
  type MapCommand,
  type Pin,
  type Span,
  type ViewDelta,
  type ViewSnapshot,
  type ViewSpec,
  type WebviewMessage,
} from "./protocol/generated";

/** The daemon's error code for a subscription it does not know (ADR 0019). */
const UNKNOWN_SUBSCRIPTION = -32002;

/** What the webview last said it shows. */
export interface Rendered {
  subscription: number;
  revision: number;
  nodes: number;
}

/** Where a relay finds its daemon: a supervisor, or nothing when no folder is open. */
export type DaemonSource =
  | Pick<DaemonSupervisor, "status" | "client" | "onDidChangeState" | "onDidConnect" | "onDidReceiveDelta">
  | { readonly status: DaemonStatus; readonly client: undefined };

/** Where the relay writes what went wrong: the « Timonier » output channel. */
export type Log = Pick<vscode.LogOutputChannel, "warn">;

/** Where the boxes moved by hand are kept, for the next pages (ADR 0021, H13). */
export interface PinStore {
  get(): Record<string, Pin>;
  set(pins: Record<string, Pin>): Thenable<void>;
}

/**
 * The relay between one webview and the daemon (ADR 0019): the webview asks for a view, the
 * relay subscribes to it and forwards the snapshot, then the deltas. After a restart of the
 * daemon, it subscribes again and sends the whole view.
 *
 * Subscribing, resyncing and closing run one at a time: each one starts from the subscription
 * the one before left, so that none is replaced without being closed.
 */
export class ViewRelay implements vscode.Disposable {
  private readonly disposables: { dispose(): unknown }[] = [];
  private readonly renderedEmitter = new Emitter<Rendered>();
  /** Each time the webview has applied a snapshot or a delta. */
  readonly onDidRender = this.renderedEmitter.event;
  private view: ViewSpec | undefined;
  private subscription: number | undefined;
  /** The connection the subscription was made on. */
  private connection: MessageConnection | undefined;
  private last: Rendered | undefined;
  /** The revision of the subscription last sent to the page. */
  private posted: number | undefined;
  /** The page listens: it said `ready`, and has not been hidden since. */
  private live = false;
  /** Commands that came while the page was loading. */
  private queued: MapCommand[] = [];
  /** The files the map draws: told to each new connection too. */
  private displayed: string[] = [];
  /** The operation running, and those queued after it. */
  private chain: Promise<void> = Promise.resolve();

  constructor(
    private readonly daemon: DaemonSource,
    private readonly post: (message: HostMessage) => Thenable<boolean>,
    private readonly log: Log,
    /** The webview asks for code: a file relative to the root, at a span of the model. */
    private readonly openCode: (file: string, span: Span | null) => Promise<unknown> = () => Promise.resolve(undefined),
    private readonly pins?: PinStore,
  ) {
    if ("onDidChangeState" in daemon) {
      this.disposables.push(
        daemon.onDidChangeState((status) => this.tell(status)),
        daemon.onDidConnect((connection) => void this.connected(connection)),
        daemon.onDidReceiveDelta((delta) => this.forward(delta)),
      );
    }
  }

  get rendered(): Rendered | undefined {
    return this.last;
  }

  /** A message from the webview. */
  async receive(message: WebviewMessage): Promise<void> {
    switch (message.type) {
      case "ready":
        this.live = true;
        this.tell(this.daemon.status);
        // Before the view: the page lays out the boxes moved by hand where they were left.
        if (this.pins) void this.send({ type: "pins", pins: this.pins.get() });
        for (const command of this.queued.splice(0)) void this.send({ type: "command", command });
        return;
      case "subscribe":
        this.view = message.view;
        await this.serial(() => this.subscribe(this.daemon.client));
        return;
      case "resync":
        await this.serial(() => this.resync());
        return;
      case "displayed":
        this.displayed = message.paths;
        this.tellDisplayed(this.daemon.client);
        return;
      case "openCode":
        await this.openCode(message.file, message.span).catch((error: unknown) =>
          this.log.warn(`cannot open ${message.file}: ${String(error)}`),
        );
        return;
      case "pins":
        await Promise.resolve(this.pins?.set(message.pins)).catch((error: unknown) =>
          this.log.warn(`cannot keep the layout: ${String(error)}`),
        );
        return;
      case "markReviewed":
        // The panel runs the command (ADR 0024).
        return;
      case "rendered":
        // Only what the page shows of the current subscription: a page that is going away, or a
        // subscription just replaced, says nothing about the map. A daemon numbers its
        // subscriptions from 1: a revision not sent yet comes from the one before a restart.
        if (message.subscription !== this.subscription) return;
        if (this.posted !== undefined && message.revision > this.posted) return;
        this.last = { subscription: message.subscription, revision: message.revision, nodes: message.nodes };
        this.renderedEmitter.fire(this.last);
        return;
    }
  }

  /** A command of the palette, for the map; kept until the page listens. */
  command(command: MapCommand): Thenable<boolean> {
    if (this.live) return this.post({ type: "command", command });
    this.queued.push(command);
    return Promise.resolve(true);
  }

  /** The page went away (a hidden panel loses it): commands wait for the next one, and what it
   * showed is gone with it. */
  unloaded(): void {
    this.live = false;
    this.last = undefined;
  }

  /** Closes the subscription, after what is running. */
  dispose(): Promise<void> {
    for (const disposable of this.disposables) disposable.dispose();
    this.renderedEmitter.dispose();
    return this.serial(async () => {
      const { connection, subscription } = this;
      this.view = undefined;
      this.subscription = undefined;
      if (connection && subscription !== undefined) await unsubscribe(connection, subscription);
    });
  }

  /** `task` once the operations before it are done. */
  private serial(task: () => Promise<void>): Promise<void> {
    const run = this.chain.then(task);
    this.chain = run.catch(() => undefined);
    return run;
  }

  /** A new connection to the daemon: the view again, and the files it draws. */
  private async connected(connection: MessageConnection): Promise<void> {
    await this.serial(() => this.subscribe(connection));
    this.tellDisplayed(connection);
  }

  /** The files the map draws come first for refinement (ADR 0014). */
  private tellDisplayed(connection: MessageConnection | undefined): void {
    if (!connection || this.displayed.length === 0) return;
    connection
      .sendNotification(METHODS.DISPLAYED, { paths: this.displayed })
      .catch((error: unknown) => this.log.warn(`view/displayed failed: ${String(error)}`));
  }

  private tell(status: DaemonStatus): void {
    void this.send({ type: "daemon", state: status.state, detail: status.detail });
  }

  /** Subscribes to the webview's view on `connection`, in place of the previous subscription. */
  private async subscribe(connection: MessageConnection | undefined): Promise<void> {
    if (!connection || !this.view) return;
    // On another connection, the previous subscription went with the old daemon.
    const previous = this.connection === connection ? this.subscription : undefined;
    let snapshot: ViewSnapshot;
    try {
      snapshot = await connection.sendRequest<ViewSnapshot>(METHODS.SUBSCRIBE, { view: this.view });
    } catch (error) {
      this.log.warn(`view/subscribe failed: ${String(error)}`);
      return;
    }
    if (connection !== this.connection || snapshot.subscription !== this.subscription) this.last = undefined;
    this.connection = connection;
    this.subscription = snapshot.subscription;
    this.posted = snapshot.revision;
    await this.send({ type: "snapshot", snapshot });
    if (previous !== undefined && previous !== snapshot.subscription) await unsubscribe(connection, previous);
  }

  /** The webview saw a gap in the revisions: the whole view again. */
  private async resync(): Promise<void> {
    const connection = this.daemon.client;
    if (!connection || connection !== this.connection || this.subscription === undefined) {
      await this.subscribe(connection);
      return;
    }
    try {
      const snapshot = await connection.sendRequest<ViewSnapshot>(METHODS.RESYNC, {
        subscription: this.subscription,
      });
      this.posted = snapshot.revision;
      await this.send({ type: "snapshot", snapshot });
    } catch (error) {
      if ((error as { code?: unknown }).code === UNKNOWN_SUBSCRIPTION) this.subscription = undefined;
      else this.log.warn(`view/resync failed: ${String(error)}; subscribing again`);
      // The webview waits for a snapshot: without one, it would let every later gap pass.
      await this.subscribe(connection);
    }
  }

  /** A message to the webview; one that went away has nothing to say. */
  private async send(message: HostMessage): Promise<void> {
    await Promise.resolve(this.post(message)).catch(() => undefined);
  }

  private forward(delta: ViewDelta): void {
    if (delta.subscription !== this.subscription || this.connection !== this.daemon.client) return;
    // A webview in the background loses it: it sees the gap later and asks for a resync.
    this.posted = delta.revision;
    void this.send({ type: "delta", delta });
  }
}

async function unsubscribe(connection: MessageConnection, subscription: number): Promise<void> {
  await connection.sendRequest(METHODS.UNSUBSCRIBE, { subscription }).catch(() => undefined);
}
