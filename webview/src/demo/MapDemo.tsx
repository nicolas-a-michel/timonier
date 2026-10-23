import { useEffect, useState } from "react";

import { App } from "../App";
import type { Locale } from "../i18n";
import type { Density, ViewSnapshot } from "../protocol";
import type { BudgetsView, DriftView, PerfMap } from "../protocol";
import {
  applyToggle,
  closeAll,
  initialUi,
  toggleChanges,
  toggleCertainty,
  toggleDebt,
  toggleIntent,
  togglePerformance,
  type Applied,
  type UiState,
} from "../uiState";
import { initialModel, reduce, type ViewModel } from "../viewModel";
import type { Pins } from "../map/layout";
import { roleGroupId } from "../map/mapModel";
import { viewOf } from "./fakeDaemon";
import { justReviewed, withReview } from "./review";

export interface MapDemoProps {
  locale: Locale;
  /** A snapshot with every package and component open (test/views). */
  open: ViewSnapshot;
  /** Every role and every container open from the start: for the measurements. */
  openAll?: boolean;
  /** A review diff on the map, some time after a review (ADR 0024). */
  review?: boolean;
  /** The filter « highlight the changes » on from the start, as a page reloaded keeps it. */
  only?: boolean;
  /** The layer of the review debt on from the start (ADR 0030). */
  debt?: boolean;
  /** The certainty layer on from the start (PLAN.md 11.5). */
  certainty?: boolean;
  /** Compared with a whiteboard drawn for Timonier (ADR 0042). */
  intent?: boolean;
  /** The Performances layer, on a run measured on Timonier (ADR 0051). */
  performance?: boolean;
  /** A budget of the whiteboard the last runs exceed, on the daemon (ADR 0052). */
  budget?: boolean;
  density?: Density;
  onRendered?: () => void;
}

/**
 * A whiteboard drawn for Timonier, as the daemon compares it (ADR 0042): the daemon in a « Core »
 * layer that may not reach the store, a CLI that must reach the daemon, and a web front the code
 * lacks.
 */
const DRAWN: DriftView = {
  drawn: true,
  problem: null,
  names: {
    "L-1": "Core",
    "L-2": "Storage",
    "C-1": "Daemon",
    "C-2": "Store",
    "C-3": "Web",
    "C-4": "Cli",
    "C-5": "Components",
    "C-6": "Model",
  },
  ancestors: {
    "pkg:cargo/timonier-daemon": [],
    "pkg:cargo/timonier-store": [],
    "pkg:cargo/timonier-cli": [],
    "pkg:cargo/timonier-core": [],
    "component:cluster/crates/timonier-index/src/cluster.rs": ["pkg:cargo/timonier-index"],
  },
  drift: {
    anchors: [
      { component: "C-1", name: "Daemon", actual: "pkg:cargo/timonier-daemon", candidates: 1 },
      { component: "C-2", name: "Store", actual: "pkg:cargo/timonier-store", candidates: 1 },
      { component: "C-3", name: "Web", actual: null, candidates: 0 },
      { component: "C-4", name: "Cli", actual: "pkg:cargo/timonier-cli", candidates: 1 },
      // Deeper than the map draws it: the box that holds it says so.
      {
        component: "C-5",
        name: "Components",
        actual: "component:cluster/crates/timonier-index/src/cluster.rs",
        candidates: 1,
      },
      { component: "C-6", name: "Model", actual: "pkg:cargo/timonier-core", candidates: 1 },
    ],
    missing: ["C-3"],
    forbidden: [
      {
        edge: "E-1",
        src: "L-1",
        dst: "L-2",
        dependencies: [
          { from: "pkg:cargo/timonier-daemon", to: "pkg:cargo/timonier-store", kinds: ["calls"], weight: 12 },
          // New since the review, in the demonstration's review: both marks on its line.
          { from: "pkg:cargo/timonier-core", to: "pkg:cargo/timonier-store", kinds: ["calls"], weight: 1 },
        ],
      },
    ],
    required: [{ edge: "E-2", src: "C-2", dst: "C-1", dependencies: [] }],
    unchecked: [],
  },
};

/** When the demonstration's review was marked: yesterday, at a fixed time. */
const REVIEWED_AT = Date.UTC(2026, 9, 17, 16, 45);

function everythingOpen(open: ViewSnapshot): UiState {
  const top = open.nodes.filter((n) => n.parent === null);
  return {
    openRoles: [...new Set(top.map((n) => roleGroupId(n.role)))].sort(),
    expanded: open.nodes.filter((n) => n.kind === "package" || n.kind === "component").map((n) => n.id),
  };
}

/** Where a run of Timonier's tests spent its time (ADR 0051, Q76): calls, own times, and traffic. */
const MEASURED: [string, number, number][] = [
  ["file:crates/timonier-daemon/src/server.rs", 420, 8_400_000_000],
  ["file:crates/timonier-daemon/src/client.rs", 610, 1_900_000_000],
  ["file:crates/timonier-core/src/protocol.rs", 1_250, 640_000_000],
  ["file:crates/timonier-core/src/rpc.rs", 1_250, 310_000_000],
  ["file:crates/timonier-cli/src/daemon.rs", 36, 2_700_000_000],
];

/** The time of the spans no other node called (Q76). */
const ENTERED: Record<string, number> = {
  "file:crates/timonier-cli/src/daemon.rs": 4_910_000_000,
  "file:crates/timonier-daemon/src/server.rs": 9_040_000_000,
};

