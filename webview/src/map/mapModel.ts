/**
 * What the map shows (PLAN.md 4.7): the daemon's view, grouped by role at the top. Pure: the
 * layout and React Flow come after.
 *
 * Semantic zoom: the opening shows one box per role; a role opens on its components; a component
 * opens on what it holds (the daemon expands it), down to the symbols.
 */
import type { EdgeReview, Role, ViewEdge, ViewNode } from "../protocol";
import type { ViewData } from "../viewModel";

/** A role group's id on the map: no view node can have it (ids have a `<prefix>:`). */
export function roleGroupId(role: Role | null): string {
  return `@role/${role ?? "none"}`;
}

export interface MapGroup {
  kind: "role";
  id: string;
  role: Role | null;
  /** Its members, sorted by name. */
  members: ViewNode[];
  open: boolean;
  /** Its members changed since the last review (ADR 0024). */
  changes: number;
}

export interface MapBox {
  kind: "box";
  id: string;
  node: ViewNode;
  /** The group or box it is drawn in. */
  parent: string;
  /** Its children are drawn inside it. */
  open: boolean;
  /**
   * The daemon sent its children, but the map is over its budget and draws them as this box
   * (PLAN.md 4.10). A click puts them first.
   */
  folded: boolean;
}

export type MapItem = MapGroup | MapBox;

/** One line on the map between two drawn items, standing for the view's edges between them. */
export interface MapEdge {
  id: string;
  source: string;
  target: string;
  /** Every edge it stands for comes from tree-sitter alone. */
  approximate: boolean;
  weight: number;
  edges: number;
  kinds: ViewEdge["kind"][];
  /**
   * New since the last review when every edge it stands for is (ADR 0024); in a cycle added
   * when one of them is.
   */
  review: EdgeReview | null;
}

export interface MapContent {
  items: MapItem[];
  edges: MapEdge[];
}

/** The order of the role groups: the entry points first, the external libraries last. */
const ROLE_ORDER: (Role | null)[] = [
  "entry_point",
  "interface",
  "background_task",
  "data_model",
  "persistence",
  "utility",
  "external_libraries",
  null,
];

/** Beyond this many items, the map draws the deeper levels folded (PLAN.md 4.10). */
export const MAX_DRAWN = 500;

/** Beyond this many edges, those between the deepest items are drawn between their containers. */
export const MAX_EDGES = 600;

export interface MapOptions {
  /** The budget of drawn items. */
  limit?: number;
  /** The budget of drawn edges. */
  edgeLimit?: number;
  /** A box whose contents come first within the budget: the last folded box clicked. */
  focus?: string;
}

/** The map of `view`, with the role groups of `openRoles` open. */
export function mapContent(view: ViewData, openRoles: ReadonlySet<string>, options: MapOptions = {}): MapContent {
  const limit = options.limit ?? MAX_DRAWN;
  const nodes = [...view.nodes.values()];
  const top = nodes.filter((node) => node.parent === null);
  const groups = new Map<string, MapGroup>();
  for (const node of top) {
    const id = roleGroupId(node.role);
    const group = groups.get(id) ?? {
      kind: "role",
      id,
      role: node.role,
      members: [],
      open: openRoles.has(id),
      changes: 0,
    };
    group.members.push(node);
    if (node.review) group.changes++;
    groups.set(id, group);
  }
  const ordered = [...groups.values()].sort((a, b) => ROLE_ORDER.indexOf(a.role) - ROLE_ORDER.indexOf(b.role));
  for (const group of ordered) group.members.sort((a, b) => a.name.localeCompare(b.name));

  // Each view node, drawn where its nearest shown ancestor is.
  const drawnAs = new Map<string, string>();
  const items: MapItem[] = [...ordered];
  const children = new Map<string, ViewNode[]>();
  for (const node of nodes) {
    if (node.parent === null) continue;
    const list = children.get(node.parent) ?? [];
    list.push(node);
    children.set(node.parent, list);
  }
  for (const list of children.values()) list.sort((a, b) => a.name.localeCompare(b.name));
  const drawn = budget(ordered, children, limit, options.focus);
  const place = (node: ViewNode, parent: string) => {
    const inside = children.get(node.id) ?? [];
    const open = inside.length > 0 && drawn.has(inside[0]?.id ?? "");
    items.push({ kind: "box", id: node.id, node, parent, open, folded: inside.length > 0 && !open });
    drawnAs.set(node.id, node.id);
    if (open) {
      for (const child of inside) place(child, node.id);
    } else {
      // Folded: its contents are drawn as it.
      const fold = (inner: ViewNode) => {
        drawnAs.set(inner.id, node.id);
        for (const child of children.get(inner.id) ?? []) fold(child);
      };
      for (const child of inside) fold(child);
    }
  };
  for (const group of ordered) {
    for (const member of group.members) {
      if (group.open) {
        place(member, group.id);
      } else {
        // A closed group stands for all that it holds.
        const hide = (node: ViewNode) => {
          drawnAs.set(node.id, group.id);
          for (const child of children.get(node.id) ?? []) hide(child);
        };
        hide(member);
      }
    }
  }

  const lines = (ends: (id: string) => string | undefined): Map<string, MapEdge> => {
    const merged = new Map<string, MapEdge>();
    const reviews = new Map<string, EdgeReview>();
    for (const edge of view.edges.values()) {
      const source = ends(edge.src);
      const target = ends(edge.dst);
      if (!source || !target || source === target) continue;
      const id = `${source}→${target}`;
      const known = merged.get(id);
      const line = known ?? { id, source, target, approximate: true, weight: 0, edges: 0, kinds: [], review: null };
      line.approximate &&= edge.approximateOnly;
      line.weight += edge.weight;
      line.edges += edge.edges;
      if (!line.kinds.includes(edge.kind)) line.kinds.push(edge.kind);
      const seen = reviews.get(id);
      const added = edge.review?.added ?? false;
      const cycle = edge.review?.cycle ?? false;
      reviews.set(id, seen ? { added: seen.added && added, cycle: seen.cycle || cycle } : { added, cycle });
      merged.set(id, line);
    }
    for (const [id, review] of reviews) {
      const line = merged.get(id);
      if (line) line.review = review.added || review.cycle ? review : null;
    }
    return merged;
  };
  let merged = lines((id) => drawnAs.get(id));
  // Over the edge budget: the ends at the deepest level go up to their container, until it fits.
  const edgeLimit = options.edgeLimit ?? MAX_EDGES;
  const parentItem = new Map(items.map((item) => [item.id, item.kind === "box" ? item.parent : undefined]));
  const depth = (id: string): number => {
    let d = 0;
    for (let at = parentItem.get(id); at; at = parentItem.get(at)) d++;
    return d;
  };
  let cut = Math.max(0, ...items.map((item) => depth(item.id)));
  while (merged.size > edgeLimit && cut > 0) {
    cut--;
    const lift = (id: string): string => {
      let at = id;
      while (depth(at) > cut) at = parentItem.get(at) ?? at;
      return at;
    };
    merged = lines((id) => {
      const drawn = drawnAs.get(id);
      return drawn && lift(drawn);
    });
  }
  const edges = [...merged.values()].sort((a, b) => a.id.localeCompare(b.id));
  return { items, edges };
}

