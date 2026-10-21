/**
 * What the webview keeps of its view, and what it does with each host message (ADR 0019).
 * Pure: `main.tsx` feeds it the messages and posts the effects back to the host.
 */
import type { DaemonState, EdgeRef, HostMessage, ReviewSummary, ViewEdge, ViewNode, WebviewMessage } from "./protocol";

export interface ViewData {
  subscription: number;
  revision: number;
  nodes: Map<string, ViewNode>;
  edges: Map<string, ViewEdge>;
  /** What changed since the last review, whatever the zoom (ADR 0024). */
  review: ReviewSummary;
}

export interface ViewModel {
  daemon: { state: DaemonState; detail: string | null };
  view: ViewData | undefined;
  /** A resync was asked and its snapshot has not arrived yet. */
  resyncing: boolean;
}

export const initialModel: ViewModel = {
  daemon: { state: "starting", detail: null },
  view: undefined,
  resyncing: false,
};

export function edgeKey(edge: EdgeRef): string {
  return `${edge.src}\u0000${edge.dst}\u0000${edge.kind}`;
}

export interface Reduced {
  model: ViewModel;
  /** Messages for the host. */
  effects: WebviewMessage[];
}

function rendered(view: ViewData): WebviewMessage {
  return { type: "rendered", subscription: view.subscription, revision: view.revision, nodes: view.nodes.size };
}

export function reduce(model: ViewModel, message: HostMessage): Reduced {
  switch (message.type) {
    // For the map itself (main.tsx), not for the view's data; the tracking is for its panels
    // (ADR 0031), the whiteboard's commands for its page (ADR 0037), the performances for the map's
    // layer and their panel (ADR 0051).
    case "command":
    case "pins":
    case "density":
    case "tracking":
    case "whiteboard":
    case "intent":
    case "intentSaved":
    case "drift":
    case "perfMap":
    case "perfRuns":
    case "perfRun":
    case "budgets":
    case "perfRegressions":
      return { model, effects: [] };
    case "daemon":
      return { model: { ...model, daemon: { state: message.state, detail: message.detail } }, effects: [] };
    case "snapshot": {
      const { snapshot } = message;
      const view: ViewData = {
        subscription: snapshot.subscription,
        revision: snapshot.revision,
        nodes: new Map(snapshot.nodes.map((node) => [node.id, node])),
        edges: new Map(snapshot.edges.map((edge) => [edgeKey(edge), edge])),
        review: snapshot.review,
      };
      return { model: { ...model, view, resyncing: false }, effects: [rendered(view)] };
    }
    case "delta": {
      const { delta } = message;
      const view = model.view;
      // A delta of another subscription, or before any snapshot: not ours to apply.
      if (!view || delta.subscription !== view.subscription) return { model, effects: [] };
      // Sent before the snapshot the webview holds.
      if (delta.base < view.revision) return { model, effects: [] };
      if (delta.base > view.revision) {
        // A delta went missing: ask once for the whole view.
        if (model.resyncing) return { model, effects: [] };
        return { model: { ...model, resyncing: true }, effects: [{ type: "resync" }] };
      }
      const nodes = new Map(view.nodes);
      const edges = new Map(view.edges);
      for (const id of delta.removedNodes) nodes.delete(id);
      for (const edge of delta.removedEdges) edges.delete(edgeKey(edge));
      for (const node of delta.nodes) nodes.set(node.id, node);
      for (const edge of delta.edges) edges.set(edgeKey(edge), edge);
      const next: ViewData = {
        subscription: view.subscription,
        revision: delta.revision,
        nodes,
        edges,
        review: delta.review ?? view.review,
      };
      return { model: { ...model, view: next }, effects: [rendered(next)] };
    }
  }
}
