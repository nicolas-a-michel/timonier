import { useEffect, useState } from "react";

import { App } from "../App";
import type { Locale } from "../i18n";
import type { Density, ViewSnapshot } from "../protocol";
import type { DriftView } from "../protocol";
import {
  applyToggle,
  closeAll,
  initialUi,
  toggleChanges,
  toggleDebt,
  toggleIntent,
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
  /** Compared with a whiteboard drawn for Timonier (ADR 0042). */
  intent?: boolean;
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

/** The webview's App, as main.tsx runs it, with a daemon that answers from a snapshot. */
export function MapDemo({
  locale,
  open: snapshot,
  openAll,
  review,
  only,
  debt,
  intent,
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
    ...(intent ? { intent: true } : {}),
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
        onToggleIntent={() => apply(toggleIntent(ui))}
        intent={DRAWN}
        onOpenWhiteboard={() => (document.body.dataset.whiteboard = "true")}
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
