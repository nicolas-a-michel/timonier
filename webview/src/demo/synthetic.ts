/**
 * A synthetic repository for the map's measurements (PLAN.md 4.10): packages of components of
 * files, as the daemon would send them with every package and component open, and calls between
 * files. Deterministic: the same arguments give the same view.
 */
import type { Role, ViewEdge, ViewNode, ViewSnapshot } from "../protocol";

const ROLES: Role[] = ["entry_point", "interface", "background_task", "data_model", "persistence", "utility"];

/** A small deterministic generator (mulberry32). */
function random(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function node(
  id: string,
  kind: ViewNode["kind"],
  name: string,
  parent: string | null,
  children: number,
  role: Role | null,
): ViewNode {
  return {
    id,
    kind,
    name,
    parent,
    role,
    language: kind === "file" ? "rust" : null,
    children,
    file: kind === "file" ? name : null,
    span: null,
  };
}

/** `packages` × `components` × `files` files, and `calls` calls per file on average. */
export function synthetic(packages: number, components: number, files: number, calls = 2): ViewSnapshot {
  const nodes: ViewNode[] = [];
  const all: string[] = [];
  for (let p = 0; p < packages; p++) {
    const pkg = `pkg:cargo/p${p}`;
    nodes.push(node(pkg, "package", `p${p}`, null, components, ROLES[p % ROLES.length] ?? null));
    for (let c = 0; c < components; c++) {
      const component = `component:p${p}/c${c}`;
      nodes.push(node(component, "component", `c${c}`, pkg, files, null));
      for (let f = 0; f < files; f++) {
        const file = `file:p${p}/c${c}/f${f}.rs`;
        nodes.push(node(file, "file", `p${p}/c${c}/f${f}.rs`, component, 4, null));
        all.push(file);
      }
    }
  }
  const next = random(packages * 10_000 + components * 100 + files);
  const edges = new Map<string, ViewEdge>();
  for (const src of all) {
    for (let i = 0; i < calls; i++) {
      // Mostly near: the same component or package, as real code.
      const index = all.indexOf(src);
      const spread = next() < 0.7 ? files : next() < 0.8 ? files * components : all.length;
      const dst = all[Math.min(all.length - 1, Math.max(0, index + Math.floor((next() - 0.5) * spread)))];
      if (!dst || dst === src) continue;
      const approximateOnly = next() < 0.6;
      edges.set(`${src}>${dst}`, {
        src,
        dst,
        kind: "calls",
        weight: 1 + Math.floor(next() * 5),
        edges: 1,
        best: approximateOnly ? "tree_sitter" : "lsp",
        approximateOnly,
      });
    }
  }
  return { subscription: 1, revision: 0, nodes, edges: [...edges.values()] };
}
