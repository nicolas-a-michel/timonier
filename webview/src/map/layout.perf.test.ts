import { it } from "vitest";

import { synthetic } from "../demo/synthetic";
import { edgeKey } from "../viewModel";
import { layout } from "./layout";
import { mapContent, roleGroupId } from "./mapModel";

// A measurement for PLAN.md 4.10, not a check: `BENCH=1 npx vitest run src/map/layout.perf`.
it.skipIf(!process.env.BENCH)(
  "lays out a synthetic repository",
  async () => {
    for (const size of [
      [20, 12, 12],
      [40, 12, 12],
    ] as const) {
      const open = synthetic(size[0], size[1], size[2]);
      const view = {
        subscription: 1,
        revision: 0,
        nodes: new Map(open.nodes.map((n) => [n.id, n])),
        edges: new Map(open.edges.map((e) => [edgeKey(e), e])),
      };
      const roles = new Set(open.nodes.filter((n) => n.parent === null).map((n) => roleGroupId(n.role)));
      let t = performance.now();
      const content = mapContent(view, roles);
      const model = performance.now() - t;
      t = performance.now();
      const placed = await layout(content);
      const first = performance.now() - t;
      t = performance.now();
      await layout(content, { content, placed });
      const next = performance.now() - t;
      console.log(
        `${size.join("x")}: ${content.items.length} items, ${content.edges.length} edges; model ${model.toFixed(0)} ms, first layout ${first.toFixed(0)} ms, next ${next.toFixed(0)} ms`,
      );
    }
  },
  300_000,
);
