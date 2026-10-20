import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { Emitter, type MessageConnection } from "vscode-jsonrpc/node";

import type { DaemonStatus } from "../../daemon";
import { METHODS, type HostMessage, type TrackingSnapshot, type TrackingView } from "../../protocol/generated";
import { TrackingRelay } from "../../trackingRelay";

const EMPTY: TrackingView = {
  milestones: [],
  open: null,
  todo: [],
  sessions: [],
  sessionsTotal: 0,
  problems: [],
  review: {
    since: null,
    added: [],
    modified: [],
    removed: [],
    dependenciesAdded: [],
    dependenciesRemoved: [],
    cycles: [],
    fans: [],
  },
  debt: null,
  sessionsSinceReview: 0,
  look: [],
  configured: false,
};

function update(subscription: number, revision: number, open: string | null = null): TrackingSnapshot {
  return { subscription, revision, tracking: { ...EMPTY, open } };
}

/** A daemon's connection: each subscription gets the next number, after a turn of the loop. */
class FakeConnection {
  readonly requests: { method: string; params: unknown }[] = [];
  constructor(private next: number) {}

  async sendRequest<T>(method: string, params: unknown): Promise<T> {
    this.requests.push({ method, params });
    await new Promise((resolve) => setImmediate(resolve));
    if (method === METHODS.TRACKING_SUBSCRIBE) return update(this.next++, 0) as T;
    return null as T;
  }
}

class FakeDaemon {
  status: DaemonStatus = { state: "connected", detail: null };
  readonly state = new Emitter<DaemonStatus>();
  private readonly connect = new Emitter<MessageConnection>();
  readonly updates = new Emitter<TrackingSnapshot>();
  readonly onDidChangeState = this.state.event;
  readonly onDidConnect = this.connect.event;
  readonly onDidReceiveTracking = this.updates.event;
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

const settle = () => new Promise((resolve) => setImmediate(resolve)).then(() => new Promise((r) => setImmediate(r)));

function page(relay: TrackingRelay) {
  const posted: HostMessage[] = [];
  const attached = relay.attach((message) => {
    posted.push(message);
    return Promise.resolve(true);
  });
  const revisions = () => posted.flatMap((m) => (m.type === "tracking" ? [m.revision] : []));
  return { posted, attached, revisions };
}

const quiet = { warn: () => undefined };

describe("the tracking relay", () => {
  it("subscribes once the first panel opens, and gives each page the state", async () => {
    const daemon = new FakeDaemon(new FakeConnection(1));
    const relay = new TrackingRelay(daemon, quiet);
    assert.equal(daemon.fake.requests.length, 0, "no panel, no subscription");
    const plan = page(relay);
    await settle();
    assert.deepEqual(plan.posted[0], { type: "daemon", state: "connected", detail: null });
    assert.deepEqual(plan.revisions(), [0]);
    const todo = page(relay);
    await settle();
    // One subscription for both, the second page served from what came.
    assert.equal(daemon.fake.requests.filter((r) => r.method === METHODS.TRACKING_SUBSCRIBE).length, 1);
    assert.deepEqual(todo.revisions(), [0]);
    daemon.updates.fire(update(1, 1, "S-2026-10-20-1"));
    assert.deepEqual(plan.revisions(), [0, 1]);
    assert.deepEqual(todo.revisions(), [0, 1]);
    assert.equal(relay.state?.tracking.open, "S-2026-10-20-1");
  });

  it("drops the updates of another subscription, and the late ones", async () => {
    const daemon = new FakeDaemon(new FakeConnection(4));
    const relay = new TrackingRelay(daemon, quiet);
    const plan = page(relay);
    await settle();
    daemon.updates.fire(update(9, 1));
    daemon.updates.fire(update(4, 2));
    daemon.updates.fire(update(4, 1));
    assert.deepEqual(plan.revisions(), [0, 2]);
  });

  it("subscribes again on a new connection, and tells the pages the daemon's state", async () => {
    const daemon = new FakeDaemon(new FakeConnection(1));
    const relay = new TrackingRelay(daemon, quiet);
    const plan = page(relay);
    await settle();
    daemon.status = { state: "restarting", detail: "lost" };
    daemon.state.fire(daemon.status);
    const fresh = new FakeConnection(1);
    daemon.restart(fresh);
    await settle();
    assert.equal(fresh.requests[0]?.method, METHODS.TRACKING_SUBSCRIBE);
    // Numbered from 1 again by the new daemon: still the one to show.
    daemon.updates.fire(update(1, 1));
    assert.deepEqual(plan.revisions(), [0, 0, 1]);
    assert.ok(plan.posted.some((m) => m.type === "daemon" && m.state === "restarting"));
  });

  it("forgets a page that went away", async () => {
    const daemon = new FakeDaemon(new FakeConnection(1));
    const relay = new TrackingRelay(daemon, quiet);
    const plan = page(relay);
    await settle();
    plan.attached.dispose();
    daemon.updates.fire(update(1, 1));
    assert.deepEqual(plan.revisions(), [0]);
  });

  it("remembers which revision each panel shows", () => {
    const daemon = new FakeDaemon(new FakeConnection(1));
    const relay = new TrackingRelay(daemon, quiet);
    relay.receive({ type: "trackingShown", panel: "todo", revision: 2 });
    assert.equal(relay.shown("todo"), 2);
    assert.equal(relay.shown("plan"), undefined);
  });

  it("unsubscribes when disposed", async () => {
    const daemon = new FakeDaemon(new FakeConnection(5));
    const relay = new TrackingRelay(daemon, quiet);
    page(relay);
    await settle();
    await relay.dispose();
    assert.deepEqual(daemon.fake.requests.at(-1), { method: METHODS.UNSUBSCRIBE, params: { subscription: 5 } });
  });

  it("tells its watchers each state, subscribing for them too", async () => {
    const daemon = new FakeDaemon(new FakeConnection(1));
    const relay = new TrackingRelay(daemon, quiet);
    const seen: number[] = [];
    relay.watch((snapshot) => seen.push(snapshot.revision));
    await settle();
    assert.deepEqual(seen, [0], "subscribed without any page");
    daemon.updates.fire(update(1, 1));
    assert.deepEqual(seen, [0, 1]);
    const late: number[] = [];
    relay.watch((snapshot) => late.push(snapshot.revision));
    assert.deepEqual(late, [1], "what came last, at once");
  });

  it("waits without a daemon: the pages hear why", () => {
    const relay = new TrackingRelay(
      { status: { state: "unavailable", detail: "no folder" }, client: undefined },
      quiet,
    );
    const plan = page(relay);
    assert.deepEqual(plan.posted, [{ type: "daemon", state: "unavailable", detail: "no folder" }]);
  });
});
