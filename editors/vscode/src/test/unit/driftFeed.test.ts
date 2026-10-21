import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { DriftFeed, REFRESH_DELAY_MS } from "../../driftFeed";
import type { DriftView, HostMessage } from "../../protocol/generated";

const view = (missing: string[]): DriftView => ({
  drawn: true,
  problem: null,
  names: {},
  ancestors: {},
  drift: { anchors: [], missing, forbidden: [], required: [], unchecked: [] },
});

/** Timers the test runs by hand. */
function clock() {
  let due: (() => void) | undefined;
  return {
    timers: {
      setTimeout: ((run: () => void) => {
        due = run;
        return 1;
      }) as unknown as typeof globalThis.setTimeout,
      clearTimeout: (() => {
        due = undefined;
      }) as unknown as typeof globalThis.clearTimeout,
    },
    fire: async () => {
      const run = due;
      due = undefined;
      run?.();
      await new Promise((resolve) => setImmediate(resolve));
    },
    pending: () => due !== undefined,
  };
}

describe("the drift for the map (ADR 0042)", () => {
  it("is given when the comparison starts, and again shortly after a change", async () => {
    const posted: HostMessage[] = [];
    let missing = ["C-1"];
    const { timers, fire, pending } = clock();
    const feed = new DriftFeed({ fetch: () => Promise.resolve(view(missing)), post: (m) => posted.push(m) }, timers);
    feed.changed();
    assert.equal(pending(), false, "not compared: nothing asked");
    await feed.compare(true);
    assert.deepEqual(posted, [{ type: "drift", drift: view(["C-1"]) }]);
    missing = [];
    feed.changed();
    feed.changed();
    assert.ok(pending(), `asked again after ${REFRESH_DELAY_MS} ms`);
    await fire();
    assert.deepEqual(posted.at(-1), { type: "drift", drift: view([]) });
    assert.equal(posted.length, 2, "the changes of a moment, once");
    await feed.compare(false);
    feed.changed();
    assert.equal(pending(), false);
  });

  it("says when there is none, and never gives an old answer after a newer one", async () => {
    const posted: HostMessage[] = [];
    const answers: ((v: DriftView) => void)[] = [];
    const feed = new DriftFeed({
      fetch: () => new Promise<DriftView>((resolve) => answers.push(resolve)),
      post: (m) => posted.push(m),
    });
    const first = feed.compare(true);
    const second = feed.compare(true);
    answers[1]?.(view(["new"]));
    answers[0]?.(view(["old"]));
    await Promise.all([first, second]);
    assert.deepEqual(posted, [{ type: "drift", drift: view(["new"]) }]);

    const none: HostMessage[] = [];
    const down = new DriftFeed({ fetch: () => Promise.reject(new Error("no daemon")), post: (m) => none.push(m) });
    await down.compare(true);
    assert.deepEqual(none, [{ type: "drift", drift: null }]);
  });
});
