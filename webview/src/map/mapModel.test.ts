import { describe, expect, it } from "vitest";

import { NO_REVIEW, type Role, type ViewEdge, type ViewNode } from "../protocol";
import type { ViewData } from "../viewModel";
import { edgeKey } from "../viewModel";
import { layout, roleHeight } from "./layout";
import { mapContent, roleGroupId, toggleExpanded, toggleOf } from "./mapModel";

function node(id: string, role: Role | null, parent: string | null = null, children = 0): ViewNode {
  return {
    id,
    kind: parent ? "file" : "package",
    name: id.replace(/^.*[/:]/, ""),
    parent,
    role,
    language: null,
    children,
    file: null,
    span: null,
    review: null,
  };
}

function edge(src: string, dst: string, approximateOnly = true, weight = 1): ViewEdge {
  return {
    src,
    dst,
    kind: "calls",
    weight,
    edges: 1,
    best: approximateOnly ? "tree_sitter" : "lsp",
    approximateOnly,
    review: null,
  };
}

function view(nodes: ViewNode[], edges: ViewEdge[] = []): ViewData {
  return {
    subscription: 1,
    revision: 0,
    nodes: new Map(nodes.map((n) => [n.id, n])),
    edges: new Map(edges.map((e) => [edgeKey(e), e])),
    review: NO_REVIEW,
  };
}

/** Timonier's top, in short: two entry points, a data model, a store. */
const top = view(
  [
    node("pkg:cargo/cli", "entry_point", null, 2),
    node("pkg:cargo/daemon", "entry_point", null, 5),
    node("pkg:cargo/core", "data_model", null, 9),
    node("pkg:cargo/store", "persistence", null, 4),
  ],
  [
    edge("pkg:cargo/cli", "pkg:cargo/daemon", false),
    edge("pkg:cargo/daemon", "pkg:cargo/core", false, 3),
    edge("pkg:cargo/daemon", "pkg:cargo/store"),
    edge("pkg:cargo/store", "pkg:cargo/core"),
  ],
);

describe("map model", () => {
  it("opens on one box per role, in a stable order", () => {
    const content = mapContent(top, new Set());
    expect(content.items.map((i) => i.id)).toEqual([
      roleGroupId("entry_point"),
      roleGroupId("data_model"),
      roleGroupId("persistence"),
    ]);
    const entry = content.items[0];
    expect(entry?.kind === "role" && entry.members.map((m) => m.name)).toEqual(["cli", "daemon"]);
  });

  it("draws the edges between the groups, the edge inside a group gone", () => {
    const { edges } = mapContent(top, new Set());
    expect(edges.map((e) => [e.source, e.target, e.approximate])).toEqual([
      [roleGroupId("entry_point"), roleGroupId("data_model"), false],
      [roleGroupId("entry_point"), roleGroupId("persistence"), true],
      [roleGroupId("persistence"), roleGroupId("data_model"), true],
    ]);
  });

  it("shows the members of an open group, and their own edges", () => {
    const content = mapContent(top, new Set([roleGroupId("entry_point")]));
    const boxes = content.items.filter((i) => i.kind === "box");
    expect(boxes.map((b) => [b.id, b.parent])).toEqual([
      ["pkg:cargo/cli", roleGroupId("entry_point")],
      ["pkg:cargo/daemon", roleGroupId("entry_point")],
    ]);
    expect(content.edges.map((e) => [e.source, e.target])).toEqual([
      [roleGroupId("persistence"), roleGroupId("data_model")],
      ["pkg:cargo/cli", "pkg:cargo/daemon"],
      ["pkg:cargo/daemon", roleGroupId("data_model")],
      ["pkg:cargo/daemon", roleGroupId("persistence")],
    ]);
  });

  it("puts what the daemon expanded inside its box", () => {
    const expanded = view(
      [...top.nodes.values(), node("file:core/src/lib.rs", null, "pkg:cargo/core")],
      [...top.edges.values()],
    );
    const content = mapContent(expanded, new Set([roleGroupId("data_model")]));
    const core = content.items.find((i) => i.id === "pkg:cargo/core");
    expect(core?.kind === "box" && core.open).toBe(true);
    const file = content.items.find((i) => i.id === "file:core/src/lib.rs");
    expect(file?.kind === "box" && file.parent).toBe("pkg:cargo/core");
  });

  it("says what a click does, and folds what a box holds with it", () => {
    const content = mapContent(top, new Set([roleGroupId("entry_point")]));
    const group = content.items.find((i) => i.id === roleGroupId("entry_point"));
    const cli = content.items.find((i) => i.id === "pkg:cargo/cli");
    expect(group && toggleOf(group)).toEqual({ kind: "role", id: roleGroupId("entry_point") });
    expect(cli && toggleOf(cli)).toEqual({ kind: "expand", id: "pkg:cargo/cli" });
    const leaf = mapContent(view([node("pkg:cargo/empty", "utility")]), new Set([roleGroupId("utility")])).items.find(
      (i) => i.id === "pkg:cargo/empty",
    );
    expect(leaf && toggleOf(leaf)).toBeUndefined();
    const nested = view([
      node("pkg:cargo/core", "data_model", null, 1),
      node("component:x", null, "pkg:cargo/core", 1),
      node("file:x.rs", null, "component:x"),
    ]);
    expect(toggleExpanded([], "pkg:cargo/core", nested)).toEqual(["pkg:cargo/core"]);
    expect(toggleExpanded(["component:x", "pkg:cargo/core"], "pkg:cargo/core", nested)).toEqual([]);
  });
});

