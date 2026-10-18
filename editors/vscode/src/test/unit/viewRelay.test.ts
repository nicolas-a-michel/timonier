import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { Emitter, type MessageConnection } from "vscode-jsonrpc/node";

import type { DaemonStatus } from "../../daemon";
import { METHODS, type HostMessage, type Pin, type ViewDelta, type ViewSnapshot } from "../../protocol/generated";
import { ViewRelay } from "../../viewRelay";

/** A daemon's connection that keeps its subscriptions, and answers after a turn of the loop. */
class FakeConnection {
  readonly live = new Set<number>();
  readonly notifications: { method: string; params: unknown }[] = [];
  /** Unsubscriptions of a subscription it did not know. */
  unknown = 0;
  failResync = false;
  constructor(private next: number) {}

  async sendRequest<T>(method: string, params: { subscription?: number }): Promise<T> {
    await new Promise((resolve) => setImmediate(resolve));
    switch (method) {
      case METHODS.SUBSCRIBE: {
        const subscription = this.next++;
        this.live.add(subscription);
        return snapshot(subscription) as T;
      }
      case METHODS.RESYNC:
        if (this.failResync) throw Object.assign(new Error("internal"), { code: -32603 });
        return snapshot(params.subscription ?? 0) as T;
      case METHODS.UNSUBSCRIBE:
        if (!this.live.delete(params.subscription ?? 0)) this.unknown++;
        return null as T;
      default:
        throw new Error(method);
    }
  }

  sendNotification(method: string, params: unknown): Promise<void> {
    this.notifications.push({ method, params });
    return Promise.resolve();
  }
}

function snapshot(subscription: number): ViewSnapshot {
  return { subscription, revision: 0, nodes: [], edges: [] };
}

/** A supervisor: its connection can change, as when the daemon restarts. */
class FakeDaemon {
  readonly status: DaemonStatus = { state: "connected", detail: null };
  private readonly state = new Emitter<DaemonStatus>();
  private readonly connect = new Emitter<MessageConnection>();
  readonly deltas = new Emitter<ViewDelta>();
  readonly onDidChangeState = this.state.event;
  readonly onDidConnect = this.connect.event;
  readonly onDidReceiveDelta = this.deltas.event;
  client: MessageConnection | undefined;

  constructor(public fake: FakeConnection) {
    this.client = fake as unknown as MessageConnection;
  }

  restart(fake: FakeConnection): void {
    this.fake = fake;
    this.client = fake as unknown as MessageConnection;
    this.connect.fire(this.client);
  }
}

function relay(daemon: FakeDaemon) {
  const posted: HostMessage[] = [];
  const warnings: string[] = [];
  const relay = new ViewRelay(
    daemon,
    (message) => {
      posted.push(message);
      return Promise.resolve(true);
    },
    { warn: (message: string) => void warnings.push(message) },
  );
  return { relay, posted, warnings };
}

const subscribe = (expanded: string[] = []) => ({ type: "subscribe", view: { expanded } }) as const;
const snapshots = (posted: HostMessage[]) => posted.filter((message) => message.type === "snapshot").length;