/** The layer as the daemon gives it, the lineage of each node read in the snapshot. */
function measured(snapshot: ViewSnapshot): PerfMap {
  const parents = new Map(snapshot.nodes.map((n) => [n.id, n.parent]));
  const lineage = (id: string) => {
    const line: string[] = [];
    for (let at = parents.get(id); at; at = parents.get(at)) line.unshift(at);
    return line;
  };
  return {
    run: "R-12",
    // Entered from no other node: the daemon's start and its server; the rest is called.
    nodes: MEASURED.map(([node, calls, selfNs]) => ({ node, calls, selfNs, enteredNs: ENTERED[node] ?? 0 })),
    links: [
      {
        from: "file:crates/timonier-cli/src/daemon.rs",
        to: "file:crates/timonier-daemon/src/client.rs",
        calls: 36,
        ns: 2_210_000_000,
      },
      {
        from: "file:crates/timonier-daemon/src/client.rs",
        to: "file:crates/timonier-core/src/rpc.rs",
        calls: 1_250,
        ns: 310_000_000,
      },
      {
        from: "file:crates/timonier-daemon/src/server.rs",
        to: "file:crates/timonier-core/src/protocol.rs",
        calls: 420,
        ns: 640_000_000,
      },
    ],
    ancestors: Object.fromEntries(MEASURED.map(([node]) => [node, lineage(node)])),
  };
}

/** A p95 budget on the daemon, exceeded, and one that holds (ADR 0052). */
function exceeded(snapshot: ViewSnapshot): BudgetsView {
  const parents = new Map(snapshot.nodes.map((n) => [n.id, n.parent]));
  const lineage = (id: string) => {
    const line: string[] = [];
    for (let at = parents.get(id); at; at = parents.get(at)) line.unshift(at);
    return line;
  };
  const daemon = "pkg:cargo/timonier-daemon";
  return {
    checks: [
      {
        id: "N-1",
        attachedTo: "C-1",
        on: "Daemon",
        state: "exceeded",
        boxes: [daemon],
        limits: [{ kind: "p95_ms", limit: 5, measured: 7.4, exceeded: true }],
        runs: ["R-12", "R-11", "R-10"],
      },
      {
        id: "N-2",
        attachedTo: "C-2",
        on: "Store",
        state: "ok",
        boxes: ["pkg:cargo/timonier-store"],
        limits: [{ kind: "memory_mb", limit: 50, measured: 31, exceeded: false }],
        runs: ["R-12", "R-11", "R-10"],
      },
    ],
    ancestors: { [daemon]: lineage(daemon), "pkg:cargo/timonier-store": lineage("pkg:cargo/timonier-store") },
  };
}

/** The webview's App, as main.tsx runs it, with a daemon that answers from a snapshot. */
export function MapDemo({
  locale,
  open: snapshot,
  openAll,
  review,
  only,
  debt,
  certainty,
  intent,
  performance,
  budget,
  density,
  onRendered,
}: MapDemoProps) {
  // Marked as reviewed again on the page: nothing changed since.
  const [markedAt, setMarkedAt] = useState<number | undefined>();
  const open = markedAt ? justReviewed(snapshot, markedAt) : review ? withReview(snapshot, REVIEWED_AT) : snapshot;
  const [ui, setUi] = useState<UiState>(() => ({
    ...(openAll ? everythingOpen(open) : initialUi),
    ...(only ? { onlyChanges: true } : {}),
    ...(debt ? { debt: true } : {}),
    ...(certainty ? { certainty: true } : {}),
    ...(intent ? { intent: true } : {}),
    ...(performance ? { performance: true } : {}),
  }));
  // The host keeps the pins: here, the page, which the tests read.
  const [pins, setPins] = useState<Pins>({});
  const [model, setModel] = useState<ViewModel>(() => {
    const connected = reduce(initialModel, { type: "daemon", state: "connected", detail: null }).model;
    const expanded = openAll ? everythingOpen(open).expanded : [];
    return reduce(connected, { type: "snapshot", snapshot: viewOf(open, expanded, 0) }).model;
  });
  useEffect(() => {
    onRendered?.();
  }, [onRendered]);
  const apply = ({ ui: next, subscribe }: Applied) => {
    setUi(next);
    if (subscribe) {
      setModel((current) => reduce(current, { type: "snapshot", snapshot: viewOf(open, subscribe, 0) }).model);
    }
  };
  return (
    <div className="tim-demo-map">
      <App
        locale={locale}
        model={model}
        ui={ui}
        onToggle={(toggle) => apply(applyToggle(ui, toggle, model.view))}
        onCloseAll={() => apply(closeAll(ui))}
        pins={pins}
        density={density}
        onPins={(next) => {
          setPins(next);
          document.body.dataset.pins = JSON.stringify(next);
        }}
        onOpenCode={(file, span) => {
          // What the host would open: the tests read it.
          document.body.dataset.opened = JSON.stringify({ file, span });
        }}
        onToggleChanges={() => apply(toggleChanges(ui))}
        onToggleDebt={() => apply(toggleDebt(ui))}
        onToggleCertainty={() => apply(toggleCertainty(ui))}
        // What the host would ask about: the tests read it.
        onRename={(box) => (document.body.dataset.rename = JSON.stringify(box))}
        onToggleIntent={() => apply(toggleIntent(ui))}
        intent={DRAWN}
        onOpenWhiteboard={() => (document.body.dataset.whiteboard = "true")}
        onTogglePerformance={() => apply(togglePerformance(ui))}
        performance={measured(snapshot)}
        onOpenPerformance={() => (document.body.dataset.performance = "true")}
        budgets={budget ? exceeded(snapshot) : null}
        onMarkReviewed={() => {
          // What the daemon would do: a new review, the diff starts from it.
          const now = Date.now();
          setMarkedAt(now);
          document.body.dataset.reviewed = "true";
          setModel(
            (current) =>
              reduce(current, { type: "snapshot", snapshot: viewOf(justReviewed(snapshot, now), ui.expanded, 0) })
                .model,
          );
        }}
      />
    </div>
  );
}
