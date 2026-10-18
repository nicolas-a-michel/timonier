/**
 * A daemon for the demonstration page (ADR 0020): it answers `subscribe` from a snapshot of a
 * real repository taken with every package and component open (test/views, written by the
 * `view_snapshot` example), as the daemon would for the boxes expanded.
 */
import type { ViewEdge, ViewNode, ViewSnapshot } from "../protocol";

export interface Fixture {
  snapshot: ViewSnapshot;
}

/** The view with `expanded` open, from a snapshot that holds at least that much. */
export function viewOf(open: ViewSnapshot, expanded: readonly string[], revision: number): ViewSnapshot {
  const byId = new Map(open.nodes.map((node) => [node.id, node]));
  const shown = new Set<string>();
  const visit = (node: ViewNode) => {
    shown.add(node.id);
    if (!expanded.includes(node.id)) return;
    for (const child of open.nodes) if (child.parent === node.id) visit(child);
  };
  for (const node of open.nodes) if (node.parent === null) visit(node);
  // Each end of an edge goes up to its nearest shown ancestor, as the daemon's aggregation does.
  const up = (id: string): string | undefined => {
    for (let at = byId.get(id); at; at = at.parent ? byId.get(at.parent) : undefined) {
      if (shown.has(at.id)) return at.id;
    }
    return undefined;
  };
  const merged = new Map<string, ViewEdge>();
  for (const edge of open.edges) {
    const src = up(edge.src);
    const dst = up(edge.dst);
    if (!src || !dst || src === dst) continue;
    const key = `${src}\u0000${dst}\u0000${edge.kind}`;
    const known = merged.get(key);
    if (known) {
      known.weight += edge.weight;
      known.edges += edge.edges;
      known.approximateOnly &&= edge.approximateOnly;
    } else {
      merged.set(key, { ...edge, src, dst });
    }
  }
  return {
    subscription: 1,
    revision,
    nodes: open.nodes.filter((node) => shown.has(node.id)),
    edges: [...merged.values()],
  };
}
