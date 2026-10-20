import { useEffect, useState } from "react";

import { App } from "../App";
import type { Locale } from "../i18n";
import type { Density, ViewSnapshot } from "../protocol";
import { applyToggle, closeAll, initialUi, toggleChanges, toggleDebt, type Applied, type UiState } from "../uiState";
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
  density?: Density;
  onRendered?: () => void;
}

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
export function MapDemo({ locale, open: snapshot, openAll, review, only, debt, density, onRendered }: MapDemoProps) {
  // Marked as reviewed again on the page: nothing changed since.
  const [markedAt, setMarkedAt] = useState<number | undefined>();
  const open = markedAt ? justReviewed(snapshot, markedAt) : review ? withReview(snapshot, REVIEWED_AT) : snapshot;
  const [ui, setUi] = useState<UiState>(() => ({
    ...(openAll ? everythingOpen(open) : initialUi),
    ...(only ? { onlyChanges: true } : {}),
    ...(debt ? { debt: true } : {}),
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