describe("map layout", () => {
  it("places every item, groups left to right along their edges", async () => {
    const content = mapContent(top, new Set());
    const placed = await layout(content);
    expect([...placed.keys()].sort()).toEqual(content.items.map((i) => i.id).sort());
    const x = (role: Role) => placed.get(roleGroupId(role))?.x ?? NaN;
    expect(x("entry_point")).toBeLessThan(x("persistence"));
    expect(x("persistence")).toBeLessThan(x("data_model"));
    expect(placed.get(roleGroupId("entry_point"))?.height).toBe(roleHeight(2));
  });

  it("sizes a group opened after a previous layout around its members, not at its old size", async () => {
    const closed = mapContent(top, new Set());
    const first = await layout(closed);
    const opened = mapContent(top, new Set([roleGroupId("entry_point")]));
    const placed = await layout(opened, { content: closed, placed: first });
    const group = placed.get(roleGroupId("entry_point"));
    expect(group).toBeTruthy();
    for (const member of ["pkg:cargo/cli", "pkg:cargo/daemon"]) {
      const at = placed.get(member);
      expect(at).toBeTruthy();
      expect(at!.x + at!.width).toBeLessThanOrEqual(group!.width);
      expect(at!.y + at!.height).toBeLessThanOrEqual(group!.height);
    }
  });

  it("sizes an open group around its members", async () => {
    const content = mapContent(top, new Set([roleGroupId("entry_point")]));
    const placed = await layout(content);
    const group = placed.get(roleGroupId("entry_point"));
    const cli = placed.get("pkg:cargo/cli");
    expect(group && cli).toBeTruthy();
    // Inside: positions are relative to the group.
    expect(cli!.x).toBeGreaterThanOrEqual(0);
    expect(cli!.x + cli!.width).toBeLessThanOrEqual(group!.width);
    expect(cli!.y).toBeGreaterThanOrEqual(40);
  });
});

describe("drawing budget (PLAN.md 4.10)", () => {
  /** Two packages of three files each, all open. */
  const wide = view([
    node("pkg:cargo/a", "utility", null, 3),
    node("pkg:cargo/b", "utility", null, 3),
    ...["1", "2", "3"].map((i) => node(`file:a/${i}.rs`, null, "pkg:cargo/a")),
    ...["1", "2", "3"].map((i) => node(`file:b/${i}.rs`, null, "pkg:cargo/b")),
  ]);
  const open = new Set([roleGroupId("utility")]);

  it("draws everything under the budget", () => {
    const content = mapContent(wide, open, { limit: 100 });
    expect(content.items).toHaveLength(9);
    expect(content.items.some((i) => i.kind === "box" && i.folded)).toBe(false);
  });

  it("over the budget, folds the deeper level whole, and says so", () => {
    const content = mapContent(wide, open, { limit: 5 });
    expect(content.items.map((i) => i.id)).toEqual([roleGroupId("utility"), "pkg:cargo/a", "pkg:cargo/b"]);
    const a = content.items[1];
    expect(a?.kind === "box" && [a.open, a.folded]).toEqual([false, true]);
    expect(a && toggleOf(a)).toEqual({ kind: "focus", id: "pkg:cargo/a" });
  });

  it("puts the focus's contents first, the others stay folded", () => {
    const content = mapContent(wide, open, { limit: 7, focus: "pkg:cargo/b" });
    const drawn = content.items.map((i) => i.id);
    expect(drawn).toContain("file:b/1.rs");
    expect(drawn).not.toContain("file:a/1.rs");
    const a = content.items.find((i) => i.id === "pkg:cargo/a");
    expect(a?.kind === "box" && a.folded).toBe(true);
  });

  it("draws the edges of folded contents from the folded box", () => {
    const linked = view([...wide.nodes.values()], [edge("file:a/1.rs", "file:b/2.rs")]);
    const { edges } = mapContent(linked, open, { limit: 5 });
    expect(edges.map((e) => [e.source, e.target])).toEqual([["pkg:cargo/a", "pkg:cargo/b"]]);
  });
});

describe("edge budget (PLAN.md 4.10)", () => {
  it("over the budget, draws the edges between the containers of their ends", () => {
    const files = view(
      [
        node("pkg:cargo/a", "utility", null, 2),
        node("pkg:cargo/b", "utility", null, 2),
        node("file:a/1.rs", null, "pkg:cargo/a"),
        node("file:a/2.rs", null, "pkg:cargo/a"),
        node("file:b/1.rs", null, "pkg:cargo/b"),
        node("file:b/2.rs", null, "pkg:cargo/b"),
      ],
      [edge("file:a/1.rs", "file:b/1.rs"), edge("file:a/2.rs", "file:b/2.rs", false)],
    );
    const open = new Set([roleGroupId("utility")]);
    expect(mapContent(files, open).edges).toHaveLength(2);
    const lifted = mapContent(files, open, { edgeLimit: 1 }).edges;
    expect(lifted.map((e) => [e.source, e.target, e.edges, e.approximate])).toEqual([
      ["pkg:cargo/a", "pkg:cargo/b", 2, false],
    ]);
  });
});
