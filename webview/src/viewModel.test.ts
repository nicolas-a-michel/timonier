import { describe, expect, it } from "vitest";

import {
  NO_REVIEW,
  type HostMessage,
  type ViewDelta,
  type ViewEdge,
  type ViewNode,
  type ViewSnapshot,
} from "./protocol";
import { initialModel, reduce, type ViewModel } from "./viewModel";

function node(id: string, name = id): ViewNode {
  return {
    id,
    kind: "package",
    name,
    parent: null,
    role: null,
    language: null,
    children: 0,
    file: null,
    span: null,
    review: null,
  };
}

function edge(src: string, dst: string): ViewEdge {
  return { src, dst, kind: "calls", weight: 1, edges: 1, best: "tree_sitter", approximateOnly: true, review: null };
}

function snapshot(subscription: number, revision: number, nodes: ViewNode[], edges: ViewEdge[] = []): HostMessage {
  const value: ViewSnapshot = { subscription, revision, nodes, edges, review: NO_REVIEW };
  return { type: "snapshot", snapshot: value };
}

function delta(partial: Partial<ViewDelta> & Pick<ViewDelta, "base">): HostMessage {
  const value: ViewDelta = {
    subscription: 1,
    revision: partial.base + 1,
    nodes: [],
    removedNodes: [],
    edges: [],
    removedEdges: [],
    review: null,
    ...partial,
  };
  return { type: "delta", delta: value };
}

/** Applies `messages` in order; returns the model and every effect. */
function run(messages: HostMessage[], from: ViewModel = initialModel) {
  let model = from;
  const effects = [];
  for (const message of messages) {
    const reduced = reduce(model, message);
    model = reduced.model;
    effects.push(...reduced.effects);
  }
  return { model, effects };
}

describe("view model", () => {
  it("takes a snapshot whole, and says what it shows", () => {
    const { model, effects } = run([
      snapshot(1, 0, [node("pkg:cargo/a"), node("pkg:cargo/b")], [edge("pkg:cargo/a", "pkg:cargo/b")]),
    ]);
    expect(model.view?.nodes.size).toBe(2);
    expect(model.view?.edges.size).toBe(1);
    expect(effects).toEqual([{ type: "rendered", subscription: 1, revision: 0, nodes: 2 }]);
  });

  it("keeps the review summary of a snapshot, and takes the one a delta brings (ADR 0024)", () => {
    const since = { seq: 2, timestampMs: 1 };
    const first = reduce(initialModel, {
      type: "snapshot",
      snapshot: { subscription: 1, revision: 0, nodes: [node("pkg:a")], edges: [], review: { ...NO_REVIEW, since } },
    }).model;
    expect(first.view?.review.since).toEqual(since);
    const unchanged = reduce(first, delta({ base: 0, nodes: [node("pkg:a", "a")] })).model;
    expect(unchanged.view?.review.since).toEqual(since);
    const modified = {
      ...NO_REVIEW,
      since,
      modified: [{ id: "pkg:a", kind: "package" as const, name: "a", role: null, within: null }],
    };
    const next = reduce(unchanged, delta({ base: 1, review: modified })).model;
    expect(next.view?.review).toEqual(modified);
  });

  it("applies a delta from its revision", () => {
    const { model, effects } = run([
      snapshot(1, 0, [node("pkg:cargo/a"), node("pkg:cargo/b")], [edge("pkg:cargo/a", "pkg:cargo/b")]),
      delta({
        base: 0,
        nodes: [node("pkg:cargo/c"), node("pkg:cargo/a", "renamed")],
        removedNodes: ["pkg:cargo/b"],
        removedEdges: [{ src: "pkg:cargo/a", dst: "pkg:cargo/b", kind: "calls" }],
        edges: [edge("pkg:cargo/a", "pkg:cargo/c")],
      }),
    ]);
    expect([...(model.view?.nodes.keys() ?? [])].sort()).toEqual(["pkg:cargo/a", "pkg:cargo/c"]);
    expect(model.view?.nodes.get("pkg:cargo/a")?.name).toBe("renamed");
    expect(model.view?.edges.size).toBe(1);
    expect(model.view?.revision).toBe(1);
    expect(effects.at(-1)).toEqual({ type: "rendered", subscription: 1, revision: 1, nodes: 2 });
  });

  it("asks once for the whole view after a gap, then follows the new snapshot", () => {
    const start = run([snapshot(1, 0, [node("pkg:cargo/a")])]).model;
    // Revision 1 went missing: 1 → 2 and 2 → 3 do not apply.
    const gap = run([delta({ base: 1 }), delta({ base: 2 })], start);
    expect(gap.effects).toEqual([{ type: "resync" }]);
    expect(gap.model.view?.revision).toBe(0);
    expect(gap.model.resyncing).toBe(true);
    const back = run(
      [
        snapshot(1, 3, [node("pkg:cargo/a"), node("pkg:cargo/z")]),
        delta({ base: 2 }),
        delta({ base: 3, nodes: [node("pkg:cargo/y")] }),
      ],
      gap.model,
    );
    expect(back.model.resyncing).toBe(false);
    expect(back.model.view?.revision).toBe(4);
    expect(back.model.view?.nodes.size).toBe(3);
    // The stale delta (2 → 3) was ignored without asking again.
    expect(back.effects.filter((e) => e.type === "resync")).toEqual([]);
  });

  it("ignores another subscription's deltas, and deltas before any snapshot", () => {
    expect(run([delta({ base: 0 })]).effects).toEqual([]);
    const start = run([snapshot(2, 0, [node("pkg:cargo/a")])]).model;
    const other = run([delta({ subscription: 1, base: 0, nodes: [node("pkg:cargo/x")] })], start);
    expect(other.model).toBe(start);
    expect(other.effects).toEqual([]);
  });

  it("replaces the view with a snapshot of a restarted daemon", () => {
    const before = run([snapshot(4, 9, [node("pkg:cargo/a")])]).model;
    const { model } = run(
      [
        { type: "daemon", state: "restarting", detail: "gone" },
        { type: "daemon", state: "connected", detail: null },
        snapshot(1, 0, [node("pkg:cargo/a"), node("pkg:cargo/b")]),
      ],
      before,
    );
    expect(model.daemon).toEqual({ state: "connected", detail: null });
    expect(model.view?.subscription).toBe(1);
    expect(model.view?.revision).toBe(0);
    expect(model.view?.nodes.size).toBe(2);
  });
});
