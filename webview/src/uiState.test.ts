import { describe, expect, it } from "vitest";

import { roleGroupId } from "./map/mapModel";
import { NO_REVIEW, type ViewNode } from "./protocol";
import { applyToggle, closeAll, initialUi, parseUi, reveal, toggleChanges, toggleDebt } from "./uiState";
import type { ViewData } from "./viewModel";

function node(id: string, parent: string | null, children: number): ViewNode {
  return {
    id,
    kind: "package",
    name: id,
    parent,
    role: null,
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

const view: ViewData = {
  subscription: 1,
  revision: 0,
  nodes: new Map([
    ["pkg:cargo/a", node("pkg:cargo/a", null, 1)],
    ["file:a.rs", node("file:a.rs", "pkg:cargo/a", 0)],
  ]),
  edges: new Map(),
  review: NO_REVIEW,
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

  it("turns the filter of the changes on and off, keeps it, and reads it back (ADR 0024)", () => {
    const on = toggleChanges(initialUi).ui;
    expect(on.onlyChanges).toBe(true);
    expect(toggleChanges(on).ui).toEqual(initialUi);
    expect(closeAll({ ...on, openRoles: ["@role/utility"] }).ui).toEqual(on);
    expect(parseUi({ openRoles: [], expanded: [], onlyChanges: true }).onlyChanges).toBe(true);
    expect(parseUi({ openRoles: [], expanded: [], onlyChanges: "yes" }).onlyChanges).toBeUndefined();
  });

  it("goes back to the opening, asking the daemon only when something was expanded", () => {
    expect(closeAll({ openRoles: ["@role/utility"], expanded: [] })).toEqual({ ui: initialUi, subscribe: undefined });
    expect(closeAll({ openRoles: [], expanded: ["pkg:cargo/a"] }).subscribe).toEqual([]);
  });

  it("keeps the debt layer across reloads and back to the roles (ADR 0030)", () => {
    const on = toggleDebt(initialUi).ui;
    expect(on.debt).toBe(true);
    expect(parseUi(JSON.parse(JSON.stringify(on))).debt).toBe(true);
    expect(parseUi({ openRoles: [], expanded: [], debt: "yes" }).debt).toBeUndefined();
    expect(closeAll(on).ui.debt).toBe(true);
    expect(toggleDebt(on).ui.debt).toBeUndefined();
  });
});

describe("revealing a box (ADR 0032, Q46)", () => {
  it("opens the role of the top of its path and expands the path, and closes nothing", () => {
    const ui = { ...initialUi, openRoles: ["@role/utility"], expanded: ["pkg:other"] };
    const shown = reveal(ui, "file:a.rs", ["pkg:cargo/a"], view);
    expect(shown.ui.openRoles).toEqual([roleGroupId(null), "@role/utility"].sort());
    expect(shown.ui.expanded).toEqual(["pkg:cargo/a", "pkg:other"]);
    expect(shown.subscribe).toEqual(["pkg:cargo/a", "pkg:other"]);
  });

  it("asks the daemon for nothing when the path is open already", () => {
    const ui = { ...initialUi, openRoles: [roleGroupId(null)], expanded: ["pkg:cargo/a"] };
    expect(reveal(ui, "file:a.rs", ["pkg:cargo/a"], view)).toEqual({ ui });
    // A box at the top: its own role.
    expect(reveal(initialUi, "pkg:cargo/a", [], view).ui.openRoles).toEqual([roleGroupId(null)]);
  });
});
