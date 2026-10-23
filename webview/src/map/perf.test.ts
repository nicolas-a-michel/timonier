/** The map's Performances layer (PLAN.md 8.7, ADR 0051, Q76). */
import { describe, expect, it } from "vitest";

import { NO_REVIEW, type PerfMap, type Role, type ViewEdge, type ViewNode } from "../protocol";
import { edgeKey, type ViewData } from "../viewModel";
import { mapContent, roleGroupId } from "./mapModel";
import { perfMarks } from "./perf";
import { percent } from "../performance/model";

function node(id: string, role: Role, parent: string | null = null, children = 0): ViewNode {
  return {
    id,
    kind: parent ? "component" : "package",
    name: id.replace(/^.*[/:]/, ""),
    parent,
    role,
    language: null,
    children,
    file: null,
    span: null,
    signature: null,
    review: null,
    debt: null,
    summary: null,
  };
}

function data(nodes: ViewNode[], edges: ViewEdge[]): ViewData {
  return {
    subscription: 1,
    revision: 0,
    nodes: new Map(nodes.map((n) => [n.id, n])),
    edges: new Map(edges.map((e) => [edgeKey(e), e])),
    review: NO_REVIEW,
  };
}

const app = node("pkg:cargo/app", "entry_point", null, 1);
const store = node("pkg:cargo/store", "persistence");
const calls: ViewEdge = {
  src: "pkg:cargo/app",
  dst: "pkg:cargo/store",
  kind: "calls",
  weight: 1,
  edges: 1,
  best: "lsp",
  approximateOnly: false,
  confirmed: 1,
  review: null,
};

/** `app` entered for 100 ms, its `ui` called from it for 30, the store from `ui` for 20. */
const measured: PerfMap = {
  run: "R-1",
  nodes: [
    { node: "pkg:cargo/app", calls: 1, selfNs: 50e6, enteredNs: 100e6 },
    { node: "component:dir/app/ui", calls: 2, selfNs: 10e6, enteredNs: 0 },
    { node: "pkg:cargo/store", calls: 3, selfNs: 20e6, enteredNs: 0 },
  ],
  links: [
    { from: "pkg:cargo/app", to: "component:dir/app/ui", calls: 2, ns: 30e6 },
    { from: "component:dir/app/ui", to: "pkg:cargo/store", calls: 3, ns: 20e6 },
  ],
  ancestors: { "pkg:cargo/app": [], "component:dir/app/ui": ["pkg:cargo/app"], "pkg:cargo/store": [] },
};

describe("the map's Performances layer", () => {
  it("gives each box its cumulative time, what it was entered for, and its own time", () => {
    const groups = new Set([roleGroupId("entry_point"), roleGroupId("persistence")]);
    const view = data([app, store], [calls]);
    const marks = perfMarks(measured, mapContent(view, groups), view);
    // `ui` is inside `app`: its call from `app` adds nothing to app's 100 ms.
    // Shares of the 80 ms of own time: they add up, the cumulative times do not.
    expect(marks.items.get("pkg:cargo/app")).toEqual({
      calls: 3,
      totalNs: 100e6,
      selfNs: 60e6,
      share: 0.75,
      largest: true,
    });
    // The store, entered from another box.
    expect(marks.items.get("pkg:cargo/store")).toEqual({
      calls: 3,
      totalNs: 20e6,
      selfNs: 20e6,
      share: 0.25,
      largest: false,
    });
    expect(marks.most).toBe(3);
  });

  it("says shares in words, the small ones too", () => {
    expect(percent(0.75, "en")).toBe("75%");
    expect(percent(0.75, "fr")).toBe("75\u00a0%");
    expect(percent(0.001, "en")).toBe("< 1%");
    expect(percent(0, "en")).toBe("0%");
  });
});