/**
 * The boxes to draw within `limit`: whole levels, from the top, while they fit; then the
 * contents of `focus` and of its ancestors, first. The children of a box are drawn all or none.
 */
function budget(groups: MapGroup[], children: Map<string, ViewNode[]>, limit: number, focus?: string): Set<string> {
  const drawn = new Set<string>();
  let count = groups.length;
  // Level by level, breadth first: the boxes whose children may be drawn next.
  let level: ViewNode[] = groups.filter((g) => g.open).flatMap((g) => g.members);
  if (count + level.length > limit) return drawn;
  for (const node of level) drawn.add(node.id);
  count += level.length;
  const parents = new Map<string, string>();
  for (const [parent, list] of children) for (const child of list) parents.set(child.id, parent);
  // The focus first: the chain from its top member down to its contents.
  if (focus) {
    const chain: string[] = [];
    for (let at: string | undefined = focus; at; at = parents.get(at)) chain.unshift(at);
    for (const id of chain) {
      if (!drawn.has(id)) break;
      const inside = children.get(id) ?? [];
      if (inside.every((child) => drawn.has(child.id))) continue;
      if (count + inside.length > limit) break;
      for (const child of inside) drawn.add(child.id);
      count += inside.length;
    }
  }
  while (level.length > 0) {
    const next: ViewNode[] = [];
    for (const node of level) next.push(...(children.get(node.id) ?? []));
    const fresh = next.filter((child) => !drawn.has(child.id));
    if (fresh.length === 0) {
      level = next;
      continue;
    }
    if (count + fresh.length > limit) break;
    for (const child of fresh) drawn.add(child.id);
    count += fresh.length;
    level = next;
  }
  return drawn;
}

/** Whether something about `item` changed since the last review (ADR 0024). */
export function changed(item: MapItem): boolean {
  return item.kind === "role" ? item.changes > 0 : item.node.review !== null;
}

/** What a click on `item` asks: open or close a role group, expand or fold a box. */
export type Toggle = { kind: "role"; id: string } | { kind: "expand"; id: string } | { kind: "focus"; id: string };

export function toggleOf(item: MapItem): Toggle | undefined {
  if (item.kind === "role") return { kind: "role", id: item.id };
  // Folded for the budget: its contents come first, the map folds others instead.
  if (item.folded) return { kind: "focus", id: item.id };
  // Nothing inside: nothing to open.
  return item.node.children > 0 ? { kind: "expand", id: item.id } : undefined;
}

/** `expanded` after folding or unfolding `id`: folding a box folds what it holds too. */
export function toggleExpanded(expanded: readonly string[], id: string, view: ViewData): string[] {
  if (!expanded.includes(id)) return [...expanded, id].sort();
  const folded = new Set([id]);
  let grew = true;
  while (grew) {
    grew = false;
    for (const node of view.nodes.values()) {
      if (node.parent !== null && folded.has(node.parent) && !folded.has(node.id)) {
        folded.add(node.id);
        grew = true;
      }
    }
  }
  return expanded.filter((e) => !folded.has(e));
}
