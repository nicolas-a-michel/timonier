/**
 * The incremental layout makes room as one would on a board (ADR 0021): on Timonier's own top
 * view (test/views/timonier/top.json), opening a role pushes the next columns right, so that
 * edges keep going left to right, and closing it puts everything back.
 */
import { readFileSync } from "node:fs";
import path from "node:path";

import { describe, expect, it } from "vitest";

import type { ViewEdge, ViewNode, ViewSnapshot } from "../protocol";
import { edgeKey, type ViewData } from "../viewModel";
import { layout, ROOT, type Layout } from "./layout";
import { mapContent, roleGroupId } from "./mapModel";

const fixture = path.join(import.meta.dirname, "..", "..", "test", "views", "timonier", "top.json");
const top = (JSON.parse(readFileSync(fixture, "utf8")) as { snapshot: ViewSnapshot }).snapshot;
const view: ViewData = {
  subscription: 1,
  revision: 0,
  nodes: new Map(top.nodes.map((n) => [n.id, n])),
  edges: new Map(top.edges.map((e) => [edgeKey(e), e])),
};

describe("the incremental layout", () => {
  it("pushes what came after an opened role to its right, and back when it closes", async () => {
    const group = roleGroupId("entry_point");
    const closed = mapContent(view, new Set());
    const opened = mapContent(view, new Set([group]));
    const first = await layout(closed);
    const open = await layout(opened, { content: closed, placed: first });

    const before = first.get(group);
    const after = open.get(group);
    if (!before || !after) throw new Error("the entry points are not on the map");
    expect(after.width).toBeGreaterThan(before.width);
    const roles = closed.items.filter((item) => item.kind === "role" && item.id !== group);
    const right = roles.filter((item) => (first.get(item.id)?.x ?? 0) >= before.x + before.width);
    expect(right.length).toBeGreaterThan(0);
    for (const item of right) expect(open.get(item.id)?.x).toBeGreaterThanOrEqual(after.x + after.width);

    const back = await layout(closed, { content: opened, placed: open });
    for (const item of closed.items) expect(back.get(item.id), item.id).toEqual(first.get(item.id));
  });

  it("moves nothing when nothing changed", async () => {
    const content = mapContent(view, new Set([roleGroupId("utility")]));
    const first = await layout(content);
    const again = await layout(content, { content, placed: first });
    expect(again).toEqual(first);
  });

  it("puts a newcomer after the column of what calls it", async () => {
    const group = roleGroupId("entry_point");
    const before = mapContent(view, new Set([group]));
    const first = await layout(before);
    const cli = top.nodes.find((n) => n.id === "pkg:cargo/timonier-cli");
    if (!cli) throw new Error("no timonier-cli in the fixture");
    const added: ViewNode = { ...cli, id: "pkg:cargo/timonier-added", name: "timonier-added" };
    const call: ViewEdge = {
      src: cli.id,
      dst: added.id,
      kind: "calls",
      weight: 1,
      edges: 1,
      best: "tree_sitter",
      approximateOnly: true,
    };
    const grown: ViewData = {
      ...view,
      nodes: new Map([...view.nodes, [added.id, added]]),
      edges: new Map([...view.edges, [edgeKey(call), call]]),
    };
    const after = mapContent(grown, new Set([group]));
    const next = await layout(after, { content: before, placed: first });
    const caller = next.get(cli.id);
    const newcomer = next.get(added.id);
    if (!caller || !newcomer) throw new Error("not laid out");
    // Right of its caller: the edge goes left to right.
    expect(newcomer.x).toBeGreaterThanOrEqual(caller.x + caller.width);
  });

  it("keeps a box moved by hand where it was left, and moves down what it covers", async () => {
    const content = mapContent(view, new Set());
    const first = await layout(content);
    const moved = roleGroupId("persistence");
    const under = roleGroupId("utility");
    const target = first.get(under);
    if (!target) throw new Error("no utility box");
    // Dropped right onto the utilities.
    const pins = { [moved]: { parent: ROOT, x: Math.round(target.x), y: Math.round(target.y) } };
    for (const next of [
      await layout(content, { content, placed: first }, pins),
      await layout(content, undefined, pins),
    ]) {
      const pinned = next.get(moved);
      const pushed = next.get(under);
      if (!pinned || !pushed) throw new Error("not laid out");
      expect([pinned.x, pinned.y]).toEqual([Math.round(target.x), Math.round(target.y)]);
      expect(pushed.y).toBeGreaterThanOrEqual(pinned.y + pinned.height);
      expect(overlaps(next)).toEqual([]);
    }
  });

  it("ignores a pin made in another container", async () => {
    const content = mapContent(view, new Set());
    const first = await layout(content);
    const pins = { [roleGroupId("persistence")]: { parent: roleGroupId("utility"), x: 0, y: 0 } };
    expect(await layout(content, { content, placed: first }, pins)).toEqual(first);
  });
});

/** Pairs of items of one container that overlap. */
function overlaps(placed: Layout): string[] {
  const items = [...placed];
  const found: string[] = [];
  for (const [i, [a, p]] of items.entries()) {
    for (const [b, q] of items.slice(i + 1)) {
      const apart = p.x + p.width <= q.x || q.x + q.width <= p.x || p.y + p.height <= q.y || q.y + q.height <= p.y;
      if (!apart) found.push(`${a} × ${b}`);
    }
  }
  return found;
}
