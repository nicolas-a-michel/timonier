import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
import { connectHost } from "./host";
import { resolveLocale } from "./i18n";
import type { Pins } from "./map/layout";
import type { Toggle } from "./map/mapModel";
import type { MapCommands } from "./map/MapView";
import { isHostMessage, readyMessage, subscribeTo } from "./protocol";
import { applyToggle, closeAll, parseUi, type Applied, type UiState } from "./uiState";
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
};

function apply({ ui: next, subscribe }: Applied) {
  ui = next;
  host?.setState?.(ui);
  if (subscribe) host?.postMessage(subscribeTo(subscribe));
  render();
}

// One function for every render: the map says the drawn files again only when they change.
const onDisplayed = (paths: string[]) => host?.postMessage({ type: "displayed", paths });
// The boxes moved by hand: the host keeps them, and gives them back to each new page.
let pins: Pins = {};
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
