/**
 * The map compared with the whiteboard (PLAN.md 7.6, ADR 0042): a drawn component shows on its
 * box, or on what holds it when the box is not drawn; a forbidden edge shows on the line that
 * stands for the dependencies crossing it.
 */
import { describe, expect, it } from "vitest";

import { NO_REVIEW, type DriftView, type Role, type ViewEdge, type ViewNode } from "../protocol";
import { edgeKey, type ViewData } from "../viewModel";
import { intentMarks, shownBy } from "./intent";
import { mapContent, roleGroupId } from "./mapModel";

function node(id: string, role: Role | null, parent: string | null = null, children = 0): ViewNode {
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
  };
}

function edge(src: string, dst: string): ViewEdge {
  return {
    src,
    dst,
    kind: "calls",
    weight: 1,
    edges: 1,
    best: "lsp",
    approximateOnly: false,
    review: null,
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

/** `app` (its `ui` component calls the store) and `store`, as the daemon compares them. */
const drift: DriftView = {
  drawn: true,
  problem: null,
  names: { "L-1": "Front", "L-2": "Data", "C-1": "App", "C-2": "Web", "C-3": "Store", "C-4": "Ui" },
  ancestors: { "pkg:cargo/app": [], "component:dir/app/ui": ["pkg:cargo/app"], "pkg:cargo/store": [] },
  drift: {
    anchors: [
      { component: "C-1", name: "App", actual: "pkg:cargo/app", candidates: 1 },
      { component: "C-2", name: "Web", actual: null, candidates: 0 },
      { component: "C-3", name: "Store", actual: "pkg:cargo/store", candidates: 1 },
      { component: "C-4", name: "Ui", actual: "component:dir/app/ui", candidates: 1 },
    ],
    missing: ["C-2"],
    forbidden: [
      {
        edge: "E-1",
        src: "L-1",
        dst: "L-2",
        dependencies: [{ from: "component:dir/app/ui", to: "pkg:cargo/store", kinds: ["calls"], weight: 1 }],
      },
    ],
    required: [],
    unchecked: [],
  },
};

const app = node("pkg:cargo/app", "entry_point", null, 1);
const store = node("pkg:cargo/store", "persistence", null, 0);
const ui = node("component:dir/app/ui", "interface", "pkg:cargo/app", 0);

describe("the map compared with the whiteboard", () => {
  it("shows a drawn component on its role group when the map opens", () => {
    const view = data([app, store], [edge("pkg:cargo/app", "pkg:cargo/store")]);
    const content = mapContent(view, new Set());
    const marks = intentMarks(drift, content, view);
    expect(marks.items.get(roleGroupId("entry_point"))).toEqual({
      drawn: [],
      within: ["App", "Ui"],
      members: [{ id: "pkg:cargo/app", name: "app", drawn: true, names: ["App", "Ui"] }],
    });
    expect(marks.items.get(roleGroupId("persistence"))).toEqual({
      drawn: [],
      within: ["Store"],
      members: [{ id: "pkg:cargo/store", name: "store", drawn: true, names: ["Store"] }],
    });
    // The line between the two groups stands for the crossing dependency.
    expect([...marks.lines.values()]).toEqual([["Front → Data"]]);
  });

  it("shows it on its box when the box is drawn, and on what holds it when not", () => {
    const groups = new Set([roleGroupId("entry_point"), roleGroupId("persistence")]);
    const closed = data([app, store], [edge("pkg:cargo/app", "pkg:cargo/store")]);
    const content = mapContent(closed, groups);
    const marks = intentMarks(drift, content, closed);
    expect(marks.items.get("pkg:cargo/app")).toEqual({ drawn: ["App"], within: ["Ui"], members: [] });
    expect(shownBy("component:dir/app/ui", drift, content, closed)).toBe("pkg:cargo/app");

    const open = data([app, ui, store], [edge("component:dir/app/ui", "pkg:cargo/store")]);
    const opened = mapContent(open, groups);
    const deeper = intentMarks(drift, opened, open);
    expect(deeper.items.get("component:dir/app/ui")).toEqual({ drawn: ["Ui"], within: [], members: [] });
    const line = opened.edges.find((e) => e.source === "component:dir/app/ui");
    expect(line && deeper.lines.get(line.id)).toEqual(["Front → Data"]);
  });

  it("marks nothing for what the code lacks, nor without a line to mark", () => {
    const view = data([app, store], []);
    const marks = intentMarks(drift, mapContent(view, new Set()), view);
    expect([...marks.items.values()].flatMap((m) => [...m.drawn, ...m.within])).not.toContain("Web");
    expect(marks.lines.size).toBe(0);
  });
});
