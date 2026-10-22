/**
 * PLAN.md 4.8, on Timonier's own view (test/views/timonier/open.json, every role open): a file
 * added, an edge added, an edge removed. Unchanged boxes must stay where they were.
 */
import { readFileSync } from "node:fs";
import path from "node:path";

import { describe, expect, it } from "vitest";

import { NO_REVIEW, type ViewEdge, type ViewNode, type ViewSnapshot } from "../protocol";
import { edgeKey, type ViewData } from "../viewModel";
import { BOX, layout } from "./layout";
import { mapContent, roleGroupId } from "./mapModel";
import { absolute, meanDisplacement } from "./stability";

const fixture = path.join(import.meta.dirname, "..", "..", "test", "views", "timonier", "open.json");
const open = (JSON.parse(readFileSync(fixture, "utf8")) as { snapshot: ViewSnapshot }).snapshot;

function data(nodes: ViewNode[], edges: ViewEdge[]): ViewData {
  return {
    subscription: 1,
    revision: 0,
    nodes: new Map(nodes.map((n) => [n.id, n])),
    edges: new Map(edges.map((e) => [edgeKey(e), e])),
    review: NO_REVIEW,
  };
}

const allRoles = new Set(open.nodes.filter((n) => n.parent === null).map((n) => roleGroupId(n.role)));
const before = mapContent(data(open.nodes, open.edges), allRoles);

/** Three saves: a new file in the daemon, a new call, a call gone. */
function changes(): { name: string; nodes: ViewNode[]; edges: ViewEdge[] }[] {
  const daemonFile = open.nodes.find((n) => n.kind === "file" && n.id.startsWith("file:crates/timonier-daemon/"));
  if (!daemonFile) throw new Error("no file of the daemon in the fixture");
  const added: ViewNode = { ...daemonFile, id: "file:crates/timonier-daemon/src/added.rs", name: "added.rs" };
  const files = open.nodes.filter((n) => n.kind === "file");
  const first = files[0];
  const last = files.at(-1);
  if (!first || !last) throw new Error("no files");
  const call: ViewEdge = {
    src: last.id,
    dst: first.id,
    kind: "calls",
    weight: 1,
    edges: 1,
    best: "tree_sitter",
    approximateOnly: true,
    review: null,
  };
  return [
    { name: "a file added", nodes: [...open.nodes, added], edges: open.edges },
    { name: "a call added", nodes: open.nodes, edges: [...open.edges, call] },
    { name: "a call removed", nodes: open.nodes, edges: open.edges.slice(1) },
  ];
}

/** The mean displacement of unchanged boxes the layout is held to: a quarter of a box's height. */
const THRESHOLD = BOX.height / 4;

describe("layout stability", () => {
  it(`keeps unchanged boxes within ${THRESHOLD} px on average`, async () => {
    const start = performance.now();
    const first = await layout(before);
    const settled = performance.now() - start;
    const placedBefore = absolute(before, first);
    const report: string[] = [];
    const kept: number[] = [];
    for (const change of changes()) {
      const after = mapContent(data(change.nodes, change.edges), allRoles);
      const t = performance.now();
      const next = await layout(after, { content: before, placed: first });
      const elapsed = performance.now() - t;
      const moved = meanDisplacement(placedBefore, absolute(after, next));
      const fresh = meanDisplacement(placedBefore, absolute(after, await layout(after)));
      kept.push(moved);
      report.push(
        `${change.name}: ${moved.toFixed(1)} px (${fresh.toFixed(1)} px from scratch), ${elapsed.toFixed(0)} ms`,
      );
    }
    console.log(`${before.items.length} items, first layout ${settled.toFixed(0)} ms\n${report.join("\n")}`);
    for (const moved of kept) expect(moved).toBeLessThan(THRESHOLD);
    // A dozen layouts of elkjs: more than vitest's 5 s on a slower runner (Windows CI).
  }, 30_000);
});
