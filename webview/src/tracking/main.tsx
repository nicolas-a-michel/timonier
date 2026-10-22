import { StrictMode, useEffect } from "react";
import { createRoot } from "react-dom/client";

import { connectHost } from "../host";
import { resolveLocale } from "../i18n";
import {
  isTrackingMessage,
  readyMessage,
  type AgentChoice,
  type TrackingView,
  type Workflow,
  type WorkflowView,
} from "../protocol";
import { panelOf } from "./panel";
import type { Setup } from "./Bridge";
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
/** The agents to set the project up for, once the host said them, and how the setup goes (T-118). */
let agents: AgentChoice[] | undefined;
let setup: Setup = { state: "idle", detail: null };
/** The process, on the settings page (ADR 0056). */
let workflowView: WorkflowView | undefined;
let workflowProblem: string | null = null;
let announced = false;
/** The bridge asks the host (ADR 0032): the map on a box, the map, the review mark. */
const actions = {
  onReveal: (target: string, path: string[]) => host?.postMessage({ type: "reveal", target, path }),
  onOpenMap: () => host?.postMessage({ type: "openMap" }),
  onOpenWhiteboard: () => host?.postMessage({ type: "openWhiteboard" }),
  onOpenPerformance: () => host?.postMessage({ type: "openPerformance" }),
  onMarkReviewed: () => host?.postMessage({ type: "markReviewed" }),
  onConfigure: (chosen?: string[]) => {
    // Without agents, init runs in a terminal, which answers nothing back: no « running » then.
    if (chosen) {
      setup = { state: "running", detail: null };
      render();
    }
    host?.postMessage({ type: "configure", agents: chosen });
  },
};

/** Rendered: the first time, the page is ready; then the revision it shows. */
function Shown({ children }: { children: React.ReactNode }) {
  useEffect(() => {
    if (!announced) {
      // StrictMode runs effects twice in development: announce only once.
      announced = true;
      host?.postMessage(readyMessage());
      if (panel === "workflow") host?.postMessage({ type: "askWorkflow" });
    }
    if (revision !== undefined) host?.postMessage({ type: "trackingShown", panel, revision });
  });
  return children;
}

function render() {
  root.render(
    <StrictMode>
      <Shown>
        <TrackingPanel
          locale={locale}
          panel={panel}
          tracking={tracking}
          daemon={daemon}
          actions={actions}
          agents={agents}
          setup={setup}
          workflow={{
            view: workflowView,
            problem: workflowProblem,
            onSet: (workflow: Workflow) => host?.postMessage({ type: "setWorkflow", workflow }),
          }}
        />
      </Shown>
    </StrictMode>,
  );
}

window.addEventListener("message", (event: MessageEvent<unknown>) => {
  if (!isTrackingMessage(event.data)) return;
  if (event.data.type === "tracking") {
    tracking = event.data.tracking;
    revision = event.data.revision;
  } else if (event.data.type === "workflow") {
    workflowView = event.data.view ?? workflowView;
    workflowProblem = event.data.problem;
  } else if (event.data.type === "agents") {
    agents = event.data.agents;
  } else if (event.data.type === "configured") {
    setup = { state: event.data.ok ? "done" : "failed", detail: event.data.detail };
  } else {
    daemon = { state: event.data.state, detail: event.data.detail, progress: event.data.progress };
  }
  render();
});

render();
// The agents init would configure (T-118), asked at once: the daemon's first view can take long
// on a large project, and « Set up » waits for them.
if (panel === "bridge") host?.postMessage({ type: "detectAgents" });
