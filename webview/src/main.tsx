import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
import { applyDensity } from "./density";
import { connectHost } from "./host";
import { resolveLocale } from "./i18n";
import type { Pins } from "./map/layout";
import type { Toggle } from "./map/mapModel";
import type { MapCommands } from "./map/MapView";
import {
  isHostMessage,
  readyMessage,
  subscribeTo,
  type Density,
  type BudgetsView,
  type DriftView,
  type PerfMap,
} from "./protocol";
import {
  applyToggle,
  closeAll,
  parseUi,
  reveal,
  toggleChanges,
  toggleDebt,
  toggleIntent,
  togglePerformance,
  type Applied,
  type UiState,
} from "./uiState";
import { initialModel, reduce, type ViewModel } from "./viewModel";
import "./tokens.css";
import "./design/design.css";
import "./map/map.css";
import "./app.css";

const container = document.getElementById("root");
if (!container) {
  throw new Error("Timonier webview: the #root element is missing.");
}

// Front-ends set <html lang> to the IDE's display language.
const locale = resolveLocale(document.documentElement.lang || navigator.language);
const host = connectHost();
const root = createRoot(container);
let model: ViewModel = initialModel;
// The view state survives a reload of the page (a panel moved to the background, then back).
let ui: UiState = parseUi(host?.getState?.());
// Commands that come before the map is drawn wait for it.
const mapCommands: MapCommands = { current: null, pending: [] };
let announced = false;
const announceReady = () => {
  // StrictMode runs effects twice in development: announce only once.
  if (announced) return;
  announced = true;
  host?.postMessage(readyMessage());
  host?.postMessage(subscribeTo(ui.expanded));
  // A map left compared with the whiteboard asks for the drift again (ADR 0042).
  if (ui.intent) host?.postMessage({ type: "compareIntent", on: true });
  // And one left showing its performances, the last run's (ADR 0051).
  if (ui.performance) host?.postMessage({ type: "showPerformance", on: true });
};

function apply({ ui: next, subscribe }: Applied) {
  ui = next;
  host?.setState?.(ui);
  if (subscribe) host?.postMessage(subscribeTo(subscribe));
  render();
}

// The drift the host gave while the map compares with the whiteboard (ADR 0042).
let intent: DriftView | null | undefined;
const onToggleIntent = () => {
  apply(toggleIntent(ui));
  intent = undefined;
  host?.postMessage({ type: "compareIntent", on: ui.intent === true });
  render();
};

// The whiteboard's budgets on the last runs, always (Q74, ADR 0052).
let budgets: BudgetsView | null | undefined;

// The Performances layer the host gave while the map shows it (ADR 0051).
let performance: PerfMap | null | undefined;
const onTogglePerformance = () => {
  apply(togglePerformance(ui));
  performance = undefined;
  host?.postMessage({ type: "showPerformance", on: ui.performance === true });
  render();
};

// One function for every render: the map says the drawn files again only when they change.
const onDisplayed = (paths: string[]) => host?.postMessage({ type: "displayed", paths });
// The boxes moved by hand: the host keeps them, and gives them back to each new page.
let pins: Pins = {};
// How dense the map is, from the host's settings (Q32).
let density: Density = "comfortable";
const onPins = (next: Pins) => {
  pins = next;
  host?.postMessage({ type: "pins", pins: { ...next } });
  render();
};

function render() {
  root.render(
    <StrictMode>
      <App
        locale={locale}
        model={model}
        ui={ui}
        onToggle={(toggle: Toggle) => apply(applyToggle(ui, toggle, model.view))}
        onCloseAll={() => apply(closeAll(ui))}
        onOpenCode={(file, span) => host?.postMessage({ type: "openCode", file, span })}
        onDisplayed={onDisplayed}
        pins={pins}
        onPins={onPins}
        density={density}
        onMarkReviewed={() => host?.postMessage({ type: "markReviewed" })}
        onToggleChanges={() => apply(toggleChanges(ui))}
        onToggleDebt={() => apply(toggleDebt(ui))}
        onToggleIntent={onToggleIntent}
        intent={intent}
        onOpenWhiteboard={() => host?.postMessage({ type: "openWhiteboard" })}
        onTogglePerformance={onTogglePerformance}
        performance={performance}
        onOpenPerformance={() => host?.postMessage({ type: "openPerformance" })}
        budgets={budgets}
        onReveal={(target, path) => apply(reveal(ui, target, path, model.view))}
        commandsRef={mapCommands}
        onReady={announceReady}
      />
    </StrictMode>,
  );
}

window.addEventListener("message", (event: MessageEvent<unknown>) => {
  if (!isHostMessage(event.data)) return;
  if (event.data.type === "pins") {
    pins = event.data.pins;
    render();
    return;
  }
  if (event.data.type === "density") {
    density = applyDensity(event.data.density);
    render();
    return;
  }
  if (event.data.type === "drift") {
    intent = event.data.drift;
    render();
    return;
  }
  if (event.data.type === "budgets") {
    budgets = event.data.budgets;
    render();
    return;
  }
  if (event.data.type === "perfMap") {
    performance = event.data.perf;
    render();
    return;
  }
  if (event.data.type === "command") {
    if (mapCommands.current) mapCommands.current(event.data.command);
    else mapCommands.pending.push(event.data.command);
    return;
  }
  const reduced = reduce(model, event.data);
  model = reduced.model;
  render();
  for (const effect of reduced.effects) host?.postMessage(effect);
});

render();
