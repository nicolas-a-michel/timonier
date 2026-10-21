import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { IntentKeeper, RETRY_DELAY_MS, SAVE_DELAY_MS, type IntentPort } from "../../intentKeeper";
import type { HostMessage, Intent, IntentRead } from "../../protocol/generated";

const intent = (name: string): Intent => ({
  version: 1,
  layers: [],
  components: [{ id: "C-1", name, layer: null, role: "" }],
  edges: [],
  notes: [],
  budgets: [],
  sketch: { shapes: {}, strokes: [], next: { C: 2 } },
});

/** Timers the test moves by hand. */
class Clock {
  now = 0;
  private due: { at: number; run: () => void; id: number }[] = [];
  private next = 1;
  setTimeout = ((run: () => void, delay?: number) => {
    const id = this.next++;
    this.due.push({ at: this.now + (delay ?? 0), run, id });
    return id;
  }) as unknown as typeof globalThis.setTimeout;
  clearTimeout = ((id: number) => {
    this.due = this.due.filter((timer) => timer.id !== id);
  }) as unknown as typeof globalThis.clearTimeout;
  async advance(ms: number): Promise<void> {
    this.now += ms;
    for (;;) {
      const ready = this.due.filter((timer) => timer.at <= this.now);
      if (ready.length === 0) break;
      this.due = this.due.filter((timer) => timer.at > this.now);
      for (const timer of ready) timer.run();
      await settle();
    }
    await settle();
  }
}

const settle = () => new Promise((resolve) => setImmediate(resolve));

/** The daemon and the page, as the keeper sees them. */
function port(file: IntentRead = { intent: null, problem: null }) {
  const posted: HostMessage[] = [];
  const writes: Intent[] = [];
  const state = { file, failing: undefined as Error | undefined };
  const fake: IntentPort = {
    read: () => Promise.resolve(state.file),
    write: (written) => {
      if (state.failing) return Promise.reject(state.failing);
      writes.push(written);
      state.file = { intent: written, problem: null };
      return Promise.resolve({ intent: written, changed: true });
    },
    post: (message) => posted.push(message),
  };
  return { fake, posted, writes, state };
}

describe("the whiteboard's keeper (Q66, ADR 0039)", () => {
  it("gives the page the file when it opens, and writes the gestures of a moment once", async () => {
    const { fake, posted, writes } = port({ intent: intent("Parser"), problem: null });
    const clock = new Clock();
    const keeper = new IntentKeeper(fake, clock);
    await keeper.opened();
    assert.deepEqual(posted, [{ type: "intent", intent: intent("Parser"), problem: null }]);
    keeper.changed(intent("Lexer"));
    keeper.changed(intent("Lexer 2"));
    await clock.advance(SAVE_DELAY_MS - 1);
    assert.equal(writes.length, 0);
    await clock.advance(1);
    assert.deepEqual(writes, [intent("Lexer 2")]);
    assert.deepEqual(posted.at(-1), { type: "intentSaved", saved: true, detail: null });
  });

  it("gives a page that reloads the board as it was left, not the file", async () => {
    const { fake, posted } = port();
    const clock = new Clock();
    const keeper = new IntentKeeper(fake, clock);
    keeper.changed(intent("Unsaved"));
    await keeper.opened();
    assert.deepEqual(posted, [{ type: "intent", intent: intent("Unsaved"), problem: null }]);
  });

  it("tries again while the daemon cannot write, and says so", async () => {
    const { fake, posted, writes, state } = port();
    const clock = new Clock();
    const keeper = new IntentKeeper(fake, clock);
    state.failing = new Error("not connected to the daemon");
    keeper.changed(intent("Parser"));
    await clock.advance(SAVE_DELAY_MS);
    assert.deepEqual(posted.at(-1), { type: "intentSaved", saved: false, detail: "not connected to the daemon" });
    state.failing = undefined;
    await clock.advance(RETRY_DELAY_MS);
    assert.deepEqual(writes, [intent("Parser")]);
    assert.deepEqual(posted.at(-1), { type: "intentSaved", saved: true, detail: null });
  });

  it("gives a page that opened before the daemon its board once the daemon is there", async () => {
    const { fake, posted, state } = port({ intent: intent("Parser"), problem: null });
    const keeper = new IntentKeeper(
      { ...fake, read: () => Promise.reject(new Error("the daemon is not running")) },
      new Clock(),
    );
    await keeper.opened();
    assert.deepEqual(posted, [{ type: "intentSaved", saved: false, detail: "the daemon is not running" }]);
    const later = new IntentKeeper(fake, new Clock());
    await later.connected();
    assert.deepEqual(posted.at(-1), { type: "intent", intent: state.file.intent, problem: null });
    // Once it has its board, a reconnection does not give it again: its history stays.
    const count = posted.length;
    await later.connected();
    assert.equal(posted.length, count);
  });

  it("does not try again a board the daemon refuses", async () => {
    const { fake, writes, state } = port();
    const clock = new Clock();
    const keeper = new IntentKeeper(fake, clock);
    state.failing = Object.assign(new Error("intent.json: E-1 does not join"), { code: -32602 });
    keeper.changed(intent("Parser"));
    await clock.advance(SAVE_DELAY_MS + RETRY_DELAY_MS * 3);
    assert.equal(writes.length, 0);
  });

  it("gives the page a file changed outside it, and ignores its own writes", async () => {
    const { fake, posted, state } = port();
    const clock = new Clock();
    const keeper = new IntentKeeper(fake, clock);
    await keeper.opened();
    keeper.changed(intent("Mine"));
    await clock.advance(SAVE_DELAY_MS);
    const count = posted.length;
    await keeper.fileChanged();
    assert.equal(posted.length, count, "its own write");
    state.file = { intent: intent("From git"), problem: null };
    await keeper.fileChanged();
    assert.deepEqual(posted.at(-1), { type: "intent", intent: intent("From git"), problem: null });
    // A gesture not written yet wins over the file.
    keeper.changed(intent("Mine again"));
    state.file = { intent: intent("Other"), problem: null };
    await keeper.fileChanged();
    assert.deepEqual(posted.at(-1), { type: "intent", intent: intent("From git"), problem: null });
  });

  it("writes what is pending when the panel closes", async () => {
    const { fake, writes } = port();
    const keeper = new IntentKeeper(fake, new Clock());
    keeper.changed(intent("Last"));
    await keeper.flush();
    assert.deepEqual(writes, [intent("Last")]);
  });

  it("passes on a file that does not read, and writes nothing until a gesture", async () => {
    const { fake, posted, writes } = port({ intent: null, problem: "intent.json: expected value" });
    const clock = new Clock();
    const keeper = new IntentKeeper(fake, clock);
    await keeper.opened();
    assert.deepEqual(posted, [{ type: "intent", intent: null, problem: "intent.json: expected value" }]);
    await clock.advance(RETRY_DELAY_MS * 2);
    assert.equal(writes.length, 0);
  });
});
