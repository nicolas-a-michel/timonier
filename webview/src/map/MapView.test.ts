/**
 * The files the map says it draws (`displayed`, ADR 0014): they come first for refinement. On
 * Timonier's own views (test/views/timonier).
 */
import { readFileSync } from "node:fs";
import path from "node:path";

import { describe, expect, it } from "vitest";

import type { ViewSnapshot } from "../protocol";
import { edgeKey, type ViewData } from "../viewModel";
import { mapContent, roleGroupId } from "./mapModel";
import { displayedFiles } from "./MapView";

function view(name: string): ViewData {
  const file = path.join(import.meta.dirname, "..", "..", "test", "views", "timonier", `${name}.json`);
  const snapshot = (JSON.parse(readFileSync(file, "utf8")) as { snapshot: ViewSnapshot }).snapshot;
  return {
    subscription: 1,
    revision: 0,
    nodes: new Map(snapshot.nodes.map((n) => [n.id, n])),
    edges: new Map(snapshot.edges.map((e) => [edgeKey(e), e])),
  };
}

describe("the files the map draws", () => {
  it("are none at the opening, where only roles and packages show", () => {
    expect(displayedFiles(mapContent(view("top"), new Set()))).toEqual([]);
  });

  it("are those of the file and symbol boxes drawn, each once, sorted", () => {
    const open = view("open");
    const roles = new Set([...open.nodes.values()].filter((n) => n.parent === null).map((n) => roleGroupId(n.role)));
    const content = mapContent(open, roles);
    const files = displayedFiles(content);
    const drawn = content.items.flatMap((item) => (item.kind === "box" && item.node.file ? [item.node.file] : []));
    expect(files.length).toBeGreaterThan(0);
    expect(files).toEqual([...new Set(drawn)].sort());
  });
});
