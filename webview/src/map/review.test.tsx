/**
 * The review diff on the map (ADR 0024): what each box says, what the lines say, and the
 * summary, before and after the first review.
 */
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import {
  NO_REVIEW,
  type NodeReview,
  type ReviewBox,
  type ReviewSummary,
  type ViewEdge,
  type ViewNode,
} from "../protocol";
import { edgeKey, type ViewData } from "../viewModel";
import { changed, mapContent, roleGroupId } from "./mapModel";
import { primaryChange, reviewLines, ReviewSummaryPanel, summaryCounts, summarySections } from "./review";

const quiet: NodeReview = { change: null, fanIn: null, fanOut: null, cycle: false };

function node(id: string, review: NodeReview | null = null, parent: string | null = null): ViewNode {
  return {
    id,
    kind: parent ? "file" : "package",
    name: id.replace(/^.*[/:]/, ""),
    parent,
    role: "utility",
    language: null,
    children: 0,
    file: null,
    span: null,
    review,
    debt: null,
  };
}

function edge(src: string, dst: string, review: ViewEdge["review"] = null): ViewEdge {
  return { src, dst, kind: "calls", weight: 1, edges: 1, best: "lsp", approximateOnly: false, review };
}

function view(nodes: ViewNode[], edges: ViewEdge[], review: ReviewSummary = NO_REVIEW): ViewData {
  return {
    subscription: 1,
    revision: 0,
    nodes: new Map(nodes.map((n) => [n.id, n])),
    edges: new Map(edges.map((e) => [edgeKey(e), e])),
    review,
  };
}

function box(name: string): ReviewBox {
  return { id: `pkg:cargo/${name}`, kind: "package", name, role: null, within: null };
}

describe("what a box says of the review", () => {
  it("shows the most telling change, and says them all", () => {
    expect(primaryChange({ ...quiet, change: "added", cycle: true })).toBe("added");
    expect(primaryChange({ ...quiet, change: "modified", cycle: true })).toBe("cycle");
    expect(primaryChange({ ...quiet, change: "modified", fanIn: { before: 1, after: 4 } })).toBe("fanIn");
    expect(primaryChange({ ...quiet, change: "modified" })).toBe("modified");
    expect(primaryChange(quiet)).toBeNull();
    const lines = reviewLines({ change: "modified", fanIn: { before: 2, after: 5 }, fanOut: null, cycle: true }, "en");
    expect(lines).toEqual([
      "Modified since the last review",
      "In a cycle added since the last review",
      "5 boxes depend on it, against 2 at the review",
    ]);
    expect(reviewLines(null, "fr")).toEqual([]);
  });
});

describe("the map's lines and role groups", () => {
  it("count the changed members of a role, and mark a line new only when all it holds is", () => {
    const added = { added: true, cycle: false };
    // Two closed roles: both edges are one line between them, and only one of them is new.
    const content = mapContent(
      view(
        [
          node("pkg:cargo/a", { ...quiet, change: "modified" }),
          { ...node("pkg:cargo/b"), role: "persistence" },
          node("file:a/one.rs", null, "pkg:cargo/a"),
          node("file:a/two.rs", null, "pkg:cargo/a"),
        ],
        [edge("file:a/one.rs", "pkg:cargo/b", added), edge("file:a/two.rs", "pkg:cargo/b")],
      ),
      new Set(),
    );
    const utility = content.items.find((item) => item.id === roleGroupId("utility"));
    expect(utility?.kind === "role" && utility.changes).toBe(1);
    expect(utility && changed(utility)).toBe(true);
    const persistence = content.items.find((item) => item.id === roleGroupId("persistence"));
    expect(persistence && changed(persistence)).toBe(false);
    expect(content.edges.map((e) => [e.source, e.target, e.review])).toEqual([
      [roleGroupId("utility"), roleGroupId("persistence"), null],
    ]);
    // Open: each file has its line, and the new one says so.
    const open = mapContent(
      view(
        [node("pkg:cargo/a"), node("pkg:cargo/b"), node("file:a/one.rs", null, "pkg:cargo/a")],
        [edge("file:a/one.rs", "pkg:cargo/b", added)],
      ),
      new Set([roleGroupId("utility")]),
    );
    expect(open.edges.map((e) => e.review)).toEqual([added]);
    const unchanged = open.items.find((item) => item.id === "pkg:cargo/b");
    expect(unchanged && changed(unchanged)).toBe(false);
    const cycle = mapContent(
      view(
        [node("pkg:cargo/a"), node("pkg:cargo/b")],
        [edge("pkg:cargo/a", "pkg:cargo/b", { added: false, cycle: true })],
      ),
      new Set([roleGroupId("utility")]),
    );
    expect(cycle.edges[0]?.review).toEqual({ added: false, cycle: true });
  });
});

describe("the summary of the review", () => {
  const since = { seq: 4, timestampMs: Date.UTC(2026, 9, 18, 12, 30) };
  const summary: ReviewSummary = {
    ...NO_REVIEW,
    since,
    added: [box("d")],
    modified: [box("a"), box("b")],
    removed: [box("gone")],
    dependenciesAdded: [{ src: box("a"), dst: box("b") }],
    cycles: [[box("a"), box("b")]],
    fans: [{ item: box("b"), fanIn: { before: 1, after: 4 }, fanOut: null }],
  };

  it("counts what changed, and lists it by heading", () => {
    expect(summaryCounts(summary, "en")).toEqual([
      "1 new box",
      "2 modified",
      "1 removed",
      "1 dependency added",
      "1 cycle added",
      "1 jump of fan-in or fan-out",
    ]);
    expect(summaryCounts({ ...NO_REVIEW, since }, "fr")).toEqual([]);
    const sections = summarySections(summary, "fr");
    expect(sections.map((s) => s.heading)).toEqual([
      "Nouvelles",
      "Modifiées",
      "Supprimées",
      "Dépendances ajoutées",
      "Cycles ajoutés",
      "Fan-in et fan-out qui ont bondi",
    ]);
    expect(sections[3]?.items).toEqual(["a → b"]);
    expect(sections[5]?.items).toEqual(["b · Fan-in 1 → 4"]);
  });

  it("says what to do before the first review, and that nothing changed after it", () => {
    const before = renderToStaticMarkup(
      <ReviewSummaryPanel summary={NO_REVIEW} locale="en" onMarkReviewed={() => {}} />,
    );
    expect(before).toContain("No review yet");
    expect(before).toContain(">Mark as reviewed</button>");
    const after = renderToStaticMarkup(<ReviewSummaryPanel summary={{ ...NO_REVIEW, since }} locale="fr" />);
    expect(after).toContain("Depuis votre revue du");
    expect(after).toContain("Rien n&#x27;a changé depuis.");
    expect(after).not.toContain("<button");
    const changes = renderToStaticMarkup(<ReviewSummaryPanel summary={summary} locale="en" />);
    expect(changes).toContain("1 new box · 2 modified");
    // The lists on demand: a disclosure.
    expect(changes).toContain('aria-expanded="false"');
  });
});
