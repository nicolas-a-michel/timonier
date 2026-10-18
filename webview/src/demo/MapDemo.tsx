import { useEffect, useState } from "react";

import { App } from "../App";
import type { Locale } from "../i18n";
import type { ViewSnapshot } from "../protocol";
import { applyToggle, closeAll, initialUi, type Applied, type UiState } from "../uiState";
import { initialModel, reduce, type ViewModel } from "../viewModel";
import type { Pins } from "../map/layout";
import { roleGroupId } from "../map/mapModel";
import { viewOf } from "./fakeDaemon";

export interface MapDemoProps {
  locale: Locale;
  /** A snapshot with every package and component open (test/views). */
  open: ViewSnapshot;
  /** Every role and every container open from the start: for the measurements. */
  openAll?: boolean;
  onRendered?: () => void;
}

function everythingOpen(open: ViewSnapshot): UiState {
  const top = open.nodes.filter((n) => n.parent === null);
  return {
    openRoles: [...new Set(top.map((n) => roleGroupId(n.role)))].sort(),
    expanded: open.nodes.filter((n) => n.kind === "package" || n.kind === "component").map((n) => n.id),
  };
}

/** The webview's App, as main.tsx runs it, with a daemon that answers from a snapshot. */
export function MapDemo({ locale, open, openAll, onRendered }: MapDemoProps) {
  const [ui, setUi] = useState<UiState>(() => (openAll ? everythingOpen(open) : initialUi));
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
        onPins={(next) => {
          setPins(next);
          document.body.dataset.pins = JSON.stringify(next);
        }}
        onOpenCode={(file, span) => {
          // What the host would open: the tests read it.
          document.body.dataset.opened = JSON.stringify({ file, span });
        }}
      />
    </div>
  );
}