describe("the view relay", () => {
  it("closes each subscription it replaces, even when asked twice at once", async () => {
    const daemon = new FakeDaemon(new FakeConnection(1));
    const { relay: r, posted } = relay(daemon);
    await Promise.all([r.receive(subscribe()), r.receive(subscribe(["a"])), r.receive(subscribe(["b"]))]);
    assert.deepEqual([...daemon.fake.live], [3]);
    assert.equal(daemon.fake.unknown, 0);
    assert.equal(snapshots(posted), 3);
  });

  it("keeps one subscription when a new connection and the webview cross", async () => {
    const daemon = new FakeDaemon(new FakeConnection(1));
    const { relay: r } = relay(daemon);
    await r.receive(subscribe());
    const old = daemon.fake;
    const fresh = new FakeConnection(10);
    daemon.restart(fresh);
    await r.receive(subscribe(["a"]));
    assert.equal(fresh.live.size, 1);
    assert.equal(fresh.unknown, 0);
    // The old daemon took its subscriptions with it: nothing is asked of it.
    assert.deepEqual([...old.live], [1]);
  });

  it("subscribes again when a resync fails", async () => {
    const daemon = new FakeDaemon(new FakeConnection(1));
    const { relay: r, posted, warnings } = relay(daemon);
    await r.receive(subscribe());
    daemon.fake.failResync = true;
    await r.receive({ type: "resync" });
    assert.equal(snapshots(posted), 2);
    assert.deepEqual([...daemon.fake.live], [2]);
    assert.equal(warnings.length, 1);
  });

  it("forwards the deltas of its subscription only", async () => {
    const daemon = new FakeDaemon(new FakeConnection(1));
    const { relay: r, posted } = relay(daemon);
    await r.receive(subscribe());
    const delta = (subscription: number) => ({ subscription, base: 0, revision: 1 }) as unknown as ViewDelta;
    daemon.deltas.fire(delta(7));
    daemon.deltas.fire(delta(1));
    await new Promise((resolve) => setImmediate(resolve));
    assert.deepEqual(
      posted.filter((message) => message.type === "delta").map((message) => message.delta.subscription),
      [1],
    );
  });

  it("tells the daemon which files the map draws, and tells a new daemon too", async () => {
    const daemon = new FakeDaemon(new FakeConnection(1));
    const { relay: r } = relay(daemon);
    await r.receive(subscribe());
    await r.receive({ type: "displayed", paths: ["src/a.ts", "src/b.ts"] });
    assert.deepEqual(daemon.fake.notifications, [
      { method: METHODS.DISPLAYED, params: { paths: ["src/a.ts", "src/b.ts"] } },
    ]);
    const fresh = new FakeConnection(10);
    daemon.restart(fresh);
    await new Promise((resolve) => setTimeout(resolve, 10));
    assert.deepEqual(fresh.notifications, [{ method: METHODS.DISPLAYED, params: { paths: ["src/a.ts", "src/b.ts"] } }]);
    assert.deepEqual([...fresh.live], [10]);
  });

  it("closes its subscription when disposed", async () => {
    const daemon = new FakeDaemon(new FakeConnection(1));
    const { relay: r } = relay(daemon);
    await r.receive(subscribe());
    await r.dispose();
    assert.deepEqual([...daemon.fake.live], []);
  });

  it("reports what the page shows of its current subscription only", async () => {
    const daemon = new FakeDaemon(new FakeConnection(1));
    const { relay: r } = relay(daemon);
    await r.receive(subscribe());
    // Read anew each time: an assertion would narrow a property read once.
    const shown = () => r.rendered;
    await r.receive({ type: "rendered", subscription: 1, revision: 0, nodes: 6 });
    assert.deepEqual(shown(), { subscription: 1, revision: 0, nodes: 6 });
    // Replaced: what it showed says nothing of the new one, and a late word of it neither.
    await r.receive(subscribe(["a"]));
    assert.equal(shown(), undefined);
    await r.receive({ type: "rendered", subscription: 1, revision: 1, nodes: 6 });
    assert.equal(shown(), undefined);
    await r.receive({ type: "rendered", subscription: 2, revision: 0, nodes: 9 });
    assert.equal(shown()?.subscription, 2);
    // A revision never sent: a late word of the daemon before a restart, numbered alike.
    await r.receive({ type: "rendered", subscription: 2, revision: 7, nodes: 9 });
    assert.equal(shown()?.revision, 0);
    // A hidden panel loses its page, and what it showed.
    r.unloaded();
    assert.equal(shown(), undefined);
  });

  it("gives each new page the boxes moved by hand, and keeps those it moves", async () => {
    const daemon = new FakeDaemon(new FakeConnection(1));
    let kept: Record<string, Pin> = { "@role/utility": { parent: "@root", x: 10, y: 20 } };
    const posted: HostMessage[] = [];
    const r = new ViewRelay(
      daemon,
      (message) => {
        posted.push(message);
        return Promise.resolve(true);
      },
      { warn: () => undefined },
      undefined,
      {
        get: () => kept,
        set: (pins) => {
          kept = pins;
          return Promise.resolve();
        },
      },
    );
    await r.receive({ type: "ready", protocol: 2 });
    assert.deepEqual(
      posted.find((message) => message.type === "pins"),
      { type: "pins", pins: { "@role/utility": { parent: "@root", x: 10, y: 20 } } },
    );
    await r.receive({ type: "pins", pins: { "@role/interface": { parent: "@root", x: 0, y: 64 } } });
    assert.deepEqual(kept, { "@role/interface": { parent: "@root", x: 0, y: 64 } });
  });
});
