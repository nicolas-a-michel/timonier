import { StrictMode, useEffect } from "react";
import { createRoot } from "react-dom/client";

import { connectHost } from "../host";
import { resolveLocale } from "../i18n";
import { isTrackingMessage, readyMessage, type TrackingView } from "../protocol";
import { panelOf } from "./panel";
import { TrackingPanel, type DaemonInfo } from "./TrackingPanel";
import "../tokens.css";
import "../design/design.css";
import "./tracking.css";

const container = document.getElementById("root");
if (!container) {
  throw new Error("Timonier webview: the #root element is missing.");
}

const panel = panelOf(container.dataset.panel);
const locale = resolveLocale(document.documentElement.lang || navigator.language);
const host = connectHost();
const root = createRoot(container);
let tracking: TrackingView | undefined;
let revision: number | undefined;
let daemon: DaemonInfo | undefined;
let announced = false;
/** The bridge asks the host (ADR 0032): the map on a box, the map, the review mark. */
const actions = {
  onReveal: (target: string, path: string[]) => host?.postMessage({ type: "reveal", target, path }),
  onOpenMap: () => host?.postMessage({ type: "openMap" }),
  onMarkReviewed: () => host?.postMessage({ type: "markReviewed" }),
};

/** Rendered: the first time, the page is ready; then the revision it shows. */
function Shown({ children }: { children: React.ReactNode }) {
  useEffect(() => {
    if (!announced) {
      // StrictMode runs effects twice in development: announce only once.
      announced = true;
      host?.postMessage(readyMessage());
    }
    if (revision !== undefined) host?.postMessage({ type: "trackingShown", panel, revision });
  });
  return children;
}

function render() {
  root.render(
    <StrictMode>
      <Shown>
        <TrackingPanel locale={locale} panel={panel} tracking={tracking} daemon={daemon} actions={actions} />
      </Shown>
    </StrictMode>,
  );
}

window.addEventListener("message", (event: MessageEvent<unknown>) => {
  if (!isTrackingMessage(event.data)) return;
  if (event.data.type === "tracking") {
    tracking = event.data.tracking;
    revision = event.data.revision;
  } else {
    daemon = { state: event.data.state, detail: event.data.detail };
  }
  render();
});

render();
