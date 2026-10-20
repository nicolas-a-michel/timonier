import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { densityOf, onPageMessage, onSettingsChange, type MapHostActions } from "../../mapHost";
import type { Density, HostMessage, WebviewMessage } from "../../protocol/generated";

/** What the panel did, in order. */
function recording(density: Density = "comfortable") {
  const done: string[] = [];
  const posted: HostMessage[] = [];
  const relayed: WebviewMessage[] = [];
  const actions: MapHostActions = {
    post: (message) => {
      posted.push(message);
      done.push(`post ${message.type}`);
    },
    density: () => density,
    ready: () => done.push("ready"),
    markReviewed: () => done.push("markReviewed"),
    relay: (message) => {
      relayed.push(message);
      done.push(`relay ${message.type}`);
    },
  };
  return { actions, done, posted, relayed };
}

describe("the map panel (review n° 2 of Phase 5, m-6)", () => {
  it("gives a new page its density before the view (Q32)", () => {
    const { actions, done, posted } = recording("compact");
    onPageMessage({ type: "ready", protocol: 3 }, actions);
    assert.deepEqual(done, ["ready", "post density", "relay ready"]);
    assert.deepEqual(posted, [{ type: "density", density: "compact" }]);
  });

  it("follows the setting while the map is open", () => {
    const { actions, posted } = recording("compact");
    onSettingsChange((section) => section === "timonier.map.density", actions);
    assert.deepEqual(posted, [{ type: "density", density: "compact" }]);
    // Another setting: nothing.
    const other = recording();
    onSettingsChange((section) => section === "timonier.daemon.path", other.actions);
    assert.deepEqual(other.posted, []);
  });

  it("reads the setting as comfortable unless compact", () => {
    assert.equal(densityOf("compact"), "compact");
    for (const setting of ["comfortable", undefined, "dense", 3]) assert.equal(densityOf(setting), "comfortable");
  });

  it("runs « Mark as reviewed » for the map's button, without the relay (ADR 0024)", () => {
    const { actions, done } = recording();
    onPageMessage({ type: "markReviewed" }, actions);
    assert.deepEqual(done, ["markReviewed"]);
  });

  it("relays the view's messages, and drops what is not a message of the page", () => {
    const { actions, done, relayed } = recording();
    onPageMessage({ type: "subscribe", view: { expanded: ["pkg:a"] } }, actions);
    // A message of the host, sent back by a page gone wrong.
    onPageMessage({ type: "density", density: "compact" }, actions);
    onPageMessage(null, actions);
    assert.deepEqual(relayed, [{ type: "subscribe", view: { expanded: ["pkg:a"] } }]);
    assert.deepEqual(done, ["relay subscribe"]);
  });
});
