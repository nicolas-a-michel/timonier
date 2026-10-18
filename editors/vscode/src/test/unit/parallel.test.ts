import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { askAgain, inParallel } from "../../parallel";

describe("questions in parallel", () => {
  it("keep the order of the items, whatever order they finish in", async () => {
    const delays = [30, 5, 20, 1, 10];
    const results = await inParallel(delays, 2, async (delay) => {
      await new Promise((resolve) => setTimeout(resolve, delay));
      return delay * 2;
    });
    assert.deepEqual(results, [60, 10, 40, 2, 20]);
  });

  it("never run more than the limit at once", async () => {
    let running = 0;
    let most = 0;
    await inParallel([...Array(20).keys()], 3, async () => {
      running++;
      most = Math.max(most, running);
      await new Promise((resolve) => setTimeout(resolve, 2));
      running--;
    });
    assert.equal(most, 3);
  });

  it("answer nothing for nothing", async () => {
    assert.deepEqual(await inParallel([], 8, () => Promise.resolve(1)), []);
  });
});

describe("doubtful questions asked again", () => {
  const noWait = () => Promise.resolve();

  it("ask only those still doubtful, until none is", async () => {
    // Doubtful: a negative answer; the server is sure the second time it is asked.
    const answer = [-1, 2, -3];
    const asked: number[][] = [];
    let round = 0;
    await askAgain(
      answer,
      (a) => a.flatMap((x, i) => (x < 0 ? [i] : [])),
      [10, 20, 40],
      () => false,
      (indices) => {
        asked.push(indices);
        round++;
        return Promise.resolve(indices.map((i) => (round === 1 && i === 2 ? -3 : 10 + i)));
      },
      noWait,
    );
    assert.deepEqual(asked, [[0, 2], [2]]);
    assert.deepEqual(answer, [10, 2, 12]);
  });

  it("stop at the last pause, doubtful or not", async () => {
    let asked = 0;
    await askAgain(
      [-1],
      () => [0],
      [10, 20],
      () => false,
      () => {
        asked++;
        return Promise.resolve([-1]);
      },
      noWait,
    );
    assert.equal(asked, 2);
  });

  it("stop once the asker no longer waits, before or during a pause (review n° 8, X5)", async () => {
    let asked = 0;
    let cancelled = false;
    await askAgain(
      [-1],
      () => [0],
      [10, 20, 40],
      () => cancelled,
      () => {
        asked++;
        return Promise.resolve([-1]);
      },
      () => {
        // Cancelled while it pauses, the second time.
        if (asked === 1) cancelled = true;
        return Promise.resolve();
      },
    );
    assert.equal(asked, 1);
    asked = 0;
    await askAgain(
      [-1],
      () => [0],
      [10],
      () => true,
      () => {
        asked++;
        return Promise.resolve([-1]);
      },
      noWait,
    );
    assert.equal(asked, 0);
  });
});
