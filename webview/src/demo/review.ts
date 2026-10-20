/**
 * A review diff on Timonier's map, for the demonstration page (`?map=timonier&review=1`), its
 * tests and the five-second test: what the daemon would send some time after a review
 * (ADR 0024). timonier-mcp is new, with a new link from timonier-cli; timonier-core now calls
 * timonier-store, which closes a cycle; timonier-daemon and a component of timonier-index were
 * modified; timonier-core's fan-in jumped; a crate and a link went away.
 */
import type { EdgeReview, NodeReview, ReviewBox, ReviewSummary, ViewEdge, ViewNode, ViewSnapshot } from "../protocol";

const CLI = "pkg:cargo/timonier-cli";
const CORE = "pkg:cargo/timonier-core";
const DAEMON = "pkg:cargo/timonier-daemon";
const INDEX = "pkg:cargo/timonier-index";
const MCP = "pkg:cargo/timonier-mcp";
const STORE = "pkg:cargo/timonier-store";
const EXTRACT = "component:cluster/crates/timonier-index/src/extract.rs";

const QUIET: NodeReview = { change: null, fanIn: null, fanOut: null, cycle: false };

const NODES: Record<string, NodeReview> = {
  [MCP]: { ...QUIET, change: "added" },
  [DAEMON]: { ...QUIET, change: "modified" },
  [INDEX]: { ...QUIET, change: "modified" },
  [EXTRACT]: { ...QUIET, change: "modified" },
  [CORE]: { ...QUIET, change: "modified", fanIn: { before: 4, after: 7 }, cycle: true },
  [STORE]: { ...QUIET, cycle: true },
};

const EDGES: Record<string, EdgeReview> = {
  [`${CLI}>${MCP}`]: { added: true, cycle: false },
  [`${CORE}>${STORE}`]: { added: true, cycle: true },
  [`${STORE}>${CORE}`]: { added: false, cycle: true },
};

function named(nodes: ViewNode[], id: string, fallback?: string): ReviewBox {
  const node = nodes.find((n) => n.id === id);
  const parent = node?.parent ? nodes.find((n) => n.id === node.parent) : undefined;
  return {
    id,
    kind: node?.kind ?? "package",
    name: node?.name ?? fallback ?? id,
    role: node?.role ?? null,
    within: parent && (parent.kind === "package" || parent.kind === "component") ? parent.name : null,
  };
}

/** `open` some time after a review marked at `since`. */
export function withReview(open: ViewSnapshot, since: number): ViewSnapshot {
  const closing: ViewEdge = {
    src: CORE,
    dst: STORE,
    kind: "calls",
    weight: 1,
    edges: 1,
    best: "lsp",
    approximateOnly: false,
    review: null,
  };
  const nodes = open.nodes.map((node) => ({ ...node, review: NODES[node.id] ?? null }));
  const edges = [...open.edges, closing].map((edge) => ({ ...edge, review: EDGES[`${edge.src}>${edge.dst}`] ?? null }));
  const box = (id: string, fallback?: string) => named(nodes, id, fallback);
  const review: ReviewSummary = {
    since: { seq: 41, timestampMs: since },
    added: [box(MCP)],
    modified: [box(INDEX), box(DAEMON), box(CORE), box(EXTRACT)].sort((a, b) => a.name.localeCompare(b.name)),
    removed: [box("pkg:cargo/timonier-legacy", "timonier-legacy")],
    dependenciesAdded: [
      { src: box(CLI), dst: box(MCP) },
      { src: box(CORE), dst: box(STORE) },
    ],
    dependenciesRemoved: [{ src: box(CLI), dst: box("pkg:cargo/timonier-legacy", "timonier-legacy") }],
    cycles: [[box(CORE), box(STORE)]],
    fans: [{ item: box(CORE), fanIn: { before: 4, after: 7 }, fanOut: null }],
  };
  return { ...open, nodes, edges, review };
}

/** `open` just marked as reviewed, at `since`: nothing changed since. */
export function justReviewed(open: ViewSnapshot, since: number): ViewSnapshot {
  return {
    ...open,
    nodes: open.nodes.map((node) => ({ ...node, review: null })),
    edges: open.edges.map((edge) => ({ ...edge, review: null })),
    review: {
      since: { seq: 42, timestampMs: since },
      added: [],
      modified: [],
      removed: [],
      dependenciesAdded: [],
      dependenciesRemoved: [],
      cycles: [],
      fans: [],
    },
  };
}
