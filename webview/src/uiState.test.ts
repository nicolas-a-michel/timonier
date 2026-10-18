import { describe, expect, it } from "vitest";

import { roleGroupId } from "./map/mapModel";
import type { ViewNode } from "./protocol";
import { applyToggle, closeAll, initialUi, parseUi } from "./uiState";
import type { ViewData } from "./viewModel";

function node(id: string, parent: string | null, children: number): ViewNode {
  return { id, kind: "package", name: id, parent, role: null, language: null, children, file: null, span: null };
}

const view: ViewData = {
  subscription: 1,
  revision: 0,
  nodes: new Map([
    ["pkg:cargo/a", node("pkg:cargo/a", null, 1)],
    ["file:a.rs", node("file:a.rs", "pkg:cargo/a", 0)],
  ]),
  edges: new Map(),
};

describe("view state", () => {
  it("reads back what was saved, and nothing else", () => {
    expect(parseUi(undefined)).toEqual(initialUi);
    expect(parseUi({ openRoles: ["@role/utility"], expanded: ["pkg:cargo/a"] })).toEqual({
      openRoles: ["@role/utility"],
      expanded: ["pkg:cargo/a"],
    });
    expect(parseUi({ openRoles: [3], expanded: "x" })).toEqual(initialUi);
  });

  it("opens a role here, and asks the daemon to expand a box", () => {
    const role = applyToggle(initialUi, { kind: "role", id: roleGroupId("utility") }, view);
    expect(role).toEqual({ ui: { openRoles: [roleGroupId("utility")], expanded: [] } });
    expect(applyToggle(role.ui, { kind: "role", id: roleGroupId("utility") }, view).ui.openRoles).toEqual([]);
    const box = applyToggle(role.ui, { kind: "expand", id: "pkg:cargo/a" }, view);
    expect(box.subscribe).toEqual(["pkg:cargo/a"]);
    expect(applyToggle(box.ui, { kind: "expand", id: "pkg:cargo/a" }, view).subscribe).toEqual([]);
  });

  it("puts a folded box first, and forgets it on the way back", () => {
    const focused = applyToggle(initialUi, { kind: "focus", id: "pkg:cargo/a" }, view);
    expect(focused).toEqual({ ui: { ...initialUi, focus: "pkg:cargo/a" } });
    expect(parseUi(focused.ui).focus).toBe("pkg:cargo/a");
    expect(closeAll(focused.ui).ui.focus).toBeUndefined();
  });

  it("goes back to the opening, asking the daemon only when something was expanded", () => {
    expect(closeAll({ openRoles: ["@role/utility"], expanded: [] })).toEqual({ ui: initialUi, subscribe: undefined });
    expect(closeAll({ openRoles: [], expanded: ["pkg:cargo/a"] }).subscribe).toEqual([]);
  });
});
